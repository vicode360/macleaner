//! Docker Desktop and OrbStack container storage inspection and guided pruning.

use std::fs;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cli::{resolve_binary, run_command_with_timeout, CliError};
use crate::rules::verified_dir_under_home;
use crate::scanner::{dir_disk_size, effective_file_size};

const DOCKER_DF_TIMEOUT: Duration = Duration::from_secs(10);
const DOCKER_INFO_TIMEOUT: Duration = Duration::from_secs(5);
const DOCKER_PRUNE_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DockerPruneScope {
    BuildCache,
    UnusedImages,
    StoppedContainers,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerResourceRow {
    pub kind: String,
    pub total_count: u64,
    pub active_count: u64,
    pub size_bytes: u64,
    pub reclaimable_bytes: u64,
    pub reclaimable_percent: u8,
    pub prune_scope: Option<DockerPruneScope>,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerReport {
    pub installed: bool,
    pub daemon_running: bool,
    pub engine_name: String,
    pub orbstack_detected: bool,
    pub disk_footprint_bytes: u64,
    pub total_reclaimable_bytes: u64,
    pub resources: Vec<DockerResourceRow>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerPruneResult {
    pub scope: DockerPruneScope,
    pub reclaimed_bytes: u64,
    pub raw_summary: String,
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

/// Parses human-readable byte strings emitted by Docker and Homebrew CLIs (SI and IEC units).
#[must_use]
pub fn parse_human_bytes(raw: &str) -> u64 {
    let head = raw.split('(').next().unwrap_or(raw).trim();
    if head.is_empty() {
        return 0;
    }

    let split_idx = head
        .find(|c: char| !c.is_ascii_digit() && c != '.' && c != ',')
        .unwrap_or(head.len());

    let num_part = head[..split_idx].trim().replace(',', "");
    let unit_part = head[split_idx..].trim().to_ascii_lowercase();

    let Ok(value) = num_part.parse::<f64>() else {
        return 0;
    };
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }

    let multiplier: f64 = match unit_part.as_str() {
        "" | "b" | "byte" | "bytes" => 1.0,
        "kb" | "k" => 1_000.0,
        "mb" | "m" => 1_000_000.0,
        "gb" | "g" => 1_000_000_000.0,
        "tb" | "t" => 1_000_000_000_000.0,
        "pb" | "p" => 1_000_000_000_000_000.0,
        "kib" => 1_024.0,
        "mib" => 1_048_576.0,
        "gib" => 1_073_741_824.0,
        "tib" => 1_099_511_627_776.0,
        "pib" => 1_125_899_906_842_624.0,
        _ => return 0,
    };

    let bytes = (value * multiplier).round();
    if !bytes.is_finite() || bytes <= 0.0 {
        return 0;
    }

    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "clamped positive finite byte count"
    )]
    let clamped = bytes.min(u64::MAX as f64) as u64;
    clamped
}

fn parse_count_field(value: Option<&Value>) -> u64 {
    match value {
        Some(Value::Number(n)) => n.as_u64().unwrap_or(0),
        Some(Value::String(s)) => s.trim().parse::<u64>().unwrap_or(0),
        _ => 0,
    }
}

fn parse_percent_suffix(raw: &str) -> Option<u8> {
    let start = raw.find('(')?;
    let rest = &raw[start + 1..];
    let end = rest.find('%')?;
    let pct = rest[..end].trim().parse::<f64>().ok()?;
    if !pct.is_finite() || pct < 0.0 {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to 0..=100"
    )]
    Some(pct.round().min(100.0) as u8)
}

