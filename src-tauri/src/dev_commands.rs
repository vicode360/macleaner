//! Tauri IPC commands for v0.3.0 Developer Ecosystem tools.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

use tauri::{Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::commands::{save_persisted_config, AppError, AppState};
use crate::containers::{
    inspect_containers, prune_docker as run_docker_prune, DockerPruneResult, DockerPruneScope,
    DockerReport,
};
use crate::dev_projects::{
    effective_roots, scan_dev_projects as run_dev_projects_scan, trash_dev_artifacts,
    validate_dev_root, DevProjectsReport, DevTrashResult,
};
use crate::rules::{normalize_path, path_starts_with_platform, paths_equal_platform};
use crate::toolchains::{
    inspect_toolchains as run_toolchains_inspect, run_brew_clean as execute_brew_clean,
    trash_toolchain_cache as execute_trash_toolchain_cache, BrewCleanAction, BrewCleanResult,
    ToolchainsReport,
};
use crate::xcode::{
    delete_simulator_runtime as execute_delete_simulator_runtime,
    delete_unavailable_simulators as execute_delete_unavailable_simulators,
    inspect_xcode as run_xcode_inspect, trash_xcode_paths, XcodeActionResult, XcodeReport,
};

const XCODE_UNAVAILABLE_SIZE_KEY: &str = "__xcode_unavailable_simulators__";
const XCODE_RUNTIME_SIZE_PREFIX: &str = "__xcode_runtime:";

struct AtomicFlagGuard {
    flag: Arc<AtomicBool>,
}

impl AtomicFlagGuard {
    fn try_acquire(flag: &Arc<AtomicBool>) -> Option<Self> {
        flag.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| Self {
                flag: Arc::clone(flag),
            })
    }
}

impl Drop for AtomicFlagGuard {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::Release);
    }
}

struct SuppressHideGuard {
    app: tauri::AppHandle,
    lowered_always_on_top: bool,
}

impl SuppressHideGuard {
    fn acquire(app: &tauri::AppHandle) -> Self {
        if let Some(popover) = app.try_state::<crate::PopoverState>() {
            popover.suppress_hide.fetch_add(1, Ordering::AcqRel);
        }
        let lowered_always_on_top = app
            .get_webview_window(crate::MAIN_WINDOW_LABEL)
            .is_some_and(|window| {
                let was_on_top = window.is_always_on_top().unwrap_or(true);
                if was_on_top {
                    let _ = window.set_always_on_top(false);
                }
                was_on_top
            });
        Self {
            app: app.clone(),
            lowered_always_on_top,
        }
    }
}

