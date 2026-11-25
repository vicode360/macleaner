//! Tauri IPC command handlers exposed to the frontend.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::State;
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_updater::UpdaterExt;
use thiserror::Error;

use crate::ai::{
    normalize_ignored_paths, validate_config, AiCacheKey, AiClassification, AiClassificationCache,
    AiClient, AiConfig, AiError, AiExtensionEvidence, AiMetadataInput, AiProvider, AiStatus,
    AiSubfolderEvidence, AppLanguage,
};
use crate::rules::{
    classify_subpath, normalize_path, path_starts_with_platform, paths_equal_platform, RiskLevel,
    RuleEngine, RuleError,
};
use crate::scanner::{
    compute_cleanable_bytes, move_items_to_trash, read_disk_health, sanitize_relative_path,
    scan_with_engine_and_ignored, summarize_scan, validate_cleanable_target, DiskHealth, ScanEvent,
    ScanItem, ScanSummary, TrashResult,
};
use crate::snapshots::{
    delete_local_snapshots, list_local_snapshots, SnapshotDeleteResult, SnapshotListResult,
};

const MAX_AI_ITEMS_PER_SCAN: usize = 12;
static CONFIG_TMP_SEQ: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "event",
    content = "data"
)]
pub enum UpdateInstallEvent {
    Checking,
    Downloading {
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    },
    Installing,
    Restarting,
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Rules(#[from] RuleError),
    #[error(transparent)]
    Ai(#[from] AiError),
    #[error("background task failed: {0}")]
    TaskJoin(String),
    #[error("HTTP client initialization failed: {0}")]
    HttpInit(String),
    #[error("failed to persist settings: {0}")]
    SettingsIo(String),
    #[error("update failed: {0}")]
    Update(String),
    #[error("scan already in progress")]
    ScanInProgress,
    #[error("{0}")]
    InvalidRevealPath(String),
    #[error("failed to reveal item in Finder: {0}")]
    RevealFailed(String),
    #[error("failed to manage APFS snapshots: {0}")]
    SnapshotFailed(String),
    #[error("failed to update launch at login: {0}")]
    Autostart(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

struct ScanGuard {
    flag: Arc<AtomicBool>,
}

impl ScanGuard {
    fn try_acquire(flag: &Arc<AtomicBool>) -> Option<Self> {
        flag.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| Self {
                flag: Arc::clone(flag),
            })
    }
}

impl Drop for ScanGuard {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::Release);
    }
}

pub struct AppState {
    pub(crate) rules: Arc<RuleEngine>,
    pub(crate) ai_client: AiClient,
    pub(crate) ai_config: Mutex<AiConfig>,
    pub(crate) config_path: Option<PathBuf>,
    pub(crate) ai_cache: Mutex<AiClassificationCache>,
    pub(crate) ai_cache_path: Option<PathBuf>,
    pub(crate) last_sizes: Mutex<HashMap<String, u64>>,
    pub(crate) scan_in_progress: Arc<AtomicBool>,
    pub(crate) low_disk_notified: AtomicBool,
}

impl AppState {
    /// Initializes shared backend state with embedded rules, persisted settings, and AI client.
    ///
    /// # Errors
    /// Returns [`AppError`] if home directory resolution, rule parsing, or HTTP client setup fails.
    pub fn new(config_path: Option<PathBuf>) -> Result<Self, AppError> {
        let rules = Arc::new(RuleEngine::load_default()?);
        let ai_client = AiClient::new().map_err(|err| AppError::HttpInit(err.to_string()))?;
        let initial_config = config_path
            .as_deref()
            .and_then(load_persisted_config)
            .unwrap_or_default();
        let ai_cache_path = config_path
            .as_deref()
            .and_then(Path::parent)
            .map(|dir| dir.join("ai_cache.json"));
        let initial_cache = ai_cache_path
            .as_deref()
            .map(AiClassificationCache::load)
            .unwrap_or_default();

        Ok(Self {
            rules,
            ai_client,
            ai_config: Mutex::new(initial_config),
            config_path,
            ai_cache: Mutex::new(initial_cache),
            ai_cache_path,
            last_sizes: Mutex::new(HashMap::new()),
            scan_in_progress: Arc::new(AtomicBool::new(false)),
            low_disk_notified: AtomicBool::new(false),
        })
    }
}

#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
pub async fn check_for_updates(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<UpdateCheckResult, AppError> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let language = state
        .ai_config
        .lock()
        .expect("ai_config lock poisoned")
        .language;

    let updater = match app.updater() {
        Ok(updater) => updater,
        Err(err) => {
            return Ok(UpdateCheckResult {
                current_version: current.clone(),
                latest_version: None,
                update_available: false,
                message: format_update_check_error(language, &current, &err.to_string()),
            });
        }
    };

    match updater.check().await {
        Ok(Some(update)) => {
            let latest = update
                .version
                .trim()
                .trim_start_matches(['v', 'V'])
                .to_string();
            let message = format_update_available(language, &latest, &current);
            Ok(UpdateCheckResult {
                current_version: current,
                latest_version: Some(latest),
                update_available: true,
                message,
            })
        }
        Ok(None) => Ok(UpdateCheckResult {
            current_version: current.clone(),
            latest_version: Some(current.clone()),
            update_available: false,
            message: format_update_up_to_date(language, &current),
        }),
        Err(err) => Ok(UpdateCheckResult {
            current_version: current.clone(),
            latest_version: None,
            update_available: false,
            message: format_update_check_error(language, &current, &err.to_string()),
        }),
    }
}

#[tauri::command]
pub async fn install_update(
    app: tauri::AppHandle,
    on_event: Channel<UpdateInstallEvent>,
) -> Result<(), AppError> {
    let _ = on_event.send(UpdateInstallEvent::Checking);

    let updater = app
        .updater()
        .map_err(|err| AppError::Update(err.to_string()))?;
    let update = updater
        .check()
        .await
        .map_err(|err| AppError::Update(err.to_string()))?
        .ok_or_else(|| AppError::Update("Nenhuma atualização disponível.".to_string()))?;

    let _ = on_event.send(UpdateInstallEvent::Downloading {
        downloaded_bytes: 0,
        total_bytes: None,
    });

    let mut downloaded_bytes = 0_u64;
    let mut last_reported = 0_u64;
    let channel_dl = on_event.clone();
    let channel_inst = on_event.clone();

    update
        .download_and_install(
            move |chunk_len, total_bytes| {
                downloaded_bytes = downloaded_bytes.saturating_add(chunk_len as u64);
                let reached_end = total_bytes.is_some_and(|total| downloaded_bytes >= total);
                if reached_end || downloaded_bytes.saturating_sub(last_reported) >= 64 * 1024 {
                    last_reported = downloaded_bytes;
                    let _ = channel_dl.send(UpdateInstallEvent::Downloading {
                        downloaded_bytes,
                        total_bytes,
                    });
                }
            },
            move || {
                let _ = channel_inst.send(UpdateInstallEvent::Installing);
            },
        )
        .await
        .map_err(|err| AppError::Update(err.to_string()))?;

    let _ = on_event.send(UpdateInstallEvent::Restarting);
    app.restart()
}

#[tauri::command]
pub fn get_disk_health(app: tauri::AppHandle) -> DiskHealth {
    crate::sync_tray_disk_status(&app);
    read_disk_health()
}

#[tauri::command]
pub async fn reveal_in_finder(path: String, state: State<'_, AppState>) -> Result<(), AppError> {
    let rules = Arc::clone(&state.rules);
    tauri::async_runtime::spawn_blocking(move || {
        let canonical = validate_cleanable_target(&rules, &path)
            .map_err(|msg| AppError::InvalidRevealPath(msg.to_string()))?;
        let status = Command::new("/usr/bin/open")
            .arg("-R")
            .arg(&canonical)
            .status()
            .map_err(|err| AppError::RevealFailed(err.to_string()))?;
        if !status.success() {
            return Err(AppError::RevealFailed(format!(
                "open exited with status {status}"
            )));
        }
        Ok(())
    })
    .await
    .map_err(|err| AppError::TaskJoin(err.to_string()))?
}

#[tauri::command]
pub async fn list_apfs_snapshots() -> Result<SnapshotListResult, AppError> {
    tauri::async_runtime::spawn_blocking(list_local_snapshots)
        .await
        .map_err(|err| AppError::TaskJoin(err.to_string()))?
        .map_err(AppError::SnapshotFailed)
}

#[tauri::command]
pub async fn delete_apfs_snapshots(
    date_tokens: Vec<String>,
    app: tauri::AppHandle,
) -> Result<SnapshotDeleteResult, AppError> {
    let result = tauri::async_runtime::spawn_blocking(move || delete_local_snapshots(&date_tokens))
        .await
        .map_err(|err| AppError::TaskJoin(err.to_string()))?
        .map_err(AppError::SnapshotFailed)?;
    if !result.deleted_tokens.is_empty() {
        crate::sync_tray_disk_status(&app);
    }
    Ok(result)
}

#[tauri::command]
pub async fn scan_system(
    min_mb: Option<u64>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    on_event: Channel<ScanEvent>,
) -> Result<ScanSummary, AppError> {
    let _scan_guard =
        ScanGuard::try_acquire(&state.scan_in_progress).ok_or(AppError::ScanInProgress)?;

    let mut ai_config = state
        .ai_config
        .lock()
        .expect("ai_config lock poisoned")
        .clone();
    let threshold_mb = min_mb.unwrap_or(ai_config.min_mb);
    let ignored_paths: Vec<PathBuf> = ai_config
        .ignored_paths
        .iter()
        .map(|raw| normalize_path(Path::new(raw)))
        .collect();

    let rules = Arc::clone(&state.rules);
    let event_channel = on_event.clone();

    let mut raw = tauri::async_runtime::spawn_blocking(move || {
        scan_with_engine_and_ignored(&rules, &ignored_paths, threshold_mb, &|event| {
            let _ = event_channel.send(event);
        })
    })
    .await
    .map_err(|err| AppError::TaskJoin(err.to_string()))?;
    let mut ai_available = false;
    let mut uncached_unknown_count = 0_usize;
    let cache_model = configured_model_key(&ai_config);

    if ai_config.provider != AiProvider::Disabled {
        let cached_hits: Vec<(usize, AiClassification)> = {
            let cache = state.ai_cache.lock().expect("ai_cache lock poisoned");
            raw.items
                .iter()
                .enumerate()
                .filter(|(_, item)| item.rule_id.is_none())
                .filter_map(|(idx, item)| {
                    cache
                        .get(AiCacheKey {
                            folder_path: &item.path,
                            language: ai_config.language,
                            provider: ai_config.provider,
                            model: &cache_model,
                            size_bytes: item.size_bytes,
                            file_count: item.file_count,
                        })
                        .map(|hit| (idx, hit))
                })
                .collect()
        };

        if !cached_hits.is_empty() {
            ai_available = true;
            for (idx, classification) in cached_hits {
                apply_classification(&mut raw.items[idx], classification);
            }
        }

        uncached_unknown_count = raw
            .items
            .iter()
            .filter(|item| item.rule_id.is_none() && !item.ai_classified)
            .count();
    }

    if uncached_unknown_count > 0 && ai_config.provider != AiProvider::Disabled {
        let ai_status = state.ai_client.check_availability(&ai_config).await;
        ai_available = ai_available || ai_status.available;

        if ai_status.available {
            if ai_config.provider == AiProvider::Ollama
                && ai_config.ollama_model != "auto"
                && ai_config.ollama_model != ai_status.model
            {
                ai_config.ollama_model = ai_status.model.clone();
                let fresh_snapshot = {
                    let mut guard = state.ai_config.lock().expect("ai_config lock poisoned");
                    if guard.provider == AiProvider::Ollama && guard.ollama_model != "auto" {
                        guard.ollama_model = ai_status.model.clone();
                        Some(guard.clone())
                    } else {
                        None
                    }
                };
                if let (Some(path), Some(snapshot)) = (state.config_path.as_deref(), fresh_snapshot)
                {
                    let _ = save_persisted_config(path, &snapshot);
                }
            }

            let effective_model = configured_model_key(&ai_config);
            let scanned_bytes = raw.items.iter().map(|item| item.size_bytes).sum();
            let batch_count = uncached_unknown_count.min(MAX_AI_ITEMS_PER_SCAN);
            let _ = on_event.send(ScanEvent::Progress {
                current_path: format_ai_progress(ai_config.language, &ai_status.model, batch_count),
                scanned_bytes,
            });

            let mut analyzed = 0_usize;
            let mut cache_dirty = false;
            for item in &mut raw.items {
                if item.rule_id.is_some() || item.ai_classified {
                    continue;
                }
                if analyzed < MAX_AI_ITEMS_PER_SCAN {
                    if let Some(classification) = query_ai_with_evidence(
                        &state.ai_client,
                        &state.rules,
                        &ai_config,
                        Some(&ai_status.model),
                        item,
                    )
                    .await
                    {
                        {
                            let mut cache = state.ai_cache.lock().expect("ai_cache lock poisoned");
                            cache.insert(
                                AiCacheKey {
                                    folder_path: &item.path,
                                    language: ai_config.language,
                                    provider: ai_config.provider,
                                    model: &effective_model,
                                    size_bytes: item.size_bytes,
                                    file_count: item.file_count,
                                },
                                classification.clone(),
                            );
                        }
                        apply_classification(item, classification);
                        cache_dirty = true;
                    }
                    analyzed += 1;
                }
            }

            if cache_dirty {
                if let Some(ref cache_path) = state.ai_cache_path {
                    let snapshot = state
                        .ai_cache
                        .lock()
                        .expect("ai_cache lock poisoned")
                        .clone();
                    let _ = snapshot.save(cache_path);
                }
            }

            if ai_config.provider == AiProvider::Ollama {
                state
                    .ai_client
                    .unload_ollama_model(&ai_config.ollama_endpoint, &ai_status.model)
                    .await;
            }
        }
    }

    {
        let mut sizes = state.last_sizes.lock().expect("last_sizes lock poisoned");
        sizes.clear();
        for item in &raw.items {
            sizes.insert(item.path.clone(), item.size_bytes);
            for sub in &item.subitems {
                sizes.insert(sub.path.clone(), sub.size_bytes);
            }
        }
    }
    let summary = summarize_scan(
        raw.items,
        raw.skipped_permission_count,
        raw.started_at,
        ai_available,
    );
    let _ = on_event.send(ScanEvent::Finished {
        summary: summary.clone(),
    });
    crate::sync_tray_disk_status(&app);

    Ok(summary)
}

#[tauri::command]
pub async fn trash_items(
    paths: Vec<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<TrashResult, AppError> {
    let rules = Arc::clone(&state.rules);
    let known_sizes = state
        .last_sizes
        .lock()
        .expect("last_sizes lock poisoned")
        .clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
        move_items_to_trash(&rules, &paths, &known_sizes)
    })
    .await
    .map_err(|err| AppError::TaskJoin(err.to_string()))?;

