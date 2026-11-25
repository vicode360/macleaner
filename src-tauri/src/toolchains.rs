//! Homebrew and modern developer toolchain cache inspection and safe cleanup.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::cli::{resolve_binary, run_command_with_env_and_timeout, CliError};
use crate::containers::parse_human_bytes;
use crate::rules::{
    has_unsafe_raw_path_syntax, normalize_path, paths_equal_platform, verified_dir_under_home,
    RuleEngine,
};
use crate::scanner::{dir_disk_size, effective_file_size};

const BREW_DRY_RUN_TIMEOUT: Duration = Duration::from_secs(20);
const BREW_CLEAN_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_PREVIEW_LINES: usize = 20;
const BREW_ENV: &[(&str, &str)] = &[
    ("HOMEBREW_NO_AUTO_UPDATE", "1"),
    ("HOMEBREW_NO_ENV_HINTS", "1"),
    ("HOMEBREW_NO_COLOR", "1"),
];

const TOOLCHAIN_CACHES: &[(&str, &str, &str, &str)] = &[
    (
        "bun-cache",
        "Bun Package Cache",
        "Bun",
        ".bun/install/cache",
    ),
    (
        "cargo-git",
        "Cargo Git Checkouts",
        "Rust",
        ".cargo/git/checkouts",
    ),
    ("go-mod-cache", "Go Module Cache", "Go", "go/pkg/mod/cache"),
    ("npm-npx", "npm npx Cache", "Node.js", ".npm/_npx"),
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomebrewReport {
    pub installed: bool,
    pub outdated_count: usize,
    pub reclaimable_bytes: u64,
    pub autoremove_packages: Vec<String>,
    pub preview_lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolchainCacheItem {
    pub id: String,
    pub name: String,
    pub ecosystem: String,
    pub path: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolchainsReport {
    pub homebrew: HomebrewReport,
    pub caches: Vec<ToolchainCacheItem>,
    pub total_reclaimable_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BrewCleanAction {
    Cleanup,
    Autoremove,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrewCleanResult {
    pub action: BrewCleanAction,
    pub freed_bytes: u64,
    pub summary: String,
}

fn format_cli_error(err: &CliError) -> String {
    match err {
        CliError::NotFound(bin) => format!("Ferramenta não encontrada: {bin}"),
        CliError::TimedOut { program, seconds } => {
            format!("{program} excedeu o tempo limite de {seconds}s.")
        }
        CliError::SpawnFailed { program, source } => {
            format!("Falha ao executar {program}: {source}")
        }
    }
}

fn split_brew_path_and_size(rest: &str) -> (&str, Option<u64>) {
    let trimmed = rest.trim();
    if let Some(open_idx) = trimmed.rfind(" (") {
        if let Some(inside) = trimmed[open_idx + 2..].strip_suffix(')') {
            let size_token = inside.rsplit(',').next().unwrap_or(inside).trim();
            let parsed = parse_human_bytes(size_token);
            if parsed > 0 || size_token.eq_ignore_ascii_case("0b") {
                return (trimmed[..open_idx].trim(), Some(parsed));
            }
        }
    }
    (trimmed, None)
}

/// Parses `brew cleanup --prune=all -n` dry-run output into item count, paths, and capped previews.
#[must_use]
pub fn parse_brew_cleanup_dry_run(stdout: &str) -> (usize, Vec<PathBuf>, Vec<String>) {
    let mut count = 0_usize;
    let mut paths = Vec::new();
    let mut preview_lines = Vec::new();

    for raw_line in stdout.lines() {
        let line = raw_line.trim();
        let Some(rest) = line.strip_prefix("Would remove:") else {
            continue;
        };
        let (path_str, _) = split_brew_path_and_size(rest);
        if path_str.is_empty() {
            continue;
        }
        count = count.saturating_add(1);
        paths.push(PathBuf::from(path_str));
        if preview_lines.len() < MAX_PREVIEW_LINES {
            preview_lines.push(line.to_string());
        }
    }

    (count, paths, preview_lines)
}

fn parse_leading_human_bytes(after: &str) -> u64 {
    let mut words = after.split_whitespace();
    let Some(first_raw) = words.next() else {
        return 0;
    };
    let first = first_raw.trim_end_matches(['.', ',']);
    let is_numeric_only = !first.is_empty()
        && first
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == ',');
    if is_numeric_only {
        if let Some(second_raw) = words.next() {
            let second = second_raw.trim_end_matches(['.', ',']);
            return parse_human_bytes(&format!("{first}{second}"));
        }
    }
    parse_human_bytes(first)
}

fn parse_brew_summary_bytes(stdout: &str) -> Option<u64> {
    for line in stdout.lines().rev() {
        let trimmed = line.trim();
        if let Some(after) = trimmed.split("approximately ").nth(1) {
            let bytes = parse_leading_human_bytes(after);
            if bytes > 0 {
                return Some(bytes);
            }
        }
        if let Some(after) = trimmed.split("freeing ").nth(1) {
            let bytes = parse_leading_human_bytes(after);
            if bytes > 0 {
                return Some(bytes);
            }
        }
    }
    None
}

fn path_size_on_disk(path: &Path) -> u64 {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return 0;
    };
    if meta.file_type().is_symlink() {
        return 0;
    }
    if meta.is_dir() {
        dir_disk_size(path)
    } else {
        effective_file_size(&meta)
    }
}

fn compute_brew_reclaimable_bytes(stdout: &str, paths: &[PathBuf]) -> u64 {
    if let Some(summary_bytes) = parse_brew_summary_bytes(stdout) {
        return summary_bytes;
    }

    let mut total = 0_u64;
    let mut any_parsed = false;

    for raw_line in stdout.lines() {
        let line = raw_line.trim();
        let Some(rest) = line.strip_prefix("Would remove:") else {
            continue;
        };
        let (path_str, maybe_size) = split_brew_path_and_size(rest);
        if let Some(size) = maybe_size {
            any_parsed = true;
            total = total.saturating_add(size);
        } else if !path_str.is_empty() {
            total = total.saturating_add(path_size_on_disk(Path::new(path_str)));
        }
    }

    if total > 0 || any_parsed {
        return total;
    }

    paths
        .iter()
        .map(|p| path_size_on_disk(p))
        .fold(0_u64, u64::saturating_add)
}

/// Parses formula names listed by `brew autoremove --dry-run`.
#[must_use]
pub fn parse_brew_autoremove_dry_run(stdout: &str) -> Vec<String> {
    let mut in_list = false;
    let mut seen = HashSet::new();
    let mut packages = Vec::new();

    for raw_line in stdout.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("==>") {
            in_list = line.contains("Would autoremove");
            continue;
        }
        if !in_list {
            continue;
        }
        if line.starts_with("Warning:") || line.starts_with("Error:") {
            continue;
        }
        if seen.insert(line.to_string()) {
            packages.push(line.to_string());
        }
    }

    packages
}

fn inspect_homebrew() -> HomebrewReport {
    let Some(brew_bin) = resolve_binary("brew") else {
        return HomebrewReport {
            installed: false,
            outdated_count: 0,
            reclaimable_bytes: 0,
            autoremove_packages: Vec::new(),
            preview_lines: Vec::new(),
        };
    };

    let (outdated_count, reclaimable_bytes, preview_lines) = run_command_with_env_and_timeout(
        &brew_bin,
        &["cleanup", "--prune=all", "-n"],
        BREW_ENV,
        BREW_DRY_RUN_TIMEOUT,
    )
    .ok()
    .filter(|out| out.success)
    .map_or((0, 0, Vec::new()), |out| {
        let (count, paths, previews) = parse_brew_cleanup_dry_run(&out.stdout);
        let bytes = compute_brew_reclaimable_bytes(&out.stdout, &paths);
        (count, bytes, previews)
    });

    let autoremove_packages = run_command_with_env_and_timeout(
        &brew_bin,
        &["autoremove", "--dry-run"],
        BREW_ENV,
        BREW_DRY_RUN_TIMEOUT,
    )
    .ok()
    .filter(|out| out.success)
    .map_or_else(Vec::new, |out| parse_brew_autoremove_dry_run(&out.stdout));

    HomebrewReport {
        installed: true,
        outdated_count,
        reclaimable_bytes,
        autoremove_packages,
        preview_lines,
    }
}

fn inspect_toolchain_caches(home_dir: &Path) -> Vec<ToolchainCacheItem> {
    let Ok(canonical_home) = fs::canonicalize(home_dir) else {
        return Vec::new();
    };
    let mut caches = Vec::new();

    for &(id, name, ecosystem, rel_path) in TOOLCHAIN_CACHES {
        let Some(full_path) = verified_dir_under_home(&canonical_home, Path::new(rel_path)) else {
            continue;
        };

        let size_bytes = dir_disk_size(&full_path);
        if size_bytes == 0 {
            continue;
        }

        caches.push(ToolchainCacheItem {
            id: id.to_string(),
            name: name.to_string(),
            ecosystem: ecosystem.to_string(),
            path: full_path.to_string_lossy().into_owned(),
            size_bytes,
        });
    }

    caches.sort_by(|a, b| {
        b.size_bytes
            .cmp(&a.size_bytes)
            .then_with(|| a.name.cmp(&b.name))
    });
    caches
}

/// Inspects Homebrew cleanup opportunities (dry-run only) and extra dev toolchain caches.
#[must_use]
pub fn inspect_toolchains() -> ToolchainsReport {
    let homebrew = inspect_homebrew();
    let caches = dirs::home_dir()
        .as_deref()
        .map_or_else(Vec::new, inspect_toolchain_caches);

    let caches_total = caches
        .iter()
        .map(|c| c.size_bytes)
        .fold(0_u64, u64::saturating_add);
    let total_reclaimable_bytes = homebrew.reclaimable_bytes.saturating_add(caches_total);

    ToolchainsReport {
        homebrew,
        caches,
        total_reclaimable_bytes,
    }
}

/// Validates that `candidate` matches one of the 4 exact allowlisted `$HOME` toolchain caches.
///
/// # Errors
/// Returns an error if the path is relative, missing, a symlink, SIP/TCC-protected, or not an
/// exact match for one of the allowlisted toolchain cache directories.
pub fn validate_toolchain_cache_target(
    candidate: &Path,
    engine: &RuleEngine,
) -> Result<PathBuf, String> {
    if !candidate.is_absolute() || has_unsafe_raw_path_syntax(candidate) {
        return Err("Caminho inválido: apenas caminhos absolutos são permitidos.".to_string());
    }

    let normalized = normalize_path(candidate);
    if engine.is_protected_or_ancestor(&normalized) {
        return Err("Caminho protegido pela política do sistema.".to_string());
    }

    let meta = fs::symlink_metadata(&normalized)
        .map_err(|_| "Cache não encontrado no disco.".to_string())?;
    if meta.file_type().is_symlink() {
        return Err("Links simbólicos não são removidos por segurança.".to_string());
    }
    if !meta.file_type().is_dir() {
        return Err("O alvo do cache deve ser um diretório.".to_string());
    }

    let canonical =
        fs::canonicalize(&normalized).map_err(|_| "Cache não encontrado no disco.".to_string())?;
    if engine.is_protected_or_ancestor(&canonical) {
        return Err("Caminho protegido pela política do sistema.".to_string());
    }

    let canonical_home = fs::canonicalize(engine.home_dir())
        .map_err(|_| "Cache não encontrado no disco.".to_string())?;

    for &(_, _, _, rel_path) in TOOLCHAIN_CACHES {
        let allowed = normalize_path(&canonical_home.join(rel_path));
        if paths_equal_platform(&canonical, &allowed) {
            return Ok(canonical);
        }
    }

    Err("Caminho fora dos caches de toolchains permitidos.".to_string())
}

/// Validates and moves a single allowlisted toolchain cache directory to the macOS Trash.
///
/// # Errors
/// Returns an error if validation fails or moving to Trash fails.
pub fn trash_toolchain_cache(
    path: &str,
    known_size: Option<u64>,
    engine: &RuleEngine,
) -> Result<u64, String> {
    let canonical = validate_toolchain_cache_target(Path::new(path), engine)?;
    let size = known_size.unwrap_or_else(|| dir_disk_size(&canonical));
    trash::delete(&canonical).map_err(|err| err.to_string())?;
    Ok(size)
}

/// Runs `brew cleanup --prune=all` or `brew autoremove`.
///
/// # Errors
/// Returns an error if `brew` is missing or the Homebrew command fails.
pub fn run_brew_clean(action: BrewCleanAction) -> Result<BrewCleanResult, String> {
    let brew_bin =
        resolve_binary("brew").ok_or_else(|| "Homebrew não encontrado no sistema.".to_string())?;

    let before_bytes = match action {
        BrewCleanAction::Cleanup => run_command_with_env_and_timeout(
            &brew_bin,
            &["cleanup", "--prune=all", "-n"],
            BREW_ENV,
            BREW_DRY_RUN_TIMEOUT,
        )
        .ok()
        .filter(|out| out.success)
        .map_or(0, |out| {
            let (_, paths, _) = parse_brew_cleanup_dry_run(&out.stdout);
            compute_brew_reclaimable_bytes(&out.stdout, &paths)
        }),
        BrewCleanAction::Autoremove => 0,
    };

    let args: &[&str] = match action {
        BrewCleanAction::Cleanup => &["cleanup", "--prune=all"],
        BrewCleanAction::Autoremove => &["autoremove"],
    };

    let output = run_command_with_env_and_timeout(&brew_bin, args, BREW_ENV, BREW_CLEAN_TIMEOUT)
        .map_err(|err| format_cli_error(&err))?;

    if !output.success {
        let msg = output.stderr.trim();
        return Err(if msg.is_empty() {
            "Falha ao executar limpeza do Homebrew.".to_string()
        } else {
            msg.to_string()
        });
    }

    let freed_bytes = if let Some(parsed) = parse_brew_summary_bytes(&output.stdout)
        .or_else(|| parse_brew_summary_bytes(&output.stderr))
    {
        parsed
    } else if action == BrewCleanAction::Cleanup && before_bytes > 0 {
        let after_bytes = run_command_with_env_and_timeout(
            &brew_bin,
            &["cleanup", "--prune=all", "-n"],
            BREW_ENV,
            BREW_DRY_RUN_TIMEOUT,
        )
        .ok()
        .filter(|out| out.success)
        .map_or(0, |out| {
            let (_, paths, _) = parse_brew_cleanup_dry_run(&out.stdout);
            compute_brew_reclaimable_bytes(&out.stdout, &paths)
        });
        before_bytes.saturating_sub(after_bytes)
    } else {
        0
    };

    let summary = output
        .stdout
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .or_else(|| {
            output
                .stderr
                .lines()
                .rev()
                .map(str::trim)
                .find(|line| !line.is_empty())
        })
        .unwrap_or("Limpeza do Homebrew concluída.")
        .to_string();

    Ok(BrewCleanResult {
        action,
        freed_bytes,
        summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    const MINIMAL_RULES: &str = r#"
[protected]
sip_prefixes = ["/System", "/usr/bin"]
tcc_user_paths = ["~/.ssh", "~/Library/Mail"]
"#;

    fn unique_temp_home(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "macleaner_toolchains_test_{label}_{}_{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("create temp home");
        dir
    }

    #[test]
    fn parses_brew_cleanup_dry_run_lines_and_sizes() {
        let sample = "\
Would remove: /opt/homebrew/Cellar/node/20.11.0 (1,923 files, 74.2MB)
Would remove: /Users/dev/Library/Caches/Homebrew/downloads/abc--foo.tar.gz (12.4MB)
Would remove: /Users/dev/Library/Logs/Homebrew/node (64B)
==> This operation would free approximately 86.6MB of disk space.
";
        let (count, paths, previews) = parse_brew_cleanup_dry_run(sample);
        assert_eq!(count, 3);
        assert_eq!(paths.len(), 3);
        assert_eq!(paths[0], PathBuf::from("/opt/homebrew/Cellar/node/20.11.0"));
        assert_eq!(
            paths[1],
            PathBuf::from("/Users/dev/Library/Caches/Homebrew/downloads/abc--foo.tar.gz")
        );
        assert_eq!(previews.len(), 3);
        assert_eq!(compute_brew_reclaimable_bytes(sample, &paths), 86_600_000);

        let no_summary = "\
Would remove: /opt/homebrew/Cellar/node/20.11.0 (1,923 files, 74.2MB)
Would remove: /Users/dev/Library/Caches/Homebrew/downloads/abc--foo.tar.gz (12.4MB)
";
        let (_, paths2, _) = parse_brew_cleanup_dry_run(no_summary);
        assert_eq!(
            compute_brew_reclaimable_bytes(no_summary, &paths2),
            86_600_000
        );
    }

    #[test]
    fn parses_brew_autoremove_dry_run_formulae() {
        let sample = "\
==> Would autoremove 3 unneeded formulae:
brotli
c-ares
libnghttp2
==> Running `brew cleanup`...
ignored-after-next-header
";
        let packages = parse_brew_autoremove_dry_run(sample);
        assert_eq!(packages, vec!["brotli", "c-ares", "libnghttp2"]);
        assert!(parse_brew_autoremove_dry_run("").is_empty());
    }

    #[test]
    fn validate_toolchain_cache_target_enforces_exact_allowlist() {
        let home = unique_temp_home("validate");
        let engine = RuleEngine::from_toml_str(MINIMAL_RULES, &home).expect("rules");

        let bun = home.join(".bun/install/cache");
        let cargo_git = home.join(".cargo/git/checkouts");
        let cargo_registry = home.join(".cargo/registry");
        let go_mod = home.join("go/pkg/mod/cache");
        let npm_npx = home.join(".npm/_npx");

        fs::create_dir_all(&bun).expect("bun");
        fs::create_dir_all(&cargo_git).expect("cargo_git");
        fs::create_dir_all(&cargo_registry).expect("cargo_registry");
        fs::create_dir_all(&go_mod).expect("go_mod");
        fs::create_dir_all(&npm_npx).expect("npm_npx");

        assert!(validate_toolchain_cache_target(&bun, &engine).is_ok());
        assert!(validate_toolchain_cache_target(&cargo_git, &engine).is_ok());
        assert!(validate_toolchain_cache_target(&go_mod, &engine).is_ok());
        assert!(validate_toolchain_cache_target(&npm_npx, &engine).is_ok());

        assert!(validate_toolchain_cache_target(&home.join(".bun"), &engine).is_err());
        assert!(validate_toolchain_cache_target(&home.join(".cargo"), &engine).is_err());
        assert!(validate_toolchain_cache_target(&cargo_registry, &engine).is_err());
        assert!(validate_toolchain_cache_target(&home.join("go/pkg/mod"), &engine).is_err());
        assert!(validate_toolchain_cache_target(&home.join(".npm"), &engine).is_err());
        assert!(validate_toolchain_cache_target(Path::new(".bun/install/cache"), &engine).is_err());

        let _ = fs::remove_dir(&bun);
        let _ = fs::remove_dir(home.join(".bun/install"));
        let _ = fs::remove_dir(home.join(".bun"));
        let _ = fs::remove_dir(&cargo_git);
        let _ = fs::remove_dir(home.join(".cargo/git"));
        let _ = fs::remove_dir(&cargo_registry);
        let _ = fs::remove_dir(home.join(".cargo"));
        let _ = fs::remove_dir(&go_mod);
        let _ = fs::remove_dir(home.join("go/pkg/mod"));
        let _ = fs::remove_dir(home.join("go/pkg"));
        let _ = fs::remove_dir(home.join("go"));
        let _ = fs::remove_dir(&npm_npx);
        let _ = fs::remove_dir(home.join(".npm"));
        let _ = fs::remove_dir(&home);
    }

    #[test]
    fn parse_brew_summary_bytes_handles_punctuation_and_spaced_units() {
        assert_eq!(
            parse_brew_summary_bytes("==> This operation would free approximately 86.6 MB.\n"),
            Some(86_600_000)
        );
        assert_eq!(
            parse_brew_summary_bytes("==> This operation would free approximately 86.6MB.\n"),
            Some(86_600_000)
        );
        assert_eq!(
            parse_brew_summary_bytes("Pruned 4 files, freeing 1.2 GB,\n"),
            Some(1_200_000_000)
        );
        assert_eq!(
            parse_brew_summary_bytes("Pruned 1 file, freeing 512 MiB.\n"),
            Some(536_870_912)
        );
        assert_eq!(
            parse_brew_summary_bytes("==> This operation would affect approximately 3 files.\n"),
            None
        );
    }

    #[cfg(unix)]
    #[test]
    fn toolchain_cache_rejects_symlinks_trailing_slashes_and_non_dirs() {
        let home = unique_temp_home("symlink_bypass");
        let outside = unique_temp_home("outside_target");
        let outside_cache = outside.join("cache");
        fs::create_dir_all(&outside_cache).expect("create outside cache");
        fs::write(outside_cache.join("pkg.bin"), vec![1_u8; 4096]).expect("write pkg");

        let engine = RuleEngine::from_toml_str(MINIMAL_RULES, &home).expect("rules");

        // 1. Direct leaf symlink: .bun/install/cache -> outside_cache (with and without trailing `/`).
        fs::create_dir_all(home.join(".bun/install")).expect("create .bun/install");
        let bun_link = home.join(".bun/install/cache");
        std::os::unix::fs::symlink(&outside_cache, &bun_link).expect("symlink bun cache");
        assert!(validate_toolchain_cache_target(&bun_link, &engine).is_err());
        let bun_link_slash = PathBuf::from(format!("{}/", bun_link.to_string_lossy()));
        assert!(validate_toolchain_cache_target(&bun_link_slash, &engine).is_err());

        // 2. Intermediate parent symlink: .cargo/git -> outside (so .cargo/git/checkouts is a real dir outside home).
        let outside_checkouts = outside.join("checkouts");
        fs::create_dir_all(&outside_checkouts).expect("create outside checkouts");
        fs::write(outside_checkouts.join("crate.rs"), vec![2_u8; 4096]).expect("write crate");
        fs::create_dir_all(home.join(".cargo")).expect("create .cargo");
        std::os::unix::fs::symlink(&outside, home.join(".cargo/git")).expect("symlink .cargo/git");
        let cargo_git_checkouts = home.join(".cargo/git/checkouts");
        assert!(validate_toolchain_cache_target(&cargo_git_checkouts, &engine).is_err());

        // 3. Regular file instead of directory at .npm/_npx.
        fs::create_dir_all(home.join(".npm")).expect("create .npm");
        let npm_file = home.join(".npm/_npx");
        fs::write(&npm_file, b"not a dir").expect("write file");
        assert!(validate_toolchain_cache_target(&npm_file, &engine).is_err());

        // inspect_toolchain_caches must skip all three!
        assert!(inspect_toolchain_caches(&home).is_empty());

        let _ = fs::remove_dir_all(&home);
        let _ = fs::remove_dir_all(&outside);
    }
}