fn parse_df_object(obj: &serde_json::Map<String, Value>) -> Option<DockerResourceRow> {
    let raw_type = obj.get("Type").and_then(Value::as_str)?.trim();
    let (kind, prune_scope, read_only) = match raw_type {
        "Build Cache" => ("build-cache", Some(DockerPruneScope::BuildCache), false),
        "Images" => ("images", Some(DockerPruneScope::UnusedImages), false),
        "Containers" => (
            "containers",
            Some(DockerPruneScope::StoppedContainers),
            false,
        ),
        "Local Volumes" => ("volumes", None, true),
        other => (other, None, true),
    };

    let total_count = parse_count_field(obj.get("TotalCount"));
    let active_count = parse_count_field(obj.get("Active"));
    let size_raw = obj.get("Size").and_then(Value::as_str).unwrap_or("0B");
    let reclaimable_raw = obj
        .get("Reclaimable")
        .and_then(Value::as_str)
        .unwrap_or("0B");

    let size_bytes = parse_human_bytes(size_raw);
    let reclaimable_bytes = parse_human_bytes(reclaimable_raw);
    let reclaimable_percent = parse_percent_suffix(reclaimable_raw).unwrap_or_else(|| {
        if size_bytes == 0 {
            0
        } else {
            let pct = u128::from(reclaimable_bytes.min(size_bytes)).saturating_mul(100)
                / u128::from(size_bytes);
            u8::try_from(pct.min(100)).unwrap_or(100)
        }
    });

    Some(DockerResourceRow {
        kind: kind.to_string(),
        total_count,
        active_count,
        size_bytes,
        reclaimable_bytes,
        reclaimable_percent,
        prune_scope,
        read_only,
    })
}

/// Parses NDJSON (or JSON array) output from `docker system df --format '{{json .}}'`.
#[must_use]
pub fn parse_docker_system_df(stdout: &str) -> Vec<DockerResourceRow> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    if trimmed.starts_with('[') {
        if let Ok(Value::Array(items)) = serde_json::from_str::<Value>(trimmed) {
            return items
                .iter()
                .filter_map(Value::as_object)
                .filter_map(parse_df_object)
                .collect();
        }
    }

    trimmed
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|val| val.as_object().and_then(parse_df_object))
        .collect()
}

fn parse_docker_reclaimed_space(stdout: &str) -> u64 {
    for line in stdout.lines().rev() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed
            .strip_prefix("Total reclaimed space:")
            .or_else(|| trimmed.strip_prefix("Total:"))
        {
            return parse_human_bytes(rest);
        }
    }
    0
}

fn is_daemon_offline_message(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    lower.contains("cannot connect to the docker daemon")
        || lower.contains("is the docker daemon running")
        || lower.contains("connect: no such file")
        || lower.contains("connection refused")
        || lower.contains("error during connect")
}

fn measure_container_disk_footprint(home_dir: &Path) -> u64 {
    let mut total = 0_u64;
    let canonical_home = fs::canonicalize(home_dir).ok();

    let docker_container_dir = home_dir.join("Library/Containers/com.docker.docker");
    if docker_container_dir.exists() {
        let raw_disk_candidates = [
            docker_container_dir.join("Data/vms/0/data/Docker.raw"),
            docker_container_dir.join("Data/vms/0/Docker.raw"),
        ];
        if let Some(raw_size) = raw_disk_candidates.iter().find_map(|path| {
            fs::symlink_metadata(path)
                .ok()
                .filter(|m| !m.file_type().is_symlink() && m.is_file())
                .map(|m| effective_file_size(&m))
        }) {
            total = total.saturating_add(raw_size);
        } else {
            total = total.saturating_add(dir_disk_size(&docker_container_dir));
        }
    }

    let orbstack_dir = home_dir.join(".orbstack");
    if fs::symlink_metadata(&orbstack_dir).is_ok() {
        total = total.saturating_add(dir_disk_size(&orbstack_dir));
        if let Some(ref canon_home) = canonical_home {
            if let Some(orbstack_group_data) = verified_dir_under_home(
                canon_home,
                Path::new("Library/Group Containers/HUAQ24HBR6.dev.orbstack/data"),
            ) {
                total = total.saturating_add(dir_disk_size(&orbstack_group_data));
            }
        }
    }

    total
}

fn detect_engine_name(docker_bin: &Path, orbstack_detected: bool) -> (String, bool) {
    if let Ok(output) = run_command_with_timeout(
        docker_bin,
        &["info", "--format", "{{.OperatingSystem}}"],
        DOCKER_INFO_TIMEOUT,
    ) {
        if output.success {
            let os = output.stdout.trim();
            let lower = os.to_ascii_lowercase();
            if lower.contains("orbstack") {
                return ("OrbStack".to_string(), true);
            }
            if lower.contains("docker desktop") {
                return ("Docker Desktop".to_string(), orbstack_detected);
            }
            if lower.contains("colima") {
                return ("Colima".to_string(), orbstack_detected);
            }
            if !os.is_empty() {
                return (os.to_string(), orbstack_detected);
            }
        }
    }

    let fallback = if orbstack_detected {
        "OrbStack"
    } else {
        "Docker Desktop"
    };
    (fallback.to_string(), orbstack_detected)
}

