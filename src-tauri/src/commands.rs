//! Tauri IPC command handlers exposed to the frontend.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::State;
use thiserror::Error;

use crate::ai::{
    validate_config, AiClient, AiConfig, AiError, AiExtensionEvidence, AiMetadataInput, AiProvider,
    AiStatus, AiSubfolderEvidence,
};
use crate::rules::{classify_subpath, RiskLevel, RuleEngine, RuleError};
use crate::scanner::{
    compute_cleanable_bytes, move_items_to_trash, read_disk_health, sanitize_relative_path,
    scan_with_engine, summarize_scan, DiskHealth, ScanEvent, ScanItem, ScanSummary, TrashResult,
};

const MAX_AI_ITEMS_PER_SCAN: usize = 12;
const GITHUB_RELEASES_API: &str =
    "https://api.github.com/repos/vicode360/macleaner/releases/latest";
const GITHUB_RELEASES_PAGE: &str = "https://github.com/vicode360/macleaner/releases";
const GITHUB_ALLOWED_PREFIX: &str = "https://github.com/vicode360/macleaner/";
const UPDATE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub release_url: Option<String>,
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct GitHubReleasePayload {
    tag_name: String,
    html_url: String,
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
    #[error("invalid release URL")]
    InvalidReleaseUrl,
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

pub struct AppState {
    pub(crate) rules: Arc<RuleEngine>,
    pub(crate) ai_client: AiClient,
    pub(crate) ai_config: Mutex<AiConfig>,
    pub(crate) config_path: Option<PathBuf>,
    pub(crate) last_sizes: Mutex<HashMap<String, u64>>,
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

        Ok(Self {
            rules,
            ai_client,
            ai_config: Mutex::new(initial_config),
            config_path,
            last_sizes: Mutex::new(HashMap::new()),
        })
    }
}

#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
pub async fn check_for_updates(state: State<'_, AppState>) -> Result<UpdateCheckResult, AppError> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let user_agent = format!("Macleaner/{current}");

    let response = match state
        .ai_client
        .http_client()
        .get(GITHUB_RELEASES_API)
        .header("user-agent", user_agent)
        .header("accept", "application/vnd.github+json")
        .timeout(UPDATE_TIMEOUT)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(_) => {
            return Ok(UpdateCheckResult {
                current_version: current.clone(),
                latest_version: None,
                update_available: false,
                release_url: Some(GITHUB_RELEASES_PAGE.to_string()),
                message: format!(
                    "Sem conexão para verificar atualizações. Versão atual: v{current}."
                ),
            });
        }
    };

    if response.status().as_u16() == 404 {
        return Ok(UpdateCheckResult {
            current_version: current.clone(),
            latest_version: None,
            update_available: false,
            release_url: Some(GITHUB_RELEASES_PAGE.to_string()),
            message: format!("Versão atual v{current} — nenhuma release publicada ainda."),
        });
    }

    if !response.status().is_success() {
        return Ok(UpdateCheckResult {
            current_version: current.clone(),
            latest_version: None,
            update_available: false,
            release_url: Some(GITHUB_RELEASES_PAGE.to_string()),
            message: format!(
                "Não foi possível verificar atualizações (HTTP {}).",
                response.status()
            ),
        });
    }

    let Ok(bytes) = response.bytes().await else {
        return Ok(UpdateCheckResult {
            current_version: current.clone(),
            latest_version: None,
            update_available: false,
            release_url: Some(GITHUB_RELEASES_PAGE.to_string()),
            message: format!("Erro ao ler resposta de releases. Versão atual: v{current}."),
        });
    };

    let Ok(release) = serde_json::from_slice::<GitHubReleasePayload>(&bytes) else {
        return Ok(UpdateCheckResult {
            current_version: current.clone(),
            latest_version: None,
            update_available: false,
            release_url: Some(GITHUB_RELEASES_PAGE.to_string()),
            message: format!("Formato de release inesperado. Versão atual: v{current}."),
        });
    };

    let latest = release
        .tag_name
        .trim()
        .trim_start_matches(['v', 'V'])
        .to_string();
    let update_available = is_newer_version(&current, &latest);
    let message = if update_available {
        format!("Nova versão v{latest} disponível (atual: v{current}).")
    } else {
        format!("Você já está na versão mais recente (v{current}).")
    };

    Ok(UpdateCheckResult {
        current_version: current,
        latest_version: Some(latest),
        update_available,
        release_url: Some(release.html_url),
        message,
    })
}

