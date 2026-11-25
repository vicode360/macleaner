//! Xcode and CoreSimulator inspection and safe cleanup.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cli::{is_valid_uuid, resolve_binary, run_command_with_timeout, CliError};
use crate::rules::{
    has_unsafe_raw_path_syntax, normalize_path, path_starts_with_platform, paths_equal_platform,
    verified_dir_under_home, RuleEngine,
};
use crate::scanner::{days_since, dir_disk_size, effective_file_size, TrashFailure};

const SIMCTL_LIST_TIMEOUT: Duration = Duration::from_secs(15);
const SIMCTL_DELETE_UNAVAILABLE_TIMEOUT: Duration = Duration::from_secs(60);
const SIMCTL_DELETE_RUNTIME_TIMEOUT: Duration = Duration::from_secs(120);

const CHILD_SCANNED_AREAS: &[(&str, &str, Option<&str>)] = &[
    ("Xcode/DerivedData", "derived_data", None),
    ("Xcode/Archives", "archives", None),
    ("Xcode/iOS DeviceSupport", "device_support", Some("iOS")),
    (
        "Xcode/watchOS DeviceSupport",
        "device_support",
        Some("watchOS"),
    ),
    ("Xcode/tvOS DeviceSupport", "device_support", Some("tvOS")),
    (
        "Xcode/visionOS DeviceSupport",
        "device_support",
        Some("visionOS"),
    ),
];

