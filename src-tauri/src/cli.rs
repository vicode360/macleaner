//! Safe subprocess execution with absolute binary resolution and timeout enforcement.

use std::fs;
use std::io::{ErrorKind, Read};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(25);
const MAX_CAPTURED_OUTPUT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliOutput {
    pub stdout: String,
    pub stderr: String,
    pub status_code: Option<i32>,
    pub success: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("binary not found: {0}")]
    NotFound(String),
    #[error("command timed out after {seconds}s: {program}")]
    TimedOut { program: String, seconds: u64 },
    #[error("failed to spawn {program}: {source}")]
    SpawnFailed {
        program: String,
        #[source]
        source: std::io::Error,
    },
}

/// Resolves an executable using fixed absolute candidate paths without relying on `$PATH`.
#[must_use]
pub fn resolve_binary(name: &str) -> Option<PathBuf> {
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0')
        || name == "."
        || name == ".."
    {
        return None;
    }

    let home = dirs::home_dir();
    let candidates = candidate_paths(name, home.as_deref());
    candidates.into_iter().find(|path| is_executable_file(path))
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        (meta.permissions().mode() & 0o111) != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn candidate_paths(name: &str, home: Option<&Path>) -> Vec<PathBuf> {
    match name {
        "xcrun" => vec![PathBuf::from("/usr/bin/xcrun")],
        "docker" => {
            let mut paths = vec![
                PathBuf::from("/opt/homebrew/bin/docker"),
                PathBuf::from("/usr/local/bin/docker"),
                PathBuf::from("/Applications/Docker.app/Contents/Resources/bin/docker"),
            ];
            if let Some(home_dir) = home {
                paths.push(home_dir.join(".orbstack/bin/docker"));
            }
            paths
        }
        "orb" => {
            let mut paths = Vec::with_capacity(3);
            if let Some(home_dir) = home {
                paths.push(home_dir.join(".orbstack/bin/orb"));
            }
            paths.push(PathBuf::from("/opt/homebrew/bin/orb"));
            paths.push(PathBuf::from("/usr/local/bin/orb"));
            paths
        }
        "brew" => vec![
            PathBuf::from("/opt/homebrew/bin/brew"),
            PathBuf::from("/usr/local/bin/brew"),
        ],
        other => vec![
            PathBuf::from(format!("/opt/homebrew/bin/{other}")),
            PathBuf::from(format!("/usr/local/bin/{other}")),
            PathBuf::from(format!("/usr/bin/{other}")),
        ],
    }
}

/// Executes `program` directly with `args`, capturing stdout and stderr concurrently up to `timeout`.
///
/// # Errors
/// Returns [`CliError::NotFound`] if the executable does not exist,
/// [`CliError::SpawnFailed`] if process creation or status polling fails,
/// or [`CliError::TimedOut`] if the process does not finish within `timeout`.
pub fn run_command_with_timeout(
    program: &Path,
    args: &[&str],
    timeout: Duration,
) -> Result<CliOutput, CliError> {
    run_command_with_env_and_timeout(program, args, &[], timeout)
}

/// Executes `program` with `args` and extra `envs`, capturing bounded stdout/stderr up to `timeout`.
///
/// # Errors
/// Returns [`CliError::NotFound`] if the executable does not exist,
/// [`CliError::SpawnFailed`] if process creation or status polling fails,
/// or [`CliError::TimedOut`] if the process does not finish within `timeout`.
pub fn run_command_with_env_and_timeout(
    program: &Path,
    args: &[&str],
    envs: &[(&str, &str)],
    timeout: Duration,
) -> Result<CliOutput, CliError> {
    let program_label = program.to_string_lossy().into_owned();
    let mut command = Command::new(program);
    command
        .args(args)
        .envs(envs.iter().copied())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(unix)]
    {
        command.process_group(0);
    }

    let mut child = command.spawn().map_err(|source| {
        if source.kind() == ErrorKind::NotFound {
            CliError::NotFound(program_label.clone())
        } else {
            CliError::SpawnFailed {
                program: program_label.clone(),
                source,
            }
        }
    })?;

    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();

    let stdout_handle = thread::spawn(move || read_pipe(stdout_pipe));
    let stderr_handle = thread::spawn(move || read_pipe(stderr_pipe));

    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout_bytes = stdout_handle.join().unwrap_or_default();
                let stderr_bytes = stderr_handle.join().unwrap_or_default();
                return Ok(CliOutput {
                    stdout: String::from_utf8_lossy(&stdout_bytes).into_owned(),
                    stderr: String::from_utf8_lossy(&stderr_bytes).into_owned(),
                    status_code: status.code(),
                    success: status.success(),
                });
            }
            Ok(None) => {
                let elapsed = started.elapsed();
                if elapsed >= timeout {
                    terminate_child(&mut child);
                    let _ = stdout_handle.join();
                    let _ = stderr_handle.join();
                    return Err(CliError::TimedOut {
                        program: program_label,
                        seconds: timeout.as_secs(),
                    });
                }
                let remaining = timeout.saturating_sub(elapsed);
                thread::sleep(POLL_INTERVAL.min(remaining));
            }
            Err(source) => {
                terminate_child(&mut child);
                let _ = stdout_handle.join();
                let _ = stderr_handle.join();
                return Err(CliError::SpawnFailed {
                    program: program_label,
                    source,
                });
            }
        }
    }
}