    if !result.trashed_paths.is_empty() {
        {
            let mut sizes = state.last_sizes.lock().expect("last_sizes lock poisoned");
            for removed in &result.trashed_paths {
                let removed_bytes = sizes.remove(removed).unwrap_or(0);
                if removed_bytes > 0 {
                    let removed_path = Path::new(removed);
                    for (existing, bytes) in sizes.iter_mut() {
                        let existing_path = Path::new(existing);
                        if path_starts_with_platform(removed_path, existing_path)
                            && !paths_equal_platform(removed_path, existing_path)
                        {
                            *bytes = bytes.saturating_sub(removed_bytes);
                        }
                    }
                }
            }
        }
        let cache_snapshot = {
            let mut cache = state.ai_cache.lock().expect("ai_cache lock poisoned");
            for removed in &result.trashed_paths {
                cache.remove_path(removed);
            }
            cache.clone()
        };
        if let Some(ref cache_path) = state.ai_cache_path {
            let _ = cache_snapshot.save(cache_path);
        }
        crate::sync_tray_disk_status(&app);
    }

    Ok(result)
}

#[tauri::command]
pub async fn check_ai_status(state: State<'_, AppState>) -> Result<AiStatus, AppError> {
    let ai_config = state
        .ai_config
        .lock()
        .expect("ai_config lock poisoned")
        .clone();
    Ok(state.ai_client.check_availability(&ai_config).await)
}