const WHOLE_DIR_AREAS: &[(&str, &str, &str)] = &[
    (
        "CoreSimulator/Caches",
        "simulator_caches",
        "CoreSimulator Caches",
    ),
    (
        "Xcode/UserData/Previews/Simulator Devices",
        "previews",
        "SwiftUI Previews Simulators",
    ),
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XcodeAreaItem {
    pub id: String,
    pub category: String,
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub inactive_days: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnavailableSimulator {
    pub udid: String,
    pub name: String,
    pub runtime: String,
    pub state: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulatorRuntime {
    pub identifier: String,
    pub name: String,
    pub build: String,
    pub size_bytes: u64,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XcodeReport {
    pub installed: bool,
    pub areas: Vec<XcodeAreaItem>,
    pub unavailable_simulators: Vec<UnavailableSimulator>,
    pub unavailable_total_bytes: u64,
    pub runtimes: Vec<SimulatorRuntime>,
    pub total_reclaimable_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XcodeActionResult {
    pub freed_bytes: u64,
    pub deleted_count: usize,
    pub failed: Vec<TrashFailure>,
}

fn find_xcrun() -> Option<PathBuf> {
    let system_xcrun = PathBuf::from("/usr/bin/xcrun");
    if system_xcrun.is_file() {
        return Some(system_xcrun);
    }
    resolve_binary("xcrun")
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

fn format_sim_runtime_label(raw: &str) -> String {
    let trimmed = raw
        .trim()
        .strip_prefix("com.apple.CoreSimulator.SimRuntime.")
        .unwrap_or(raw.trim());

    let parts: Vec<&str> = trimmed.split('-').filter(|s| !s.is_empty()).collect();
    if parts.len() >= 2
        && parts[1..]
            .iter()
            .all(|p| p.chars().all(|c| c.is_ascii_digit()))
    {
        return format!("{} {}", parts[0], parts[1..].join("."));
    }

    trimmed.replace('-', " ")
}

fn platform_display_name(platform_identifier: &str) -> Option<&'static str> {
    let lower = platform_identifier.to_ascii_lowercase();
    if lower.contains("iphonesimulator") {
        Some("iOS")
    } else if lower.contains("watchsimulator") {
        Some("watchOS")
    } else if lower.contains("appletvsimulator") {
        Some("tvOS")
    } else if lower.contains("xrsimulator") || lower.contains("visionsimulator") {
        Some("visionOS")
    } else {
        None
    }
}

fn entry_disk_size(path: &Path) -> u64 {
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

/// Parses unavailable CoreSimulator devices from `xcrun simctl list devices -j` output.
#[must_use]
pub fn parse_unavailable_simulators(json: &str) -> Vec<UnavailableSimulator> {
    let Ok(root) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    let Some(devices_map) = root.get("devices").and_then(Value::as_object) else {
        return Vec::new();
    };

    let mut unavailable = Vec::new();

    for (runtime_key, list_value) in devices_map {
        let Some(devices) = list_value.as_array() else {
            continue;
        };
        let runtime_label = format_sim_runtime_label(runtime_key);

        for device in devices {
            let Some(obj) = device.as_object() else {
                continue;
            };

            let is_available = obj
                .get("isAvailable")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let has_availability_error = obj
                .get("availabilityError")
                .and_then(Value::as_str)
                .is_some_and(|msg| !msg.trim().is_empty());

            if is_available && !has_availability_error {
                continue;
            }

            let udid = obj
                .get("udid")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_string();
            if udid.is_empty() {
                continue;
            }

            let name = obj
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("Simulator")
                .to_string();
            let state = obj
                .get("state")
                .and_then(Value::as_str)
                .unwrap_or("Shutdown")
                .to_string();

            let disk_size = obj
                .get("dataPath")
                .and_then(Value::as_str)
                .map(Path::new)
                .filter(|path| path.exists())
                .map_or(0, dir_disk_size);

            let size_bytes = if disk_size > 0 {
                disk_size
            } else {
                obj.get("dataPathSize").and_then(Value::as_u64).unwrap_or(0)
            };

            unavailable.push(UnavailableSimulator {
                udid,
                name,
                runtime: runtime_label.clone(),
                state,
                size_bytes,
            });
        }
    }

    unavailable.sort_by(|a, b| {
        b.size_bytes
            .cmp(&a.size_bytes)
            .then_with(|| a.name.cmp(&b.name))
    });
    unavailable
}

fn parse_runtime_object(
    fallback_key: Option<&str>,
    obj: &serde_json::Map<String, Value>,
) -> Option<SimulatorRuntime> {
    if obj.get("deletable").and_then(Value::as_bool) == Some(false) {
        return None;
    }

    let identifier = obj
        .get("identifier")
        .and_then(Value::as_str)
        .or(fallback_key)
        .map(str::trim)
        .filter(|id| is_valid_uuid(id))?
        .to_string();

    let build = obj
        .get("build")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let size_bytes = obj.get("sizeBytes").and_then(Value::as_u64).unwrap_or(0);
    let state = obj
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("Unknown")
        .to_string();

    let name = if let Some(explicit) = obj
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        explicit.to_string()
    } else if let (Some(platform), Some(version)) = (
        obj.get("platformIdentifier")
            .and_then(Value::as_str)
            .and_then(platform_display_name),
        obj.get("version")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty()),
    ) {
        format!("{platform} {version}")
    } else if let Some(runtime_id) = obj
        .get("runtimeIdentifier")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        format_sim_runtime_label(runtime_id)
    } else if let Some(version) = obj
        .get("version")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        format!("Simulator {version}")
    } else {
        "Simulator Runtime".to_string()
    };

    Some(SimulatorRuntime {
        identifier,
        name,
        build,
        size_bytes,
        state,
    })
}

/// Parses installed simulator runtime disk images from `xcrun simctl runtime list -j` output.
#[must_use]
pub fn parse_simulator_runtimes(json: &str) -> Vec<SimulatorRuntime> {
    let Ok(root) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };

    let mut runtimes = Vec::new();

    match &root {
        Value::Object(map) => {
            for (key, value) in map {
                if let Some(obj) = value.as_object() {
                    if let Some(runtime) = parse_runtime_object(Some(key), obj) {
                        runtimes.push(runtime);
                    }
                }
            }
        }
        Value::Array(items) => {
            for value in items {
                if let Some(obj) = value.as_object() {
                    if let Some(runtime) = parse_runtime_object(None, obj) {
                        runtimes.push(runtime);
                    }
                }
            }
        }
        _ => {}
    }

    runtimes.sort_by(|a, b| {
        b.size_bytes
            .cmp(&a.size_bytes)
            .then_with(|| a.name.cmp(&b.name))
    });
    runtimes
}

fn scan_developer_areas(home_dir: &Path) -> Vec<XcodeAreaItem> {
    let Ok(canonical_home) = fs::canonicalize(home_dir) else {
        return Vec::new();
    };
    let dev_rel = Path::new("Library/Developer");
    let mut areas = Vec::new();
    let now = SystemTime::now();

    for &(rel_dir, category, platform_prefix) in CHILD_SCANNED_AREAS {
        let Some(dir_path) = verified_dir_under_home(&canonical_home, &dev_rel.join(rel_dir))
        else {
            continue;
        };
        let Ok(entries) = fs::read_dir(&dir_path) else {
            continue;
        };

        for entry_result in entries {
            let Ok(entry) = entry_result else {
                continue;
            };
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }

            let entry_path = entry.path();
            let size_bytes = if file_type.is_dir() {
                dir_disk_size(&entry_path)
            } else {
                entry
                    .metadata()
                    .ok()
                    .map_or(0, |meta| effective_file_size(&meta))
            };
            if size_bytes == 0 {
                continue;
            }

            let raw_name = entry.file_name().to_string_lossy().into_owned();
            let display_name = match platform_prefix {
                Some(prefix) => format!("{prefix} · {raw_name}"),
                None => raw_name,
            };
            let modified = entry.metadata().ok().and_then(|meta| meta.modified().ok());
            let inactive_days = u32::try_from(days_since(modified, now)).unwrap_or(u32::MAX);
            let path_string = entry_path.to_string_lossy().into_owned();

            areas.push(XcodeAreaItem {
                id: format!("{category}:{path_string}"),
                category: category.to_string(),
                name: display_name,
                path: path_string,
                size_bytes,
                inactive_days,
            });
        }
    }

    for &(rel_dir, category, display_name) in WHOLE_DIR_AREAS {
        let Some(dir_path) = verified_dir_under_home(&canonical_home, &dev_rel.join(rel_dir))
        else {
            continue;
        };
        let Ok(meta) = fs::symlink_metadata(&dir_path) else {
            continue;
        };
        if meta.file_type().is_symlink() || !meta.is_dir() {
            continue;
        }

        let size_bytes = dir_disk_size(&dir_path);
        if size_bytes == 0 {
            continue;
        }

        let inactive_days =
            u32::try_from(days_since(meta.modified().ok(), now)).unwrap_or(u32::MAX);
        let path_string = dir_path.to_string_lossy().into_owned();
        areas.push(XcodeAreaItem {
            id: format!("{category}:{path_string}"),
            category: category.to_string(),
            name: display_name.to_string(),
            path: path_string,
            size_bytes,
            inactive_days,
        });
    }

    areas.sort_by(|a, b| {
        b.size_bytes
            .cmp(&a.size_bytes)
            .then_with(|| a.name.cmp(&b.name))
    });
    areas
}