impl Drop for SuppressHideGuard {
    fn drop(&mut self) {
        if let Some(window) = self.app.get_webview_window(crate::MAIN_WINDOW_LABEL) {
            if self.lowered_always_on_top {
                let _ = window.set_always_on_top(true);
            }
            let _ = window.show();
            let _ = window.set_focus();
        }
        if let Some(popover) = self.app.try_state::<crate::PopoverState>() {
            popover.suppress_hide.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

fn current_effective_roots(state: &AppState) -> Vec<String> {
    let (configured, initialized) = {
        let cfg = state.ai_config.lock().expect("ai_config lock poisoned");
        (cfg.dev_project_roots.clone(), cfg.dev_roots_initialized)
    };
    effective_roots(&configured, initialized, &state.rules)
}

fn update_dev_roots<F>(state: &AppState, mutate: F) -> Result<Vec<String>, AppError>
where
    F: FnOnce(&mut Vec<String>),
{
    let mut guard = state.ai_config.lock().expect("ai_config lock poisoned");
    let mut roots = effective_roots(
        &guard.dev_project_roots,
        guard.dev_roots_initialized,
        &state.rules,
    );
    mutate(&mut roots);
    let mut next = guard.clone();
    next.dev_project_roots = roots.clone();
    next.dev_roots_initialized = true;
    if let Some(ref cfg_path) = state.config_path {
        save_persisted_config(cfg_path, &next)?;
    }
    *guard = next;
    Ok(roots)
}

fn reconcile_size_maps(
    last_sizes: &mut HashMap<String, u64>,
    dev_last_sizes: &mut HashMap<String, u64>,
    removed_paths: &[String],
    fallback_freed_bytes: u64,
) {
    if removed_paths.is_empty() {
        return;
    }

    let mut matched_any_bytes = false;
    let mut per_path_removed: Vec<(PathBuf, u64)> = Vec::with_capacity(removed_paths.len());

    for removed in removed_paths {
        let removed_norm = normalize_path(Path::new(removed));
        let mut exact_bytes = 0_u64;

        for sizes in [&mut *last_sizes, &mut *dev_last_sizes] {
            let mut to_remove = Vec::new();
            for (key, &bytes) in sizes.iter() {
                let key_norm = normalize_path(Path::new(key));
                if paths_equal_platform(&key_norm, &removed_norm) {
                    exact_bytes = exact_bytes.max(bytes);
                    to_remove.push(key.clone());
                } else if path_starts_with_platform(&key_norm, &removed_norm) {
                    to_remove.push(key.clone());
                }
            }
            for key in to_remove {
                sizes.remove(&key);
            }
        }

        if exact_bytes > 0 {
            matched_any_bytes = true;
        }
        per_path_removed.push((removed_norm, exact_bytes));
    }

    if !matched_any_bytes && fallback_freed_bytes > 0 && per_path_removed.len() == 1 {
        per_path_removed[0].1 = fallback_freed_bytes;
    }

    for (removed_norm, removed_bytes) in per_path_removed {
        if removed_bytes == 0 {
            continue;
        }
        for sizes in [&mut *last_sizes, &mut *dev_last_sizes] {
            for (existing, bytes) in sizes.iter_mut() {
                let existing_norm = normalize_path(Path::new(existing));
                if path_starts_with_platform(&removed_norm, &existing_norm)
                    && !paths_equal_platform(&removed_norm, &existing_norm)
                {
                    *bytes = bytes.saturating_sub(removed_bytes);
                }
            }
        }
    }
}

fn reconcile_removed_paths(state: &AppState, removed_paths: &[String], fallback_freed_bytes: u64) {
    if removed_paths.is_empty() {
        return;
    }
    let mut last_sizes = state.last_sizes.lock().expect("last_sizes lock poisoned");
    let mut dev_last_sizes = state
        .dev_last_sizes
        .lock()
        .expect("dev_last_sizes lock poisoned");
    reconcile_size_maps(
        &mut last_sizes,
        &mut dev_last_sizes,
        removed_paths,
        fallback_freed_bytes,
    );
}

#[tauri::command]
pub fn get_dev_project_roots(state: State<'_, AppState>) -> Vec<String> {
    current_effective_roots(&state)
}

#[tauri::command]
pub async fn pick_dev_root_folder(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<String>, AppError> {
    let Some(_picker_guard) = AtomicFlagGuard::try_acquire(&state.folder_picker_open) else {
        return Ok(None);
    };
    let _suppress_guard = SuppressHideGuard::acquire(&app);
    let (tx, rx) = mpsc::channel();
    app.dialog().file().pick_folder(move |folder| {
        let _ = tx.send(folder);
    });

    let picked = tauri::async_runtime::spawn_blocking(move || rx.recv().ok().flatten())
        .await
        .map_err(|err| AppError::TaskJoin(err.to_string()))?;

    let Some(file_path) = picked else {
        return Ok(None);
    };

    let raw_path: PathBuf = file_path
        .into_path()
        .map_err(|err| AppError::DevTools(err.to_string()))?;

    let rules = Arc::clone(&state.rules);
    let canonical =
        tauri::async_runtime::spawn_blocking(move || validate_dev_root(&raw_path, &rules))
            .await
            .map_err(|err| AppError::TaskJoin(err.to_string()))?
            .map_err(AppError::DevTools)?;

    let canonical_str = canonical.to_string_lossy().to_string();
    let _ = update_dev_roots(&state, |roots| {
        if !roots
            .iter()
            .any(|existing| paths_equal_platform(Path::new(existing), &canonical))
        {
            roots.push(canonical_str.clone());
        }
    })?;
    Ok(Some(canonical_str))
}

#[tauri::command]
pub async fn add_dev_project_root(
    path: String,
    state: State<'_, AppState>,
) -> Result<Vec<String>, AppError> {
    let rules = Arc::clone(&state.rules);
    let candidate = PathBuf::from(path.trim());
    let canonical =
        tauri::async_runtime::spawn_blocking(move || validate_dev_root(&candidate, &rules))
            .await
            .map_err(|err| AppError::TaskJoin(err.to_string()))?
            .map_err(AppError::DevTools)?;

    update_dev_roots(&state, |roots| {
        if !roots
            .iter()
            .any(|existing| paths_equal_platform(Path::new(existing), &canonical))
        {
            roots.push(canonical.to_string_lossy().to_string());
        }
    })
}

#[tauri::command]
pub fn remove_dev_project_root(
    path: String,
    state: State<'_, AppState>,
) -> Result<Vec<String>, AppError> {
    let target = normalize_path(Path::new(path.trim()));
    update_dev_roots(&state, |roots| {
        roots.retain(|entry| !paths_equal_platform(&normalize_path(Path::new(entry)), &target));
    })
}

#[tauri::command]
pub async fn scan_dev_projects(state: State<'_, AppState>) -> Result<DevProjectsReport, AppError> {
    let Some(_guard) = AtomicFlagGuard::try_acquire(&state.dev_scan_in_progress) else {
        return Err(AppError::ScanInProgress);
    };

    let roots = current_effective_roots(&state);
    let ignored_paths = state
        .ai_config
        .lock()
        .expect("ai_config lock poisoned")
        .ignored_paths
        .clone();
    let rules = Arc::clone(&state.rules);

    let report = tauri::async_runtime::spawn_blocking(move || {
        run_dev_projects_scan(&roots, &ignored_paths, &rules)
    })
    .await
    .map_err(|err| AppError::TaskJoin(err.to_string()))?;

    {
        let mut sizes = state
            .dev_last_sizes
            .lock()
            .expect("dev_last_sizes lock poisoned");
        for project in &report.projects {
            for artifact in &project.artifacts {
                sizes.insert(artifact.path.clone(), artifact.size_bytes);
            }
        }
    }

    Ok(report)
}

#[tauri::command]
pub async fn trash_dev_project_artifacts(
    paths: Vec<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<DevTrashResult, AppError> {
    let roots = current_effective_roots(&state);
    let rules = Arc::clone(&state.rules);
    let known_sizes = state
        .dev_last_sizes
        .lock()
        .expect("dev_last_sizes lock poisoned")
        .clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
        trash_dev_artifacts(&paths, &roots, &known_sizes, &rules)
    })
    .await
    .map_err(|err| AppError::TaskJoin(err.to_string()))?;

    if !result.trashed_paths.is_empty() {
        reconcile_removed_paths(&state, &result.trashed_paths, result.freed_bytes);
        crate::sync_tray_disk_status(&app);
    }

    Ok(result)
}

#[tauri::command]
pub async fn inspect_xcode(state: State<'_, AppState>) -> Result<XcodeReport, AppError> {
    let report = tauri::async_runtime::spawn_blocking(run_xcode_inspect)
        .await
        .map_err(|err| AppError::TaskJoin(err.to_string()))?;

    {
        let mut sizes = state
            .dev_last_sizes
            .lock()
            .expect("dev_last_sizes lock poisoned");
        for area in &report.areas {
            sizes.insert(area.path.clone(), area.size_bytes);
        }
        sizes.insert(
            XCODE_UNAVAILABLE_SIZE_KEY.to_string(),
            report.unavailable_total_bytes,
        );
        for rt in &report.runtimes {
            sizes.insert(
                format!("{XCODE_RUNTIME_SIZE_PREFIX}{}", rt.identifier),
                rt.size_bytes,
            );
        }
    }

    Ok(report)
}

#[tauri::command]
pub async fn trash_xcode_items(
    paths: Vec<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<XcodeActionResult, AppError> {
    let rules = Arc::clone(&state.rules);
    let known_sizes = state
        .dev_last_sizes
        .lock()
        .expect("dev_last_sizes lock poisoned")
        .clone();

    let paths_for_reconcile = paths.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        trash_xcode_paths(&paths, &known_sizes, &rules)
    })
    .await
    .map_err(|err| AppError::TaskJoin(err.to_string()))?;

    if result.deleted_count > 0 {
        let failed_set: std::collections::HashSet<&str> =
            result.failed.iter().map(|f| f.path.as_str()).collect();
        let succeeded: Vec<String> = paths_for_reconcile
            .into_iter()
            .filter(|p| !failed_set.contains(p.as_str()))
            .collect();
        reconcile_removed_paths(&state, &succeeded, result.freed_bytes);
        crate::sync_tray_disk_status(&app);
    }

    Ok(result)
}

#[tauri::command]
pub async fn delete_unavailable_simulators(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<XcodeActionResult, AppError> {
    let deleted_count = tauri::async_runtime::spawn_blocking(execute_delete_unavailable_simulators)
        .await
        .map_err(|err| AppError::TaskJoin(err.to_string()))?
        .map_err(AppError::DevTools)?;

    let known_bytes = state
        .dev_last_sizes
        .lock()
        .expect("dev_last_sizes lock poisoned")
        .remove(XCODE_UNAVAILABLE_SIZE_KEY)
        .unwrap_or(0);

    crate::sync_tray_disk_status(&app);
    Ok(XcodeActionResult {
        freed_bytes: if deleted_count > 0 { known_bytes } else { 0 },
        deleted_count,
        failed: Vec::new(),
    })
}

#[tauri::command]
pub async fn delete_simulator_runtime(
    identifier: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<XcodeActionResult, AppError> {
    let id_clone = identifier.clone();
    tauri::async_runtime::spawn_blocking(move || execute_delete_simulator_runtime(&id_clone))
        .await
        .map_err(|err| AppError::TaskJoin(err.to_string()))?
        .map_err(AppError::DevTools)?;

    let known_bytes = state
        .dev_last_sizes
        .lock()
        .expect("dev_last_sizes lock poisoned")
        .remove(&format!("{XCODE_RUNTIME_SIZE_PREFIX}{identifier}"))
        .unwrap_or(0);

    crate::sync_tray_disk_status(&app);
    Ok(XcodeActionResult {
        freed_bytes: known_bytes,
        deleted_count: 1,
        failed: Vec::new(),
    })
}

#[tauri::command]
pub async fn inspect_docker_containers() -> Result<DockerReport, AppError> {
    tauri::async_runtime::spawn_blocking(inspect_containers)
        .await
        .map_err(|err| AppError::TaskJoin(err.to_string()))
}

#[tauri::command]
pub async fn prune_docker_scope(
    scope: DockerPruneScope,
    app: tauri::AppHandle,
) -> Result<DockerPruneResult, AppError> {
    let result = tauri::async_runtime::spawn_blocking(move || run_docker_prune(scope))
        .await
        .map_err(|err| AppError::TaskJoin(err.to_string()))?
        .map_err(AppError::DevTools)?;

    crate::sync_tray_disk_status(&app);
    Ok(result)
}

#[tauri::command]
pub async fn inspect_dev_toolchains(
    state: State<'_, AppState>,
) -> Result<ToolchainsReport, AppError> {
    let report = tauri::async_runtime::spawn_blocking(run_toolchains_inspect)
        .await
        .map_err(|err| AppError::TaskJoin(err.to_string()))?;

    {
        let mut sizes = state
            .dev_last_sizes
            .lock()
            .expect("dev_last_sizes lock poisoned");
        for item in &report.caches {
            sizes.insert(item.path.clone(), item.size_bytes);
        }
    }

    Ok(report)
}

#[tauri::command]
pub async fn trash_dev_toolchain_cache(
    path: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<u64, AppError> {
    let rules = Arc::clone(&state.rules);
    let known_size = state
        .dev_last_sizes
        .lock()
        .expect("dev_last_sizes lock poisoned")
        .get(&path)
        .copied();
    let path_for_reconcile = path.clone();

    let freed = tauri::async_runtime::spawn_blocking(move || {
        execute_trash_toolchain_cache(&path, known_size, &rules)
    })
    .await
    .map_err(|err| AppError::TaskJoin(err.to_string()))?
    .map_err(AppError::DevTools)?;

    reconcile_removed_paths(&state, &[path_for_reconcile], freed);
    crate::sync_tray_disk_status(&app);
    Ok(freed)
}

#[tauri::command]
pub async fn run_homebrew_clean(
    action: BrewCleanAction,
    app: tauri::AppHandle,
) -> Result<BrewCleanResult, AppError> {
    let result = tauri::async_runtime::spawn_blocking(move || execute_brew_clean(action))
        .await
        .map_err(|err| AppError::TaskJoin(err.to_string()))?
        .map_err(AppError::DevTools)?;

    crate::sync_tray_disk_status(&app);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconcile_size_maps_updates_ancestors_across_maps_and_clears_descendants() {
        let mut last_sizes = HashMap::from([
            (
                "/Users/dev/Library/Developer/Xcode/DerivedData".to_string(),
                5_000_000,
            ),
            (
                "/Users/dev/Library/Developer/Xcode/DerivedData/MyApp/Build".to_string(),
                1_500_000,
            ),
        ]);
        let mut dev_last_sizes = HashMap::from([(
            "/Users/dev/Library/Developer/Xcode/DerivedData/MyApp".to_string(),
            2_000_000,
        )]);

        reconcile_size_maps(
            &mut last_sizes,
            &mut dev_last_sizes,
            &["/Users/dev/Library/Developer/Xcode/DerivedData/MyApp".to_string()],
            0,
        );

        assert!(
            !dev_last_sizes.contains_key("/Users/dev/Library/Developer/Xcode/DerivedData/MyApp")
        );
        assert!(
            !last_sizes.contains_key("/Users/dev/Library/Developer/Xcode/DerivedData/MyApp/Build")
        );
        assert_eq!(
            last_sizes.get("/Users/dev/Library/Developer/Xcode/DerivedData"),
            Some(&3_000_000)
        );
    }

    #[test]
    fn reconcile_size_maps_falls_back_to_known_freed_bytes_when_absent_from_maps() {
        let mut last_sizes = HashMap::from([("/Users/dev/.npm".to_string(), 10_000_000)]);
        let mut dev_last_sizes = HashMap::new();

        reconcile_size_maps(
            &mut last_sizes,
            &mut dev_last_sizes,
            &["/Users/dev/.npm/_npx".to_string()],
            4_000_000,
        );

        assert_eq!(last_sizes.get("/Users/dev/.npm"), Some(&6_000_000));
    }

    #[test]
    fn atomic_flag_guard_enforces_single_flight() {
        let flag = Arc::new(AtomicBool::new(false));
        let first = AtomicFlagGuard::try_acquire(&flag).expect("acquires");
        assert!(AtomicFlagGuard::try_acquire(&flag).is_none());
        drop(first);
        assert!(AtomicFlagGuard::try_acquire(&flag).is_some());
    }
}