/// Inspects Docker Desktop and OrbStack storage usage without mutating any container state.
#[must_use]
pub fn inspect_containers() -> DockerReport {
    let docker_bin = resolve_binary("docker");
    let orb_bin = resolve_binary("orb");

    let home_dir = dirs::home_dir();
    let orbstack_dir_exists = home_dir
        .as_ref()
        .is_some_and(|home| home.join(".orbstack").exists());
    let orbstack_detected = orb_bin.is_some() || orbstack_dir_exists;
    let disk_footprint_bytes = home_dir
        .as_deref()
        .map_or(0, measure_container_disk_footprint);

    let Some(docker_path) = docker_bin else {
        return DockerReport {
            installed: false,
            daemon_running: false,
            engine_name: if orbstack_detected {
                "OrbStack".to_string()
            } else {
                "Docker".to_string()
            },
            orbstack_detected,
            disk_footprint_bytes,
            total_reclaimable_bytes: 0,
            resources: Vec::new(),
            error: None,
        };
    };

    let df_output = match run_command_with_timeout(
        &docker_path,
        &["system", "df", "--format", "{{json .}}"],
        DOCKER_DF_TIMEOUT,
    ) {
        Ok(out) => out,
        Err(err) => {
            return DockerReport {
                installed: true,
                daemon_running: false,
                engine_name: if orbstack_detected {
                    "OrbStack".to_string()
                } else {
                    "Docker Desktop".to_string()
                },
                orbstack_detected,
                disk_footprint_bytes,
                total_reclaimable_bytes: 0,
                resources: Vec::new(),
                error: Some(format_cli_error(&err)),
            };
        }
    };

    if !df_output.success {
        let stderr = df_output.stderr.trim();
        let error = if stderr.is_empty() || is_daemon_offline_message(stderr) {
            None
        } else {
            Some(stderr.to_string())
        };
        return DockerReport {
            installed: true,
            daemon_running: false,
            engine_name: if orbstack_detected {
                "OrbStack".to_string()
            } else {
                "Docker Desktop".to_string()
            },
            orbstack_detected,
            disk_footprint_bytes,
            total_reclaimable_bytes: 0,
            resources: Vec::new(),
            error,
        };
    }

    let (engine_name, orbstack_detected) = detect_engine_name(&docker_path, orbstack_detected);
    let resources = parse_docker_system_df(&df_output.stdout);
    let total_reclaimable_bytes = resources
        .iter()
        .filter(|row| !row.read_only && row.prune_scope.is_some())
        .map(|row| row.reclaimable_bytes)
        .fold(0_u64, u64::saturating_add);

    DockerReport {
        installed: true,
        daemon_running: true,
        engine_name,
        orbstack_detected,
        disk_footprint_bytes,
        total_reclaimable_bytes,
        resources,
        error: None,
    }
}