/// Inspects Xcode build artifacts, DeviceSupport, simulator caches, previews, and CoreSimulator state.
#[must_use]
pub fn inspect_xcode() -> XcodeReport {
    let Some(home_dir) = dirs::home_dir() else {
        return XcodeReport {
            installed: false,
            areas: Vec::new(),
            unavailable_simulators: Vec::new(),
            unavailable_total_bytes: 0,
            runtimes: Vec::new(),
            total_reclaimable_bytes: 0,
        };
    };

    let dev_root = home_dir.join("Library/Developer");
    let has_dev_dirs = dev_root.join("Xcode").exists() || dev_root.join("CoreSimulator").exists();
    let should_probe_simctl = has_dev_dirs
        || Path::new("/Applications/Xcode.app").exists()
        || Path::new("/Library/Developer/CommandLineTools").exists();

    let mut simctl_available = false;
    let mut unavailable_simulators = Vec::new();
    let mut runtimes = Vec::new();

    if should_probe_simctl {
        if let Some(ref xcrun_path) = find_xcrun() {
            if let Ok(output) = run_command_with_timeout(
                xcrun_path,
                &["simctl", "list", "devices", "-j"],
                SIMCTL_LIST_TIMEOUT,
            ) {
                if output.success {
                    simctl_available = true;
                    unavailable_simulators = parse_unavailable_simulators(&output.stdout);
                }
            }

            if simctl_available {
                if let Ok(output) = run_command_with_timeout(
                    xcrun_path,
                    &["simctl", "runtime", "list", "-j"],
                    SIMCTL_LIST_TIMEOUT,
                ) {
                    if output.success {
                        runtimes = parse_simulator_runtimes(&output.stdout);
                    }
                }
            }
        }
    }

    let installed = has_dev_dirs || simctl_available;
    if !installed {
        return XcodeReport {
            installed: false,
            areas: Vec::new(),
            unavailable_simulators: Vec::new(),
            unavailable_total_bytes: 0,
            runtimes: Vec::new(),
            total_reclaimable_bytes: 0,
        };
    }

    let areas = scan_developer_areas(&home_dir);
    let areas_total = areas
        .iter()
        .map(|item| item.size_bytes)
        .fold(0_u64, u64::saturating_add);
    let unavailable_total_bytes = unavailable_simulators
        .iter()
        .map(|sim| sim.size_bytes)
        .fold(0_u64, u64::saturating_add);
    let runtimes_total = runtimes
        .iter()
        .map(|rt| rt.size_bytes)
        .fold(0_u64, u64::saturating_add);
    let total_reclaimable_bytes = areas_total
        .saturating_add(unavailable_total_bytes)
        .saturating_add(runtimes_total);

    XcodeReport {
        installed: true,
        areas,
        unavailable_simulators,
        unavailable_total_bytes,
        runtimes,
        total_reclaimable_bytes,
    }
}

