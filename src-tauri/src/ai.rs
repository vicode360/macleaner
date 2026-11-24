//! AI classification client supporting local Ollama and optional external HTTPS APIs.

use std::fmt;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri_plugin_http::reqwest::{self, Url};
use thiserror::Error;

use crate::rules::RiskLevel;

const HEALTH_CONNECT_TIMEOUT: Duration = Duration::from_millis(500);
const HEALTH_REQUEST_TIMEOUT: Duration = Duration::from_millis(1_500);
const UNLOAD_TIMEOUT: Duration = Duration::from_millis(2_000);
const DEFAULT_MIN_MB: u64 = 10;
const DEFAULT_OLLAMA_ENDPOINT: &str = "http://127.0.0.1:11434";
const DEFAULT_OLLAMA_MODEL: &str = "auto";
const DEFAULT_EXTERNAL_ENDPOINT: &str = "https://api.openai.com/v1/chat/completions";
const DEFAULT_EXTERNAL_MODEL: &str = "gpt-4o-mini";
const DEFAULT_TIMEOUT_MS: u64 = 20_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AppLanguage {
    #[default]
    PtBr,
    En,
    Es,
}

impl AppLanguage {
    fn prompt_name(self) -> &'static str {
        match self {
            Self::PtBr => "Portuguese (Brazil)",
            Self::En => "English",
            Self::Es => "Spanish",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum AiProvider {
    #[default]
    Ollama,
    ExternalApi,
    Disabled,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConfig {
    pub provider: AiProvider,
    #[serde(default)]
    pub language: AppLanguage,
    pub min_mb: u64,
    pub ollama_endpoint: String,
    pub ollama_model: String,
    pub external_endpoint: String,
    pub external_model: String,
    pub external_api_key: String,
    pub timeout_ms: u64,
}

impl fmt::Debug for AiConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let masked_key = if self.external_api_key.is_empty() {
            ""
        } else {
            "[REDACTED]"
        };
        f.debug_struct("AiConfig")
            .field("provider", &self.provider)
            .field("language", &self.language)
            .field("min_mb", &self.min_mb)
            .field("ollama_endpoint", &self.ollama_endpoint)
            .field("ollama_model", &self.ollama_model)
            .field("external_endpoint", &self.external_endpoint)
            .field("external_model", &self.external_model)
            .field("external_api_key", &masked_key)
            .field("timeout_ms", &self.timeout_ms)
            .finish()
    }
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: AiProvider::Ollama,
            language: AppLanguage::PtBr,
            min_mb: DEFAULT_MIN_MB,
            ollama_endpoint: DEFAULT_OLLAMA_ENDPOINT.to_string(),
            ollama_model: DEFAULT_OLLAMA_MODEL.to_string(),
            external_endpoint: DEFAULT_EXTERNAL_ENDPOINT.to_string(),
            external_model: DEFAULT_EXTERNAL_MODEL.to_string(),
            external_api_key: String::new(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiStatus {
    pub available: bool,
    pub provider: AiProvider,
    pub endpoint: String,
    pub model: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiSubfolderVerdict {
    pub name: String,
    pub risk: RiskLevel,
    pub reason: String,
    pub safe_to_clean: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiClassification {
    pub risk: RiskLevel,
    pub reason: String,
    pub subfolders: Vec<AiSubfolderVerdict>,
}

#[derive(Debug, Clone, Copy)]
pub struct AiSubfolderEvidence<'a> {
    pub name: &'a str,
    pub size_bytes: u64,
    pub file_count: u64,
    pub days_since_modified: u64,
    pub known_risk: Option<RiskLevel>,
}

#[derive(Debug, Clone, Copy)]
pub struct AiExtensionEvidence<'a> {
    pub ext: &'a str,
    pub size_bytes: u64,
    pub file_count: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct AiMetadataInput<'a> {
    pub name: &'a str,
    pub relative_path: &'a str,
    pub category: &'a str,
    pub size_bytes: u64,
    pub file_count: u64,
    pub days_since_modified: u64,
    pub language: AppLanguage,
    pub subfolders: &'a [AiSubfolderEvidence<'a>],
    pub extensions: &'a [AiExtensionEvidence<'a>],
}

#[derive(Debug, Error)]
pub enum AiError {
    #[error("invalid AI endpoint URL: {0}")]
    InvalidUrl(String),
    #[error("only local loopback HTTP endpoints are permitted for Ollama")]
    NonLoopbackEndpoint,
    #[error("external API endpoint must use HTTPS")]
    HttpsRequired,
    #[error("AI model name cannot be empty")]
    MissingModel,
    #[error("external API key is required when External API provider is selected")]
    MissingApiKey,
    #[error("minimum folder size must be at least 1 MB")]
    InvalidMinMb,
    #[error("Ollama request failed: {0}")]
    RequestFailed(String),
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    #[serde(default)]
    models: Vec<OllamaModelInfo>,
}

#[derive(Debug, Deserialize)]
struct OllamaModelInfo {
    name: String,
}

#[derive(Debug, Serialize)]
struct OllamaGenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    stream: bool,
    format: &'a str,
    think: bool,
    keep_alive: &'a str,
}