#[tauri::command]
pub fn open_release_url(url: String) -> Result<(), AppError> {
    if !url.starts_with(GITHUB_ALLOWED_PREFIX) {
        return Err(AppError::InvalidReleaseUrl);
    }
    Command::new("open")
        .arg(&url)
        .spawn()
        .map_err(|err| AppError::SettingsIo(err.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn get_disk_health() -> DiskHealth {
    read_disk_health()
}

#[tauri::command]
pub async fn scan_system(
    min_mb: Option<u64>,
    state: State<'_, AppState>,
    on_event: Channel<ScanEvent>,
) -> Result<ScanSummary, AppError> {
    let mut ai_config = state
        .ai_config
        .lock()
        .expect("ai_config lock poisoned")
        .clone();
    let threshold_mb = min_mb.unwrap_or(ai_config.min_mb);

    let rules = Arc::clone(&state.rules);
    let event_channel = on_event.clone();

    let mut raw = tauri::async_runtime::spawn_blocking(move || {
        scan_with_engine(&rules, threshold_mb, &|event| {
            let _ = event_channel.send(event);
        })
    })
    .await
    .map_err(|err| AppError::TaskJoin(err.to_string()))?;

    let unknown_count = raw
        .items
        .iter()
        .filter(|item| item.rule_id.is_none())
        .count();
    let mut ai_available = false;

    if unknown_count > 0 && ai_config.provider != AiProvider::Disabled {
        let ai_status = state.ai_client.check_availability(&ai_config).await;
        ai_available = ai_status.available;

        if ai_status.available {
            if ai_config.provider == AiProvider::Ollama
                && ai_config.ollama_model != "auto"
                && ai_config.ollama_model != ai_status.model
            {
                ai_config.ollama_model = ai_status.model.clone();
                if let Some(ref path) = state.config_path {
                    let _ = save_persisted_config(path, &ai_config);
                }
                if let Ok(mut guard) = state.ai_config.lock() {
                    guard.ollama_model = ai_status.model.clone();
                }
            }

            let scanned_bytes = raw.items.iter().map(|item| item.size_bytes).sum();
            let batch_count = unknown_count.min(MAX_AI_ITEMS_PER_SCAN);
            let _ = on_event.send(ScanEvent::Progress {
                current_path: format!(
                    "IA ({}) analisando {batch_count} pastas desconhecidas…",
                    ai_status.model
                ),
                scanned_bytes,
            });

            let mut analyzed = 0_usize;
            for item in &mut raw.items {
                if item.rule_id.is_some() {
                    continue;
                }
                if analyzed < MAX_AI_ITEMS_PER_SCAN {
                    apply_ai_with_evidence(
                        &state.ai_client,
                        &state.rules,
                        &ai_config,
                        Some(&ai_status.model),
                        item,
                    )
                    .await;
                    analyzed += 1;
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

    Ok(summary)
}

#[tauri::command]
pub async fn trash_items(
    paths: Vec<String>,
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
        let mut sizes = state.last_sizes.lock().expect("last_sizes lock poisoned");
        for removed in &result.trashed_paths {
            sizes.remove(removed);
        }
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
pub fn set_ai_config(config: AiConfig, state: State<'_, AppState>) -> Result<AiConfig, AppError> {
    validate_config(&config)?;
    if let Some(ref path) = state.config_path {
        save_persisted_config(path, &config)?;
    }
    let mut guard = state.ai_config.lock().expect("ai_config lock poisoned");
    *guard = config.clone();
    Ok(config)
}

async fn apply_ai_with_evidence(
    ai_client: &AiClient,
    rules: &RuleEngine,
    config: &AiConfig,
    resolved_model: Option<&str>,
    item: &mut ScanItem,
) {
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

    let Some(classification) = ai_client
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
    else {
        return;
    };

    item.risk = classification.risk;
    item.reason = classification.reason;
    item.ai_classified = true;
    item.selected = matches!(classification.risk, RiskLevel::Baixo);

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

    item.cleanable_bytes = compute_cleanable_bytes(item.risk, item.size_bytes, &item.subitems);
}

#[must_use]
fn is_newer_version(current: &str, latest: &str) -> bool {
    let parse = |s: &str| -> Option<(u64, u64, u64)> {
        let clean = s.trim().trim_start_matches(['v', 'V']);
        let base = clean.split('-').next().unwrap_or(clean);
        let mut parts = base.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next().unwrap_or("0").parse().ok()?;
        let patch = parts.next().unwrap_or("0").parse().ok()?;
        Some((major, minor, patch))
    };

    match (parse(current), parse(latest)) {
        (Some(cur), Some(lat)) => lat > cur,
        _ => false,
    }
}

fn load_persisted_config(path: &Path) -> Option<AiConfig> {
    let raw = fs::read(path).ok()?;
    let mut parsed: AiConfig = serde_json::from_slice(&raw).ok()?;
    if parsed.timeout_ms < 15_000 {
        parsed.timeout_ms = 20_000;
    }
    validate_config(&parsed).ok()?;
    Some(parsed)
}

fn save_persisted_config(path: &Path, config: &AiConfig) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| AppError::SettingsIo(err.to_string()))?;
    }
    let payload =
        serde_json::to_vec_pretty(config).map_err(|err| AppError::SettingsIo(err.to_string()))?;
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, payload).map_err(|err| AppError::SettingsIo(err.to_string()))?;
    fs::rename(&tmp_path, path).map_err(|err| AppError::SettingsIo(err.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_comparison_detects_newer_releases() {
        assert!(is_newer_version("0.1.0", "0.2.0"));
        assert!(is_newer_version("0.1.0", "v0.1.1"));
        assert!(is_newer_version("0.9.9", "1.0.0"));
        assert!(!is_newer_version("0.1.0", "0.1.0"));
        assert!(!is_newer_version("0.2.0", "0.1.9"));
    }
}