/// Validates that `candidate` lies inside one of the strictly allowlisted Xcode cleanup directories.
///
/// # Errors
/// Returns an error if the path is relative, missing, a symlink, SIP/TCC-protected, or outside
/// the allowlisted `$HOME/Library/Developer/...` directories.
pub fn validate_xcode_trash_target(
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
        .map_err(|_| "Item não encontrado no disco.".to_string())?;
    if meta.file_type().is_symlink() {
        return Err("Links simbólicos não são removidos por segurança.".to_string());
    }

    let canonical =
        fs::canonicalize(&normalized).map_err(|_| "Item não encontrado no disco.".to_string())?;
    if engine.is_protected_or_ancestor(&canonical) {
        return Err("Caminho protegido pela política do sistema.".to_string());
    }

    let canonical_home = fs::canonicalize(engine.home_dir())
        .map_err(|_| "Item não encontrado no disco.".to_string())?;
    let dev_root = canonical_home.join("Library/Developer");

    for &(rel_dir, _, _) in CHILD_SCANNED_AREAS {
        let allowed_root = normalize_path(&dev_root.join(rel_dir));
        if path_starts_with_platform(&canonical, &allowed_root)
            && !paths_equal_platform(&canonical, &allowed_root)
        {
            return Ok(canonical);
        }
    }

    for &(rel_dir, _, _) in WHOLE_DIR_AREAS {
        let allowed_root = normalize_path(&dev_root.join(rel_dir));
        if path_starts_with_platform(&canonical, &allowed_root) {
            return Ok(canonical);
        }
    }

    Err("Caminho fora do escopo seguro de limpeza do Xcode.".to_string())
}

/// Validates and moves selected Xcode paths to the macOS Trash.
#[must_use]
pub fn trash_xcode_paths(
    paths: &[String],
    known_sizes: &HashMap<String, u64>,
    engine: &RuleEngine,
) -> XcodeActionResult {
    let mut normalized_inputs: Vec<(String, PathBuf)> = paths
        .iter()
        .map(|raw| (raw.clone(), normalize_path(Path::new(raw))))
        .collect();
    normalized_inputs.sort_by_key(|(_, path)| path.components().count());

    let mut accepted_normalized: Vec<PathBuf> = Vec::new();
    let mut accepted_canonical: Vec<PathBuf> = Vec::new();
    let mut failed: Vec<TrashFailure> = Vec::new();
    let mut freed_bytes = 0_u64;
    let mut deleted_count = 0_usize;

    for (raw_path, normalized) in normalized_inputs {
        let raw_candidate = Path::new(&raw_path);
        if raw_candidate.is_absolute()
            && !has_unsafe_raw_path_syntax(raw_candidate)
            && accepted_normalized
                .iter()
                .any(|parent| path_starts_with_platform(&normalized, parent))
        {
            continue;
        }

        let canonical = match validate_xcode_trash_target(raw_candidate, engine) {
            Ok(path) => path,
            Err(reason) => {
                failed.push(TrashFailure {
                    path: raw_path,
                    reason,
                });
                continue;
            }
        };

        if accepted_canonical
            .iter()
            .any(|parent| path_starts_with_platform(&canonical, parent))
        {
            continue;
        }

        let estimated_size = known_sizes
            .get(&raw_path)
            .copied()
            .unwrap_or_else(|| entry_disk_size(&canonical));

        match trash::delete(&canonical) {
            Ok(()) => {
                freed_bytes = freed_bytes.saturating_add(estimated_size);
                deleted_count = deleted_count.saturating_add(1);
                accepted_normalized.push(normalized);
                accepted_canonical.push(canonical);
            }
            Err(err) => {
                failed.push(TrashFailure {
                    path: raw_path,
                    reason: err.to_string(),
                });
            }
        }
    }

    XcodeActionResult {
        freed_bytes,
        deleted_count,
        failed,
    }
}