/// Runs a non-destructive Docker prune command for the chosen scope (volumes are never pruned).
///
/// # Errors
/// Returns an error if `docker` is missing, the daemon is unreachable, or the prune fails.
pub fn prune_docker(scope: DockerPruneScope) -> Result<DockerPruneResult, String> {
    let docker_path = resolve_binary("docker")
        .ok_or_else(|| "Docker CLI não encontrado no sistema.".to_string())?;

    let args: &[&str] = match scope {
        DockerPruneScope::BuildCache => &["builder", "prune", "-f"],
        DockerPruneScope::UnusedImages => &["image", "prune", "-f"],
        DockerPruneScope::StoppedContainers => &["container", "prune", "-f"],
    };

    let output = run_command_with_timeout(&docker_path, args, DOCKER_PRUNE_TIMEOUT)
        .map_err(|err| format_cli_error(&err))?;

    if !output.success {
        let msg = output.stderr.trim();
        return Err(if msg.is_empty() {
            "Falha ao executar prune no Docker.".to_string()
        } else {
            msg.to_string()
        });
    }

    let reclaimed_bytes = parse_docker_reclaimed_space(&output.stdout);
    let raw_summary = output
        .stdout
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("Prune concluído.")
        .to_string();

    Ok(DockerPruneResult {
        scope,
        reclaimed_bytes,
        raw_summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_human_bytes_for_si_and_iec_units() {
        assert_eq!(parse_human_bytes("0B"), 0);
        assert_eq!(parse_human_bytes("12.4kB"), 12_400);
        assert_eq!(parse_human_bytes("412MB"), 412_000_000);
        assert_eq!(parse_human_bytes("23.7GB"), 23_700_000_000);
        assert_eq!(parse_human_bytes("1.5TB"), 1_500_000_000_000);
        assert_eq!(parse_human_bytes("128KiB"), 131_072);
        assert_eq!(parse_human_bytes("512MiB"), 536_870_912);
        assert_eq!(parse_human_bytes("2.5GiB"), 2_684_354_560);
        assert_eq!(parse_human_bytes("4.2GB (94%)"), 4_200_000_000);
        assert_eq!(parse_human_bytes(""), 0);
        assert_eq!(parse_human_bytes("invalid"), 0);
    }

    #[test]
    fn parses_docker_system_df_ndjson_and_keeps_volumes_read_only() {
        let ndjson = r#"{"Active":"2","Reclaimable":"4.12GB (65%)","Size":"6.34GB","TotalCount":"14","Type":"Images"}
{"Active":"1","Reclaimable":"120MB (50%)","Size":"240MB","TotalCount":"4","Type":"Containers"}
{"Active":"3","Reclaimable":"8.5GB (40%)","Size":"21.2GB","TotalCount":"7","Type":"Local Volumes"}
{"Active":"0","Reclaimable":"15.4GB (100%)","Size":"15.4GB","TotalCount":"92","Type":"Build Cache"}
"#;

        let rows = parse_docker_system_df(ndjson);
        assert_eq!(rows.len(), 4);

        assert_eq!(rows[0].kind, "images");
        assert_eq!(rows[0].total_count, 14);
        assert_eq!(rows[0].active_count, 2);
        assert_eq!(rows[0].size_bytes, 6_340_000_000);
        assert_eq!(rows[0].reclaimable_bytes, 4_120_000_000);
        assert_eq!(rows[0].reclaimable_percent, 65);
        assert_eq!(rows[0].prune_scope, Some(DockerPruneScope::UnusedImages));
        assert!(!rows[0].read_only);

        assert_eq!(rows[1].kind, "containers");
        assert_eq!(
            rows[1].prune_scope,
            Some(DockerPruneScope::StoppedContainers)
        );
        assert!(!rows[1].read_only);

        assert_eq!(rows[2].kind, "volumes");
        assert_eq!(rows[2].prune_scope, None);
        assert!(rows[2].read_only);

        assert_eq!(rows[3].kind, "build-cache");
        assert_eq!(rows[3].prune_scope, Some(DockerPruneScope::BuildCache));
        assert!(!rows[3].read_only);
        assert_eq!(rows[3].reclaimable_percent, 100);
    }

    #[test]
    fn extracts_total_reclaimed_space_from_prune_output() {
        let out = "\
Deleted build cache objects:
abc123def456
7890fedcba12

Total reclaimed space: 3.45GB
";
        assert_eq!(parse_docker_reclaimed_space(out), 3_450_000_000);
        let buildkit_out = "ID\tRECLAIMABLE\tSIZE\nabc\ttrue\t3.45GB\nTotal:\t3.45GB\n";
        assert_eq!(parse_docker_reclaimed_space(buildkit_out), 3_450_000_000);
        assert_eq!(parse_docker_reclaimed_space("Total: 512MB\n"), 512_000_000);
        assert_eq!(
            parse_docker_reclaimed_space("Total reclaimed space: 0B\n"),
            0
        );
        assert_eq!(parse_docker_reclaimed_space("Nothing pruned"), 0);
    }

    #[test]
    fn measure_container_disk_footprint_includes_orbstack_group_container_data() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let home = std::env::temp_dir().join(format!(
            "macleaner_containers_orb_{}_{nanos}",
            std::process::id()
        ));
        let orb_dir = home.join(".orbstack");
        let orb_data = home.join("Library/Group Containers/HUAQ24HBR6.dev.orbstack/data");
        fs::create_dir_all(&orb_dir).expect("create .orbstack");
        fs::create_dir_all(&orb_data).expect("create orb group data");
        fs::write(orb_dir.join("config.json"), vec![1_u8; 4096]).expect("write config");
        fs::write(orb_data.join("data.img"), vec![2_u8; 16384]).expect("write data");

        let footprint = measure_container_disk_footprint(&home);
        let _ = fs::remove_dir_all(&home);
        assert_eq!(footprint, 4096 + 16384);
    }
}