#[tauri::command]
pub async fn list_ollama_models(state: State<'_, AppState>) -> Result<Vec<String>, AppError> {
    let endpoint = state
        .ai_config
        .lock()
        .expect("ai_config lock poisoned")
        .ollama_endpoint
        .clone();
    Ok(state.ai_client.list_ollama_models(&endpoint).await?)
}

#[tauri::command]
pub fn get_ai_config(state: State<'_, AppState>) -> Result<AiConfig, AppError> {
    let config = state
        .ai_config
        .lock()
        .expect("ai_config lock poisoned")
        .clone();
    Ok(config)
}

#[tauri::command]
pub fn set_ai_config(
    mut config: AiConfig,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<AiConfig, AppError> {
    config.ignored_paths = normalize_ignored_paths(&config.ignored_paths);
    validate_config(&config)?;
    if let Some(ref path) = state.config_path {
        save_persisted_config(path, &config)?;
    }
    {
        let mut guard = state.ai_config.lock().expect("ai_config lock poisoned");
        *guard = config.clone();
    }
    crate::sync_tray_disk_status(&app);
    Ok(config)
}

#[tauri::command]
pub fn add_ignored_path(path: String, state: State<'_, AppState>) -> Result<AiConfig, AppError> {
    let trimmed = path.trim();
    if trimmed.is_empty() || !Path::new(trimmed).is_absolute() {
        return Err(AppError::InvalidRevealPath(
            "Caminho inválido para ignorar.".to_string(),
        ));
    }

    let updated = {
        let mut guard = state.ai_config.lock().expect("ai_config lock poisoned");
        let mut next = guard.clone();
        next.ignored_paths.push(trimmed.to_string());
        next.ignored_paths = normalize_ignored_paths(&next.ignored_paths);
        if let Some(ref cfg_path) = state.config_path {
            save_persisted_config(cfg_path, &next)?;
        }
        *guard = next.clone();
        next
    };

    Ok(updated)
}

#[tauri::command]
pub fn remove_ignored_path(path: String, state: State<'_, AppState>) -> Result<AiConfig, AppError> {
    let target = normalize_path(Path::new(path.trim()));
    let updated = {
        let mut guard = state.ai_config.lock().expect("ai_config lock poisoned");
        let mut next = guard.clone();
        next.ignored_paths
            .retain(|entry| !paths_equal_platform(Path::new(entry), &target));
        next.ignored_paths = normalize_ignored_paths(&next.ignored_paths);
        if let Some(ref cfg_path) = state.config_path {
            save_persisted_config(cfg_path, &next)?;
        }
        *guard = next.clone();
        next
    };

    Ok(updated)
}

#[tauri::command]
pub fn get_autostart_enabled(app: tauri::AppHandle) -> Result<bool, AppError> {
    app.autolaunch()
        .is_enabled()
        .map_err(|err| AppError::Autostart(err.to_string()))
}

#[tauri::command]
pub fn set_autostart_enabled(enabled: bool, app: tauri::AppHandle) -> Result<bool, AppError> {
    let manager = app.autolaunch();
    if enabled {
        manager
            .enable()
            .map_err(|err| AppError::Autostart(err.to_string()))?;
    } else {
        manager
            .disable()
            .map_err(|err| AppError::Autostart(err.to_string()))?;
    }
    manager
        .is_enabled()
        .map_err(|err| AppError::Autostart(err.to_string()))
}

async fn query_ai_with_evidence(
    ai_client: &AiClient,
    rules: &RuleEngine,
    config: &AiConfig,
    resolved_model: Option<&str>,
    item: &ScanItem,
) -> Option<AiClassification> {
    let relative = sanitize_relative_path(Path::new(&item.path), rules.home_dir());
    let subfolder_evidence: Vec<AiSubfolderEvidence<'_>> = item
        .subitems
        .iter()
        .map(|sub| AiSubfolderEvidence {
            name: &sub.name,
            size_bytes: sub.size_bytes,
            file_count: sub.file_count,
            days_since_modified: sub.days_since_modified,
            known_risk: classify_subpath(&sub.name).map(|(risk, _, _)| risk),
        })
        .collect();
    let extension_evidence: Vec<AiExtensionEvidence<'_>> = item
        .top_extensions
        .iter()
        .map(|ext| AiExtensionEvidence {
            ext: &ext.ext,
            size_bytes: ext.size_bytes,
            file_count: ext.file_count,
        })
        .collect();

    ai_client
        .classify_metadata(
            config,
            resolved_model,
            AiMetadataInput {
                name: &item.name,
                relative_path: &relative,
                category: &item.category,
                size_bytes: item.size_bytes,
                file_count: item.file_count,
                days_since_modified: item.days_since_modified,
                language: config.language,
                subfolders: &subfolder_evidence,
                extensions: &extension_evidence,
            },
        )
        .await
}

fn configured_model_key(config: &AiConfig) -> String {
    match config.provider {
        AiProvider::Ollama => config.ollama_model.trim().to_string(),
        AiProvider::ExternalApi => config.external_model.trim().to_string(),
        AiProvider::Disabled => String::new(),
    }
}

fn apply_classification(item: &mut ScanItem, classification: AiClassification) {
    item.risk = classification.risk;
    item.reason = classification.reason;
    item.ai_classified = true;

    for verdict in classification.subfolders {
        if let Some(sub) = item
            .subitems
            .iter_mut()
            .find(|s| s.name.eq_ignore_ascii_case(&verdict.name))
        {
            let locked_high =
                classify_subpath(&sub.name).is_some_and(|(r, _, _)| matches!(r, RiskLevel::Alto));
            if locked_high {
                continue;
            }
            sub.risk = verdict.risk;
            sub.reason = verdict.reason;
            sub.safe_to_clean = verdict.safe_to_clean && matches!(verdict.risk, RiskLevel::Baixo);
        }
    }

    let has_high_sub = item
        .subitems
        .iter()
        .any(|sub| matches!(sub.risk, RiskLevel::Alto));
    if has_high_sub {
        if matches!(item.risk, RiskLevel::Baixo) {
            item.risk = RiskLevel::Alto;
        }
        item.selected = false;
    } else {
        item.selected = matches!(item.risk, RiskLevel::Baixo);
    }

    item.cleanable_bytes = compute_cleanable_bytes(item.risk, item.size_bytes, &item.subitems);
}

fn format_ai_progress(language: AppLanguage, model: &str, count: usize) -> String {
    match language {
        AppLanguage::PtBr => format!("IA ({model}) analisando {count} pastas desconhecidas…"),
        AppLanguage::En => format!("AI ({model}) analyzing {count} unknown folders…"),
        AppLanguage::Es => format!("IA ({model}) analizando {count} carpetas desconocidas…"),
    }
}

fn format_update_check_error(language: AppLanguage, current: &str, detail: &str) -> String {
    match language {
        AppLanguage::PtBr => {
            format!("Não foi possível verificar atualizações (v{current}): {detail}")
        }
        AppLanguage::En => {
            format!("Could not check for updates (v{current}): {detail}")
        }
        AppLanguage::Es => {
            format!("No se pudo comprobar actualizaciones (v{current}): {detail}")
        }
    }
}

fn format_update_available(language: AppLanguage, latest: &str, current: &str) -> String {
    match language {
        AppLanguage::PtBr => format!("Nova versão v{latest} disponível (atual: v{current})."),
        AppLanguage::En => format!("New version v{latest} available (current: v{current})."),
        AppLanguage::Es => format!("Nueva versión v{latest} disponible (actual: v{current})."),
    }
}

fn format_update_up_to_date(language: AppLanguage, current: &str) -> String {
    match language {
        AppLanguage::PtBr => format!("Você já está na versão mais recente (v{current})."),
        AppLanguage::En => format!("You are on the latest version (v{current})."),
        AppLanguage::Es => format!("Ya tienes la versión más reciente (v{current})."),
    }
}

fn load_persisted_config(path: &Path) -> Option<AiConfig> {
    let raw = fs::read(path).ok()?;
    let mut parsed: AiConfig = serde_json::from_slice(&raw).ok()?;
    if parsed.timeout_ms < 15_000 {
        parsed.timeout_ms = 20_000;
    }
    parsed.ignored_paths = normalize_ignored_paths(&parsed.ignored_paths);
    validate_config(&parsed).ok()?;
    Some(parsed)
}

fn save_persisted_config(path: &Path, config: &AiConfig) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| AppError::SettingsIo(err.to_string()))?;
    }
    let payload =
        serde_json::to_vec_pretty(config).map_err(|err| AppError::SettingsIo(err.to_string()))?;
    let seq = CONFIG_TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp_name = format!(
        "{}.{}.{seq}.tmp",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("settings.json"),
        std::process::id()
    );
    let tmp_path = path.with_file_name(tmp_name);
    fs::write(&tmp_path, payload).map_err(|err| AppError::SettingsIo(err.to_string()))?;
    fs::rename(&tmp_path, path).map_err(|err| AppError::SettingsIo(err.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_install_event_serializes_expected_variants() {
        let evt = UpdateInstallEvent::Downloading {
            downloaded_bytes: 512,
            total_bytes: Some(1024),
        };
        let json = serde_json::to_string(&evt).expect("serializes");
        assert!(json.contains("\"event\":\"downloading\""));
        assert!(json.contains("\"downloadedBytes\":512"));
        assert!(json.contains("\"totalBytes\":1024"));
    }

    #[test]
    fn scan_guard_prevents_concurrent_scans_and_releases_on_drop() {
        let flag = Arc::new(AtomicBool::new(false));
        let first = ScanGuard::try_acquire(&flag).expect("first acquires");
        assert!(ScanGuard::try_acquire(&flag).is_none());
        drop(first);
        assert!(ScanGuard::try_acquire(&flag).is_some());
    }

    #[test]
    fn apply_classification_never_selects_parent_with_locked_high_subitem() {
        let mut item = ScanItem {
            id: "unclassified:/Users/tester/Library/Application Support/Steam".to_string(),
            name: "Steam".to_string(),
            path: "/Users/tester/Library/Application Support/Steam".to_string(),
            category: "apps".to_string(),
            size_bytes: 10_000_000,
            file_count: 100,
            days_since_modified: 1,
            risk: RiskLevel::Alto,
            reason: String::new(),
            rule_id: None,
            ai_classified: false,
            selected: false,
            subitems: vec![crate::scanner::ScanSubItem {
                name: "steamapps/common".to_string(),
                path: "/Users/tester/Library/Application Support/Steam/steamapps/common"
                    .to_string(),
                size_bytes: 9_000_000,
                file_count: 80,
                days_since_modified: 1,
                is_dir: true,
                risk: RiskLevel::Alto,
                reason: "Jogos instalados.".to_string(),
                safe_to_clean: false,
            }],
            cleanable_bytes: 0,
            top_extensions: vec![],
        };

        apply_classification(
            &mut item,
            AiClassification {
                risk: RiskLevel::Baixo,
                reason: "Hallucinated safe".to_string(),
                subfolders: vec![crate::ai::AiSubfolderVerdict {
                    name: "steamapps/common".to_string(),
                    risk: RiskLevel::Baixo,
                    reason: "Hallucinated".to_string(),
                    safe_to_clean: true,
                }],
            },
        );

        assert_eq!(item.risk, RiskLevel::Alto);
        assert!(!item.selected);
        assert_eq!(item.subitems[0].risk, RiskLevel::Alto);
        assert!(!item.subitems[0].safe_to_clean);
    }
}