fn terminate_child(child: &mut Child) {
    #[cfg(unix)]
    {
        let pid = child.id();
        if pid > 0 {
            let _ = Command::new("/bin/kill")
                .args(["-KILL", "--", &format!("-{pid}")])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn read_pipe<R: Read>(pipe: Option<R>) -> Vec<u8> {
    read_pipe_capped(pipe, MAX_CAPTURED_OUTPUT_BYTES)
}

fn read_pipe_capped<R: Read>(pipe: Option<R>, limit: u64) -> Vec<u8> {
    let mut buffer = Vec::new();
    if let Some(reader) = pipe {
        let _ = reader.take(limit).read_to_end(&mut buffer);
    }
    buffer
}

/// Validates a canonical 36-character `8-4-4-4-12` ASCII hexadecimal UUID string.
#[must_use]
pub fn is_valid_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 36 {
        return false;
    }

    for (idx, &byte) in bytes.iter().enumerate() {
        if matches!(idx, 8 | 13 | 18 | 23) {
            if byte != b'-' {
                return false;
            }
        } else if !byte.is_ascii_hexdigit() {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_validation_accepts_canonical_and_rejects_malformed() {
        assert!(is_valid_uuid("550e8400-e29b-41d4-a716-446655440000"));
        assert!(is_valid_uuid("ABCDEF01-2345-6789-ABCD-EF0123456789"));

        assert!(!is_valid_uuid(""));
        assert!(!is_valid_uuid("550e8400e29b41d4a716446655440000"));
        assert!(!is_valid_uuid("550e8400-e29b-41d4-a716-44665544000g"));
        assert!(!is_valid_uuid("550e8400-e29b-41d4-a716-4466554400000"));
        assert!(!is_valid_uuid("550e8400-e29b-41d4-a716-44665544000"));
        assert!(!is_valid_uuid("550e8400_e29b_41d4_a716_446655440000"));
        assert!(!is_valid_uuid(" 550e8400-e29b-41d4-a716-446655440000"));
    }

    #[test]
    fn run_command_with_timeout_succeeds_for_fast_command() {
        let output =
            run_command_with_timeout(Path::new("/usr/bin/true"), &[], Duration::from_secs(2))
                .expect("true should succeed");
        assert!(output.success);
        assert_eq!(output.status_code, Some(0));
        assert_eq!(output.stdout, "");
        assert_eq!(output.stderr, "");
    }

    #[test]
    fn run_command_with_timeout_kills_slow_process() {
        let err =
            run_command_with_timeout(Path::new("/bin/sleep"), &["5"], Duration::from_millis(120))
                .expect_err("sleep should time out");
        assert!(matches!(err, CliError::TimedOut { .. }));
    }

    #[test]
    fn run_command_with_timeout_reports_missing_binary() {
        let err = run_command_with_timeout(
            Path::new("/nonexistent/macleaner_missing_bin"),
            &[],
            Duration::from_secs(1),
        )
        .expect_err("missing binary should fail");
        assert!(matches!(err, CliError::NotFound(_)));
    }

    #[test]
    fn resolve_binary_rejects_traversal_and_finds_system_binary() {
        assert!(resolve_binary("").is_none());
        assert!(resolve_binary("../bin/sh").is_none());
        assert!(resolve_binary("foo/bar").is_none());
        assert_eq!(resolve_binary("true"), Some(PathBuf::from("/usr/bin/true")));
    }

    #[test]
    fn run_command_with_env_and_timeout_passes_env_vars() {
        let output = run_command_with_env_and_timeout(
            Path::new("/bin/sh"),
            &["-c", "printf '%s' \"$MACLEANER_TEST_ENV\""],
            &[("MACLEANER_TEST_ENV", "hardened-value")],
            Duration::from_secs(2),
        )
        .expect("sh should succeed");
        assert!(output.success);
        assert_eq!(output.stdout, "hardened-value");
    }

    #[cfg(unix)]
    #[test]
    fn timeout_terminates_entire_process_group_including_grandchildren() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let pid_file = std::env::temp_dir().join(format!(
            "macleaner_cli_pgid_{}_{nanos}.pid",
            std::process::id()
        ));
        let script = format!(
            "/bin/sleep 30 & echo $! > '{}'; wait",
            pid_file.to_string_lossy()
        );

        let err = run_command_with_timeout(
            Path::new("/bin/sh"),
            &["-c", &script],
            Duration::from_millis(200),
        )
        .expect_err("shell with grandchild sleep should time out");
        assert!(matches!(err, CliError::TimedOut { .. }));

        let pid_text = fs::read_to_string(&pid_file).unwrap_or_default();
        let _ = fs::remove_file(&pid_file);
        let grandchild_pid = pid_text.trim();
        assert!(
            !grandchild_pid.is_empty(),
            "grandchild pid should have been written"
        );

        let alive = Command::new("/bin/kill")
            .args(["-0", grandchild_pid])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        assert!(
            !alive,
            "grandchild pid {grandchild_pid} must be terminated with its process group"
        );
    }

    #[test]
    fn read_pipe_capped_truncates_at_limit() {
        let payload = vec![b'x'; 256];
        let captured = read_pipe_capped(Some(std::io::Cursor::new(payload)), 64);
        assert_eq!(captured.len(), 64);
    }

    #[cfg(unix)]
    #[test]
    fn is_executable_file_requires_execute_bit() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let file =
            std::env::temp_dir().join(format!("macleaner_cli_exec_{}_{nanos}", std::process::id()));
        fs::write(&file, b"#!/bin/sh\n").expect("write temp file");
        fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).expect("chmod 644");
        assert!(!is_executable_file(&file));

        fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).expect("chmod 755");
        assert!(is_executable_file(&file));
        let _ = fs::remove_file(&file);
    }
}