#[derive(Debug, Serialize)]
struct OllamaUnloadRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    stream: bool,
    keep_alive: u8,
}

#[derive(Debug, Deserialize)]
struct OllamaGenerateResponse {
    response: String,
}

#[derive(Debug, Serialize)]
struct OpenAiChatRequest<'a> {
    model: &'a str,
    temperature: f32,
    response_format: OpenAiResponseFormat<'a>,
    messages: [OpenAiMessage<'a>; 2],
}

#[derive(Debug, Serialize)]
struct OpenAiResponseFormat<'a> {
    #[serde(rename = "type")]
    format_type: &'a str,
}

#[derive(Debug, Serialize)]
struct OpenAiMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    #[serde(default)]
    choices: Vec<OpenAiChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoiceMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawAiSubfolderVerdict {
    name: String,
    risk: RiskLevel,
    reason: String,
    #[serde(default)]
    safe_to_clean: bool,
}

#[derive(Debug, Deserialize)]
struct RawAiClassification {
    risk: RiskLevel,
    reason: String,
    #[serde(default)]
    subfolders: Vec<RawAiSubfolderVerdict>,
}

#[derive(Debug, Clone)]
pub struct AiClient {
    http: reqwest::Client,
}

impl AiClient {
    /// Builds a reusable HTTP client configured for fast loopback checks.
    ///
    /// # Errors
    /// Returns an error if the underlying TLS/HTTP client fails to initialize.
    pub fn new() -> Result<Self, reqwest::Error> {
        let http = reqwest::Client::builder()
            .connect_timeout(HEALTH_CONNECT_TIMEOUT)
            .build()?;
        Ok(Self { http })
    }

    #[must_use]
    pub fn http_client(&self) -> &reqwest::Client {
        &self.http
    }

    /// Queries the local Ollama daemon for installed model names.
    ///
    /// # Errors
    /// Returns [`AiError`] if the endpoint is invalid/non-loopback or Ollama is unreachable.
    pub async fn list_ollama_models(&self, endpoint: &str) -> Result<Vec<String>, AiError> {
        let base_url = validate_loopback_endpoint(endpoint)?;
        let tags_url = base_url
            .join("api/tags")
            .map_err(|err| AiError::InvalidUrl(err.to_string()))?;

        let response = self
            .http
            .get(tags_url)
            .timeout(HEALTH_REQUEST_TIMEOUT)
            .send()
            .await
            .map_err(|err| AiError::RequestFailed(err.to_string()))?;

        if !response.status().is_success() {
            return Err(AiError::RequestFailed(format!(
                "HTTP {}",
                response.status()
            )));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|err| AiError::RequestFailed(err.to_string()))?;
        let tags: OllamaTagsResponse = serde_json::from_slice(&bytes)
            .map_err(|err| AiError::RequestFailed(err.to_string()))?;

        Ok(tags.models.into_iter().map(|m| m.name).collect())
    }