/// Deletes unavailable CoreSimulator devices via `xcrun simctl delete unavailable`.
///
/// # Errors
/// Returns an error if `xcrun` is unavailable or `simctl` fails.
pub fn delete_unavailable_simulators() -> Result<usize, String> {
    let xcrun = find_xcrun().ok_or_else(|| {
        "xcrun não encontrado; instale o Xcode ou Command Line Tools.".to_string()
    })?;

    let list_out = run_command_with_timeout(
        &xcrun,
        &["simctl", "list", "devices", "-j"],
        SIMCTL_LIST_TIMEOUT,
    )
    .map_err(|err| format_cli_error(&err))?;

    if !list_out.success {
        let msg = list_out.stderr.trim();
        return Err(if msg.is_empty() {
            "Falha ao listar simuladores do Xcode.".to_string()
        } else {
            msg.to_string()
        });
    }

    let before_count = parse_unavailable_simulators(&list_out.stdout).len();
    if before_count == 0 {
        return Ok(0);
    }

    let delete_out = run_command_with_timeout(
        &xcrun,
        &["simctl", "delete", "unavailable"],
        SIMCTL_DELETE_UNAVAILABLE_TIMEOUT,
    )
    .map_err(|err| format_cli_error(&err))?;

    if !delete_out.success {
        let msg = delete_out.stderr.trim();
        return Err(if msg.is_empty() {
            "Falha ao remover simuladores indisponíveis.".to_string()
        } else {
            msg.to_string()
        });
    }

    let after_count = run_command_with_timeout(
        &xcrun,
        &["simctl", "list", "devices", "-j"],
        SIMCTL_LIST_TIMEOUT,
    )
    .ok()
    .filter(|out| out.success)
    .map_or(0, |out| parse_unavailable_simulators(&out.stdout).len());

    Ok(before_count.saturating_sub(after_count))
}