    pub async fn check_availability(&self, config: &AiConfig) -> AiStatus {
        match config.provider {
            AiProvider::Disabled => AiStatus {
                available: false,
                provider: AiProvider::Disabled,
                endpoint: String::new(),
                model: String::new(),
                reason: Some("IA desativada — usando apenas regras locais.".to_string()),
            },
            AiProvider::ExternalApi => {
                if let Err(err) = validate_config(config) {
                    return AiStatus {
                        available: false,
                        provider: AiProvider::ExternalApi,
                        endpoint: config.external_endpoint.clone(),
                        model: config.external_model.clone(),
                        reason: Some(err.to_string()),
                    };
                }
                AiStatus {
                    available: true,
                    provider: AiProvider::ExternalApi,
                    endpoint: config.external_endpoint.clone(),
                    model: config.external_model.clone(),
                    reason: None,
                }
            }
            AiProvider::Ollama => self.check_ollama(config).await,
        }
    }

    async fn check_ollama(&self, config: &AiConfig) -> AiStatus {
        let models = match self.list_ollama_models(&config.ollama_endpoint).await {
            Ok(models) => models,
            Err(err) => {
                return AiStatus {
                    available: false,
                    provider: AiProvider::Ollama,
                    endpoint: config.ollama_endpoint.clone(),
                    model: config.ollama_model.clone(),
                    reason: Some(err.to_string()),
                };
            }
        };

        let Some(resolved) = resolve_ollama_model(&models, &config.ollama_model) else {
            return AiStatus {
                available: false,
                provider: AiProvider::Ollama,
                endpoint: config.ollama_endpoint.clone(),
                model: config.ollama_model.clone(),
                reason: Some(
                    "Nenhum modelo instalado no Ollama. Execute 'ollama pull <modelo>'."
                        .to_string(),
                ),
            };
        };

        AiStatus {
            available: true,
            provider: AiProvider::Ollama,
            endpoint: config.ollama_endpoint.clone(),
            model: resolved,
            reason: None,
        }
    }

    pub async fn classify_metadata(
        &self,
        config: &AiConfig,
        resolved_model: Option<&str>,
        input: AiMetadataInput<'_>,
    ) -> Option<AiClassification> {
        let user_prompt = build_metadata_prompt(input);

        match config.provider {
            AiProvider::Disabled => None,
            AiProvider::Ollama => {
                let model = resolved_model.unwrap_or(config.ollama_model.trim());
                self.classify_with_ollama(config, model, &user_prompt).await
            }
            AiProvider::ExternalApi => self.classify_with_external_api(config, &user_prompt).await,
        }
    }

    pub async fn unload_ollama_model(&self, endpoint: &str, model: &str) {
        let Ok(base_url) = validate_loopback_endpoint(endpoint) else {
            return;
        };
        let Ok(generate_url) = base_url.join("api/generate") else {
            return;
        };
        let Ok(body) = serde_json::to_vec(&OllamaUnloadRequest {
            model,
            prompt: "",
            stream: false,
            keep_alive: 0,
        }) else {
            return;
        };

        let _ = self
            .http
            .post(generate_url)
            .header("content-type", "application/json")
            .body(body)
            .timeout(UNLOAD_TIMEOUT)
            .send()
            .await;
    }

    async fn classify_with_ollama(
        &self,
        config: &AiConfig,
        model: &str,
        prompt: &str,
    ) -> Option<AiClassification> {
        if model.trim().is_empty() || model.eq_ignore_ascii_case("auto") {
            return None;
        }
        let base_url = validate_loopback_endpoint(&config.ollama_endpoint).ok()?;
        let generate_url = base_url.join("api/generate").ok()?;

        let body = serde_json::to_vec(&OllamaGenerateRequest {
            model: model.trim(),
            prompt,
            stream: false,
            format: "json",
            think: false,
            keep_alive: "30s",
        })
        .ok()?;

        let timeout = Duration::from_millis(config.timeout_ms.clamp(15_000, 60_000));
        let response = self
            .http
            .post(generate_url)
            .header("content-type", "application/json")
            .body(body)
            .timeout(timeout)
            .send()
            .await
            .ok()?;

        if !response.status().is_success() {
            return None;
        }

        let bytes = response.bytes().await.ok()?;
        let outer: OllamaGenerateResponse = serde_json::from_slice(&bytes).ok()?;
        parse_classification_json(&outer.response)
    }

    async fn classify_with_external_api(
        &self,
        config: &AiConfig,
        user_prompt: &str,
    ) -> Option<AiClassification> {
        validate_config(config).ok()?;
        let endpoint = validate_https_endpoint(&config.external_endpoint).ok()?;

        let body = serde_json::to_vec(&OpenAiChatRequest {
            model: config.external_model.trim(),
            temperature: 0.0,
            response_format: OpenAiResponseFormat {
                format_type: "json_object",
            },
            messages: [
                OpenAiMessage {
                    role: "system",
                    content: "You analyze real macOS directory telemetry (subfolders and extensions) for safe disk cleanup. Cite the actual subfolder names and weights observed; never invent generic summaries. Reply strictly with JSON: {\"risk\":\"baixo\"|\"medio\"|\"alto\",\"reason\":\"concise explanation in requested language citing subfolders\",\"subfolders\":[{\"name\":\"exact subfolder name\",\"risk\":\"baixo\"|\"medio\"|\"alto\",\"reason\":\"short explanation in requested language\",\"safeToClean\":boolean}]}",
                },
                OpenAiMessage {
                    role: "user",
                    content: user_prompt,
                },
            ],
        })
        .ok()?;

        let timeout = Duration::from_millis(config.timeout_ms.clamp(500, 30_000));
        let response = self
            .http
            .post(endpoint)
            .bearer_auth(config.external_api_key.trim())
            .header("content-type", "application/json")
            .body(body)
            .timeout(timeout)
            .send()
            .await
            .ok()?;

        if !response.status().is_success() {
            return None;
        }

        let bytes = response.bytes().await.ok()?;
        let parsed: OpenAiChatResponse = serde_json::from_slice(&bytes).ok()?;
        let content = parsed.choices.into_iter().next()?.message.content?;
        parse_classification_json(&content)
    }
}

/// Validates complete AI and scanning configuration before persisting or running queries.
///
/// # Errors
/// Returns [`AiError`] if `min_mb` is zero or the selected provider fields are invalid.
pub fn validate_config(config: &AiConfig) -> Result<(), AiError> {
    if config.min_mb < 1 {
        return Err(AiError::InvalidMinMb);
    }

    match config.provider {
        AiProvider::Ollama => {
            validate_loopback_endpoint(&config.ollama_endpoint)?;
        }
        AiProvider::ExternalApi => {
            validate_https_endpoint(&config.external_endpoint)?;
            if config.external_model.trim().is_empty() {
                return Err(AiError::MissingModel);
            }
            if config.external_api_key.trim().is_empty() {
                return Err(AiError::MissingApiKey);
            }
        }
        AiProvider::Disabled => {}
    }

    Ok(())
}

/// Validates that an Ollama endpoint strictly targets local loopback HTTP.
///
/// # Errors
/// Returns [`AiError::InvalidUrl`] if malformed or [`AiError::NonLoopbackEndpoint`] if non-local.
pub fn validate_loopback_endpoint(endpoint: &str) -> Result<Url, AiError> {
    let parsed = Url::parse(endpoint.trim()).map_err(|err| AiError::InvalidUrl(err.to_string()))?;
    if parsed.scheme() != "http" {
        return Err(AiError::NonLoopbackEndpoint);
    }

    let is_loopback = matches!(
        parsed.host_str(),
        Some("127.0.0.1" | "localhost" | "::1" | "[::1]")
    );
    if !is_loopback {
        return Err(AiError::NonLoopbackEndpoint);
    }

    Ok(parsed)
}

/// Validates that an external LLM API endpoint uses HTTPS.
///
/// # Errors
/// Returns [`AiError::InvalidUrl`] if malformed or [`AiError::HttpsRequired`] if not HTTPS.
pub fn validate_https_endpoint(endpoint: &str) -> Result<Url, AiError> {
    let parsed = Url::parse(endpoint.trim()).map_err(|err| AiError::InvalidUrl(err.to_string()))?;
    if parsed.scheme() != "https" || parsed.host_str().is_none() {
        return Err(AiError::HttpsRequired);
    }
    Ok(parsed)
}