/// Deletes a CoreSimulator runtime disk image by validated UUID via `xcrun simctl runtime delete`.
///
/// # Errors
/// Returns an error if `identifier` is not a valid UUID, `xcrun` is missing, or deletion fails.
pub fn delete_simulator_runtime(identifier: &str) -> Result<(), String> {
    let trimmed = identifier.trim();
    if !is_valid_uuid(trimmed) {
        return Err("Identificador UUID do runtime inválido.".to_string());
    }

    let xcrun = find_xcrun().ok_or_else(|| {
        "xcrun não encontrado; instale o Xcode ou Command Line Tools.".to_string()
    })?;

    let output = run_command_with_timeout(
        &xcrun,
        &["simctl", "runtime", "delete", trimmed],
        SIMCTL_DELETE_RUNTIME_TIMEOUT,
    )
    .map_err(|err| format_cli_error(&err))?;

    if !output.success {
        let msg = output.stderr.trim();
        return Err(if msg.is_empty() {
            "Falha ao remover runtime do simulador.".to_string()
        } else {
            msg.to_string()
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

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
            "macleaner_xcode_test_{label}_{}_{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("create temp home");
        dir
    }

    #[test]
    fn parses_unavailable_simulators_from_simctl_json() {
        let sample = r#"{
            "devices": {
                "com.apple.CoreSimulator.SimRuntime.iOS-17-2": [
                    {
                        "udid": "11111111-2222-3333-4444-555555555555",
                        "name": "iPhone 15",
                        "state": "Shutdown",
                        "isAvailable": false,
                        "availabilityError": "runtime profile not found",
                        "dataPath": "/nonexistent/path/sim1",
                        "dataPathSize": 412000000
                    },
                    {
                        "udid": "AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE",
                        "name": "iPhone 16 Pro",
                        "state": "Shutdown",
                        "isAvailable": true,
                        "dataPathSize": 999000000
                    }
                ],
                "com.apple.CoreSimulator.SimRuntime.watchOS-10-4": [
                    {
                        "udid": "99999999-8888-7777-6666-555555555555",
                        "name": "Apple Watch Series 9",
                        "state": "Shutdown",
                        "isAvailable": true,
                        "availabilityError": "incompatible runtime",
                        "dataPathSize": 120000000
                    }
                ]
            }
        }"#;

        let parsed = parse_unavailable_simulators(sample);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].udid, "11111111-2222-3333-4444-555555555555");
        assert_eq!(parsed[0].name, "iPhone 15");
        assert_eq!(parsed[0].runtime, "iOS 17.2");
        assert_eq!(parsed[0].size_bytes, 412_000_000);
        assert_eq!(parsed[1].udid, "99999999-8888-7777-6666-555555555555");
        assert_eq!(parsed[1].runtime, "watchOS 10.4");
        assert_eq!(parsed[1].size_bytes, 120_000_000);
    }

    #[test]
    fn parses_simulator_runtimes_from_simctl_runtime_json() {
        let sample = r#"{
            "11111111-2222-3333-4444-555555555555": {
                "build": "21E213",
                "identifier": "11111111-2222-3333-4444-555555555555",
                "platformIdentifier": "com.apple.platform.iphonesimulator",
                "runtimeIdentifier": "com.apple.CoreSimulator.SimRuntime.iOS-17-4",
                "sizeBytes": 7456123904,
                "state": "Ready",
                "version": "17.4"
            },
            "22222222-3333-4444-5555-666666666666": {
                "build": "22A3351",
                "platformIdentifier": "com.apple.platform.xrsimulator",
                "sizeBytes": 9100000000,
                "state": "Ready",
                "version": "2.0"
            }
        }"#;

        let runtimes = parse_simulator_runtimes(sample);
        assert_eq!(runtimes.len(), 2);
        assert_eq!(
            runtimes[0].identifier,
            "22222222-3333-4444-5555-666666666666"
        );
        assert_eq!(runtimes[0].name, "visionOS 2.0");
        assert_eq!(runtimes[0].build, "22A3351");
        assert_eq!(runtimes[0].size_bytes, 9_100_000_000);

        assert_eq!(
            runtimes[1].identifier,
            "11111111-2222-3333-4444-555555555555"
        );
        assert_eq!(runtimes[1].name, "iOS 17.4");
        assert_eq!(runtimes[1].state, "Ready");
    }

    #[test]
    fn delete_simulator_runtime_rejects_non_uuid_identifiers() {
        assert!(delete_simulator_runtime("").is_err());
        assert!(delete_simulator_runtime("not-a-uuid").is_err());
        assert!(
            delete_simulator_runtime("11111111-2222-3333-4444-555555555555; rm -rf /").is_err()
        );
        assert!(delete_simulator_runtime("../11111111-2222-3333-4444-555555555555").is_err());
    }

    #[test]
    fn validate_xcode_trash_target_allows_previews_and_rejects_userdata_siblings() {
        let home = unique_temp_home("validate");
        let engine = RuleEngine::from_toml_str(MINIMAL_RULES, &home).expect("rules");

        let dev = home.join("Library/Developer");
        let derived_child = dev.join("Xcode/DerivedData/MyApp-abc");
        let userdata = dev.join("Xcode/UserData");
        let keybindings = userdata.join("KeyBindings");
        let previews_parent = userdata.join("Previews");
        let previews_devices = previews_parent.join("Simulator Devices");
        let previews_child = previews_devices.join("DEV-UUID-1");
        let sim_caches = dev.join("CoreSimulator/Caches");
        let sim_caches_dyld = sim_caches.join("dyld");

        fs::create_dir_all(&derived_child).expect("derived");
        fs::create_dir_all(&keybindings).expect("keybindings");
        fs::create_dir_all(&previews_child).expect("previews");
        fs::create_dir_all(&sim_caches_dyld).expect("sim_caches");

        assert!(validate_xcode_trash_target(&derived_child, &engine).is_ok());
        assert!(validate_xcode_trash_target(&dev.join("Xcode/DerivedData"), &engine).is_err());

        assert!(validate_xcode_trash_target(&userdata, &engine).is_err());
        assert!(validate_xcode_trash_target(&keybindings, &engine).is_err());
        assert!(validate_xcode_trash_target(&previews_parent, &engine).is_err());

        assert!(validate_xcode_trash_target(&previews_devices, &engine).is_ok());
        assert!(validate_xcode_trash_target(&previews_child, &engine).is_ok());
        assert!(validate_xcode_trash_target(&sim_caches, &engine).is_ok());
        assert!(validate_xcode_trash_target(&sim_caches_dyld, &engine).is_ok());

        assert!(validate_xcode_trash_target(Path::new("relative/path"), &engine).is_err());

        #[cfg(unix)]
        {
            let link = dev.join("Xcode/DerivedData/escape_link");
            std::os::unix::fs::symlink(&keybindings, &link).expect("symlink");
            assert!(validate_xcode_trash_target(&link, &engine).is_err());
            let _ = fs::remove_file(&link);
        }

        let _ = fs::remove_dir(&sim_caches_dyld);
        let _ = fs::remove_dir(&sim_caches);
        let _ = fs::remove_dir(dev.join("CoreSimulator"));
        let _ = fs::remove_dir(&previews_child);
        let _ = fs::remove_dir(&previews_devices);
        let _ = fs::remove_dir(&previews_parent);
        let _ = fs::remove_dir(&keybindings);
        let _ = fs::remove_dir(&userdata);
        let _ = fs::remove_dir(&derived_child);
        let _ = fs::remove_dir(dev.join("Xcode/DerivedData"));
        let _ = fs::remove_dir(dev.join("Xcode"));
        let _ = fs::remove_dir(&dev);
        let _ = fs::remove_dir(home.join("Library"));
        let _ = fs::remove_dir(&home);
    }

    #[test]
    fn parse_simulator_runtimes_skips_non_deletable_and_non_uuid_entries() {
        let sample = r#"{
            "11111111-2222-3333-4444-555555555555": {
                "build": "21E213",
                "deletable": true,
                "identifier": "11111111-2222-3333-4444-555555555555",
                "platformIdentifier": "com.apple.platform.iphonesimulator",
                "sizeBytes": 5000000000,
                "state": "Ready",
                "version": "17.4"
            },
            "22222222-3333-4444-5555-666666666666": {
                "build": "22A100",
                "deletable": false,
                "identifier": "22222222-3333-4444-5555-666666666666",
                "platformIdentifier": "com.apple.platform.iphonesimulator",
                "sizeBytes": 8000000000,
                "state": "Ready",
                "version": "18.0"
            },
            "not-a-uuid": {
                "build": "22B100",
                "deletable": true,
                "identifier": "not-a-uuid",
                "platformIdentifier": "com.apple.platform.iphonesimulator",
                "sizeBytes": 3000000000,
                "state": "Ready",
                "version": "18.1"
            }
        }"#;

        let runtimes = parse_simulator_runtimes(sample);
        assert_eq!(runtimes.len(), 1);
        assert_eq!(
            runtimes[0].identifier,
            "11111111-2222-3333-4444-555555555555"
        );
    }

    #[cfg(unix)]
    #[test]
    fn xcode_rejects_trailing_slash_symlink_and_symlinked_allowlisted_root() {
        let home = unique_temp_home("symlink_bypass");
        let engine = RuleEngine::from_toml_str(MINIMAL_RULES, &home).expect("rules");

        let dev = home.join("Library/Developer");
        let xcode_dir = dev.join("Xcode");
        let userdata = xcode_dir.join("UserData");
        let keybindings = userdata.join("KeyBindings");
        fs::create_dir_all(&keybindings).expect("create keybindings");
        fs::write(keybindings.join("custom.idekeybindings"), vec![1_u8; 4096])
            .expect("write keybindings");

        // Symlink DerivedData -> UserData (allowlisted root escape).
        let derived_link = xcode_dir.join("DerivedData");
        std::os::unix::fs::symlink(&userdata, &derived_link).expect("symlink DerivedData");

        assert!(validate_xcode_trash_target(&derived_link.join("KeyBindings"), &engine).is_err());
        assert!(scan_developer_areas(&home).is_empty());

        // Real Archives dir containing a symlink queried with trailing slash.
        let archives = xcode_dir.join("Archives");
        fs::create_dir_all(&archives).expect("create Archives");
        let trailing_link = archives.join("escape_link");
        std::os::unix::fs::symlink(&keybindings, &trailing_link).expect("symlink archive");
        let with_slash = PathBuf::from(format!("{}/", trailing_link.to_string_lossy()));
        assert!(validate_xcode_trash_target(&with_slash, &engine).is_err());

        let _ = fs::remove_dir_all(&home);
    }
}