#[must_use]
pub(crate) fn resolve_ollama_model(installed: &[String], requested: &str) -> Option<String> {
    if installed.is_empty() {
        return None;
    }
    let req = requested.trim();
    if !req.is_empty() && !req.eq_ignore_ascii_case("auto") {
        if let Some(exact) = installed.iter().find(|m| model_matches(m, req)) {
            return Some(exact.clone());
        }
    }
    Some(installed[0].clone())
}

fn build_metadata_prompt(input: AiMetadataInput<'_>) -> String {
    let subfolder_lines = if input.subfolders.is_empty() {
        "  (nenhuma subpasta relevante)".to_string()
    } else {
        input
            .subfolders
            .iter()
            .map(|sub| {
                let tag = match sub.known_risk {
                    Some(RiskLevel::Baixo) => "regra:baixo",
                    Some(RiskLevel::Medio) => "regra:medio",
                    Some(RiskLevel::Alto) => "regra:alto",
                    None => "desconhecido",
                };
                format!(
                    "  - {}: {} ({} arq, {}d) [{}]",
                    sub.name,
                    format_mb_gb(sub.size_bytes),
                    sub.file_count,
                    sub.days_since_modified,
                    tag
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let ext_summary = if input.extensions.is_empty() {
        "nenhuma".to_string()
    } else {
        input
            .extensions
            .iter()
            .map(|ext| {
                format!(
                    "{} ({}, {} arq)",
                    ext.ext,
                    format_mb_gb(ext.size_bytes),
                    ext.file_count
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    };

    let lang = input.language.prompt_name();
    format!(
        "Analyze this macOS directory using ONLY the observed filesystem evidence below. \
         Do NOT write vague generic summaries; explicitly cite the real subfolder names and weights. \
         Write all explanations in {lang}. Return valid JSON:\n\
         {{\"risk\":\"baixo\"|\"medio\"|\"alto\",\"reason\":\"concise summary in {lang} citing real subfolders\",\"subfolders\":[{{\"name\":\"exact subfolder name\",\"risk\":\"baixo\"|\"medio\"|\"alto\",\"reason\":\"short explanation in {lang}\",\"safeToClean\":true|false}}]}}\n\
         Rules: keep [regra:alto] subfolders as alto and safeToClean=false. Never guess baixo if uncertain.\n\n\
         Folder: {} ({})\nCategory: {}\nTotal: {} | Files: {} | Age: {}d\nTop extensions: {}\nObserved subfolders:\n{}",
        input.name,
        input.relative_path,
        input.category,
        format_mb_gb(input.size_bytes),
        input.file_count,
        input.days_since_modified,
        ext_summary,
        subfolder_lines
    )
}

fn format_mb_gb(bytes: u64) -> String {
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    #[expect(clippy::cast_precision_loss, reason = "prompt display only")]
    let b = bytes as f64;
    if b >= GIB {
        format!("{:.1} GB", b / GIB)
    } else {
        format!("{:.0} MB", (b / MIB).max(1.0))
    }
}

fn model_matches(installed: &str, requested: &str) -> bool {
    let req = requested.trim();
    installed == req
        || installed
            .strip_suffix(":latest")
            .is_some_and(|base| base == req)
}

fn parse_classification_json(raw: &str) -> Option<AiClassification> {
    let trimmed = raw
        .trim()
        .strip_prefix("```json")
        .or_else(|| raw.trim().strip_prefix("```"))
        .unwrap_or(raw.trim());
    let cleaned = trimmed.strip_suffix("```").unwrap_or(trimmed).trim();

    let parsed: RawAiClassification = serde_json::from_str(cleaned).ok()?;
    let reason = parsed.reason.trim();
    if reason.is_empty() {
        return None;
    }

    let subfolders = parsed
        .subfolders
        .into_iter()
        .filter(|sub| !sub.name.trim().is_empty() && !sub.reason.trim().is_empty())
        .map(|sub| AiSubfolderVerdict {
            name: sub.name.trim().to_string(),
            risk: sub.risk,
            reason: sub.reason.trim().to_string(),
            safe_to_clean: sub.safe_to_clean && matches!(sub.risk, RiskLevel::Baixo),
        })
        .collect();

    Some(AiClassification {
        risk: parsed.risk,
        reason: reason.to_string(),
        subfolders,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_and_https_validations_enforce_security_boundaries() {
        assert!(validate_loopback_endpoint("http://127.0.0.1:11434").is_ok());
        assert!(validate_loopback_endpoint("http://localhost:11434").is_ok());
        assert!(validate_loopback_endpoint("http://[::1]:11434").is_ok());
        assert!(validate_loopback_endpoint("https://api.example.com").is_err());
        assert!(validate_loopback_endpoint("http://192.168.1.50:11434").is_err());

        assert!(validate_https_endpoint("https://api.openai.com/v1/chat/completions").is_ok());
        assert!(validate_https_endpoint("http://api.openai.com/v1/chat/completions").is_err());
    }

    #[test]
    fn resolve_ollama_model_handles_auto_exact_and_missing_fallback() {
        let installed = vec!["qwen3:8b".to_string(), "mistral:latest".to_string()];

        assert_eq!(
            resolve_ollama_model(&installed, "auto").as_deref(),
            Some("qwen3:8b")
        );
        assert_eq!(
            resolve_ollama_model(&installed, "mistral").as_deref(),
            Some("mistral:latest")
        );
        assert_eq!(
            resolve_ollama_model(&installed, "llama3.2:3b").as_deref(),
            Some("qwen3:8b")
        );
        assert_eq!(resolve_ollama_model(&[], "qwen3:8b"), None);
    }

    #[test]
    fn validate_config_checks_provider_requirements_and_debug_redacts_key() {
        let mut cfg = AiConfig {
            provider: AiProvider::ExternalApi,
            external_api_key: "sk-secret-value".to_string(),
            ..AiConfig::default()
        };
        assert!(validate_config(&cfg).is_ok());
        assert!(!format!("{cfg:?}").contains("sk-secret-value"));
        assert!(format!("{cfg:?}").contains("[REDACTED]"));

        cfg.external_api_key.clear();
        assert!(validate_config(&cfg).is_err());

        cfg.provider = AiProvider::Ollama;
        cfg.ollama_model.clear();
        assert!(validate_config(&cfg).is_ok());

        cfg.provider = AiProvider::Disabled;
        assert!(validate_config(&cfg).is_ok());

        cfg.min_mb = 0;
        assert!(validate_config(&cfg).is_err());
    }

    #[test]
    fn parses_structured_subfolder_json_and_legacy_json() {
        let raw = r#"{
          "risk": "alto",
          "reason": "Steam concentra 16 GB em steamapps/common e 1.1 GB em steamapps/shadercache.",
          "subfolders": [
            {"name": "steamapps/common", "risk": "alto", "reason": "Jogos instalados.", "safeToClean": false},
            {"name": "steamapps/shadercache", "risk": "baixo", "reason": "Cache de shaders.", "safeToClean": true}
          ]
        }"#;
        let parsed = parse_classification_json(raw).expect("valid structured json");
        assert_eq!(parsed.risk, RiskLevel::Alto);
        assert_eq!(parsed.subfolders.len(), 2);
        assert!(!parsed.subfolders[0].safe_to_clean);
        assert!(parsed.subfolders[1].safe_to_clean);

        let fenced = "```json\n{\"risk\":\"medio\",\"reason\":\"Dados de build.\"}\n```";
        let parsed_fenced = parse_classification_json(fenced).expect("valid fenced json");
        assert_eq!(parsed_fenced.risk, RiskLevel::Medio);
        assert!(parsed_fenced.subfolders.is_empty());

        assert!(parse_classification_json(r#"{"risk":"unknown","reason":"x"}"#).is_none());
    }

    #[test]
    fn disabled_ai_returns_offline_without_network() {
        let client = AiClient::new().expect("client builds");
        let cfg = AiConfig {
            provider: AiProvider::Disabled,
            ..AiConfig::default()
        };
        let status = tauri::async_runtime::block_on(client.check_availability(&cfg));
        assert!(!status.available);
    }
}
