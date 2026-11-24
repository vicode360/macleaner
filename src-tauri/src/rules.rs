//! Cleaning rules and SIP-protected path filters for macOS.

use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

const KNOWN_RULES_TOML: &str = include_str!("../rules/known.toml");

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    #[serde(alias = "safe", alias = "low")]
    Baixo,
    #[serde(alias = "moderate", alias = "medium")]
    Medio,
    #[serde(alias = "caution", alias = "high")]
    Alto,
}

#[derive(Debug, Error)]
pub enum RuleError {
    #[error("could not resolve user home directory")]
    MissingHomeDir,
    #[error("failed to parse rules configuration: {0}")]
    Parse(#[from] toml::de::Error),
}

#[derive(Debug, Deserialize)]
struct RawRulesFile {
    protected: RawProtectedConfig,
    #[serde(default)]
    rule: Vec<RawRuleEntry>,
}

#[derive(Debug, Deserialize)]
struct RawProtectedConfig {
    #[serde(default)]
    sip_prefixes: Vec<String>,
    #[serde(default)]
    tcc_user_paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawRuleEntry {
    id: String,
    name: String,
    category: String,
    patterns: Vec<String>,
    risk: RiskLevel,
    reason: String,
    #[serde(default)]
    selected_by_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownRule {
    pub id: String,
    pub name: String,
    pub category: String,
    patterns: Vec<Vec<String>>,
    pub risk: RiskLevel,
    pub reason: String,
    pub selected_by_default: bool,
}

#[derive(Debug, Clone)]
pub struct RuleEngine {
    home_dir: PathBuf,
    protected_prefixes: Vec<PathBuf>,
    rules: Vec<KnownRule>,
}

impl RuleEngine {
    /// Loads the embedded macOS cleanup rules using the current user's home directory.
    ///
    /// # Errors
    /// Returns [`RuleError::MissingHomeDir`] if the home directory cannot be resolved,
    /// or [`RuleError::Parse`] if the embedded TOML is malformed.
    pub fn load_default() -> Result<Self, RuleError> {
        let home_dir = dirs::home_dir().ok_or(RuleError::MissingHomeDir)?;
        Ok(Self::from_toml_str(KNOWN_RULES_TOML, &home_dir)?)
    }

    /// Parses rules and protected prefixes from a TOML string relative to `home_dir`.
    ///
    /// # Errors
    /// Returns an error if `toml_str` is invalid TOML or missing required fields.
    pub fn from_toml_str(toml_str: &str, home_dir: &Path) -> Result<Self, toml::de::Error> {
        let raw: RawRulesFile = toml::from_str(toml_str)?;
        let normalized_home = normalize_path(home_dir);

        let protected_prefixes = raw
            .protected
            .sip_prefixes
            .into_iter()
            .chain(raw.protected.tcc_user_paths)
            .map(|entry| expand_tilde(&entry, &normalized_home))
            .map(|path| normalize_path(&path))
            .collect();

        let rules = raw
            .rule
            .into_iter()
            .map(|entry| KnownRule {
                id: entry.id,
                name: entry.name,
                category: entry.category,
                patterns: entry
                    .patterns
                    .into_iter()
                    .map(|p| split_pattern_components(&p))
                    .filter(|parts| !parts.is_empty())
                    .collect(),
                risk: entry.risk,
                reason: entry.reason,
                selected_by_default: entry.selected_by_default,
            })
            .collect();

        Ok(Self {
            home_dir: normalized_home,
            protected_prefixes,
            rules,
        })
    }

    #[must_use]
    pub fn home_dir(&self) -> &Path {
        &self.home_dir
    }

    #[must_use]
    pub fn is_protected_path(&self, path: &Path) -> bool {
        let normalized = normalize_path(path);
        if self.matches_protected(&normalized) {
            return true;
        }

        if let Ok(canonical) = fs::canonicalize(path) {
            if self.matches_protected(&canonical) {
                return true;
            }
        }

        false
    }

    #[must_use]
    pub fn classify_path(&self, path: &Path) -> Option<&KnownRule> {
        let components = lower_path_components(&normalize_path(path));
        if components.is_empty() {
            return None;
        }

        self.rules
            .iter()
            .filter_map(|rule| {
                rule.patterns
                    .iter()
                    .filter(|pattern| components.ends_with(pattern.as_slice()))
                    .map(|pattern| (pattern.len(), rule))
                    .max_by_key(|(len, _)| *len)
            })
            .max_by_key(|(len, _)| *len)
            .map(|(_, rule)| rule)
    }

    fn matches_protected(&self, candidate: &Path) -> bool {
        if candidate == Path::new("/") || candidate == self.home_dir {
            return true;
        }

        self.protected_prefixes
            .iter()
            .any(|prefix| candidate.starts_with(prefix))
    }
}

/// Classifies a relative subfolder path inside a scanned container deterministically.
#[must_use]
pub fn classify_subpath(sub_rel_name: &str) -> Option<(RiskLevel, &'static str, bool)> {
    let parts: Vec<String> = sub_rel_name
        .split(['/', '\\'])
        .filter(|segment| !segment.is_empty())
        .map(str::to_lowercase)
        .collect();

    if parts.is_empty() {
        return None;
    }

    if parts
        .windows(2)
        .any(|w| w[0] == "steamapps" && w[1] == "common")
    {
        return Some((
            RiskLevel::Alto,
            "Jogos instalados da Steam; remover exige baixar tudo novamente.",
            false,
        ));
    }

    for part in &parts {
        match part.as_str() {
            "workshop" => {
                return Some((
                    RiskLevel::Alto,
                    "Mods e conteúdo do Workshop; revise antes de remover.",
                    false,
                ));
            }
            "userdata" => {
                return Some((
                    RiskLevel::Alto,
                    "Saves locais, perfis e configurações do usuário.",
                    false,
                ));
            }
            "saves" | "savegames" => {
                return Some((
                    RiskLevel::Alto,
                    "Arquivos de salvamento de jogos ou aplicativos.",
                    false,
                ));
            }
            "profiles" => {
                return Some((
                    RiskLevel::Alto,
                    "Perfis locais com preferências e sessões do usuário.",
                    false,
                ));
            }
            "databases" | "indexeddb" | "local storage" => {
                return Some((
                    RiskLevel::Alto,
                    "Banco de dados local de estado do aplicativo.",
                    false,
                ));
            }
            "backups" | "projects" | "repositories" => {
                return Some((
                    RiskLevel::Alto,
                    "Projetos ou cópias de segurança locais do usuário.",
                    false,
                ));
            }
            _ => {}
        }
    }

    for part in parts.iter().rev() {
        match part.as_str() {
            "shadercache" => {
                return Some((
                    RiskLevel::Baixo,
                    "Cache de shaders compilados; recriado ao abrir o jogo ou app.",
                    true,
                ));
            }
            "gpucache" | "dawncache" | "grshadercache" => {
                return Some((
                    RiskLevel::Baixo,
                    "Cache gráfico de GPU recriado automaticamente pelo app.",
                    true,
                ));
            }
            "code cache" | "scriptcache" => {
                return Some((
                    RiskLevel::Baixo,
                    "Cache de scripts compilados; seguro para remover.",
                    true,
                ));
            }
            "httpcache" | "webcache" => {
                return Some((
                    RiskLevel::Baixo,
                    "Cache de requisições HTTP e mídia temporária.",
                    true,
                ));
            }
            "appcache" | "cache" | "caches" | "cache_data" => {
                return Some((
                    RiskLevel::Baixo,
                    "Cache temporário recriado pelo aplicativo quando necessário.",
                    true,
                ));
            }
            "logs" | "log" | "crashpad" | "crashes" | "crashreports" | "diagnosticreports" => {
                return Some((
                    RiskLevel::Baixo,
                    "Registros antigos de diagnóstico e relatórios de falha.",
                    true,
                ));
            }
            "tmp" | "temp" | "downloading" | "updates" | "installers" => {
                return Some((
                    RiskLevel::Baixo,
                    "Arquivos temporários de download ou atualização.",
                    true,
                ));
            }
            _ => {}
        }
    }

    None
}

fn split_pattern_components(pattern: &str) -> Vec<String> {
    pattern
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(str::to_lowercase)
        .collect()
}

fn lower_path_components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().to_lowercase()),
            _ => None,
        })
        .collect()
}

fn expand_tilde(raw: &str, home_dir: &Path) -> PathBuf {
    if raw == "~" {
        return home_dir.to_path_buf();
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        return home_dir.join(rest);
    }
    PathBuf::from(raw)
}

#[must_use]
pub(crate) fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Component::RootDir.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !matches!(
                    normalized.components().next_back(),
                    Some(Component::RootDir) | None
                ) {
                    normalized.pop();
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_engine() -> RuleEngine {
        RuleEngine::from_toml_str(KNOWN_RULES_TOML, Path::new("/Users/tester"))
            .expect("embedded known.toml should parse")
    }

    #[test]
    fn protected_paths_block_sip_tcc_and_traversal() {
        let engine = test_engine();
        assert!(engine.is_protected_path(Path::new("/System/Library/Caches")));
        assert!(engine.is_protected_path(Path::new("/usr/bin/env")));
        assert!(engine.is_protected_path(Path::new("/Users/tester/Library/Mail")));
        assert!(engine.is_protected_path(Path::new("/Users/tester/Library/Caches/../Safari")));
        assert!(engine.is_protected_path(Path::new("/Users/tester")));
        assert!(engine.is_protected_path(Path::new("/")));
    }

    #[test]
    fn classifies_all_known_heavy_macos_paths_case_insensitively() {
        let engine = test_engine();
        let cases = [
            (
                "/Users/tester/Library/Developer/Xcode/DerivedData",
                "xcode-derived-data",
                RiskLevel::Baixo,
            ),
            (
                "/Users/tester/Library/Developer/CoreSimulator/Caches",
                "coresimulator-caches",
                RiskLevel::Baixo,
            ),
            (
                "/Users/tester/Library/Caches/Homebrew",
                "homebrew",
                RiskLevel::Baixo,
            ),
            ("/Users/tester/.cache/npm", "npm", RiskLevel::Baixo),
            (
                "/Users/tester/Library/Caches/pnpm",
                "pnpm",
                RiskLevel::Baixo,
            ),
            (
                "/Users/tester/.cargo/registry",
                "cargo-registry",
                RiskLevel::Baixo,
            ),
            ("/Users/tester/Library/Caches/pip", "pip", RiskLevel::Baixo),
            (
                "/Users/tester/Library/Caches/go-build",
                "go-build",
                RiskLevel::Baixo,
            ),
            (
                "/Users/tester/Library/Caches/Google/Chrome/Default/Cache",
                "chrome-cache",
                RiskLevel::Baixo,
            ),
            (
                "/Users/tester/Library/Caches/Firefox/Profiles",
                "firefox-profiles",
                RiskLevel::Medio,
            ),
            (
                "/Users/tester/Library/Application Support/Spotify/Data",
                "spotify-data",
                RiskLevel::Baixo,
            ),
            (
                "/Users/tester/Library/Application Support/discord/Cache",
                "discord-cache",
                RiskLevel::Baixo,
            ),
            (
                "/Users/tester/Library/Application Support/Slack/Cache",
                "slack-cache",
                RiskLevel::Baixo,
            ),
            (
                "/Users/tester/.cache/huggingface/hub",
                "huggingface-hub",
                RiskLevel::Medio,
            ),
            (
                "/Users/tester/.ollama/models",
                "ollama-models",
                RiskLevel::Medio,
            ),
        ];

        for (path, expected_id, expected_risk) in cases {
            let matched = engine
                .classify_path(Path::new(path))
                .unwrap_or_else(|| panic!("expected rule match for {path}"));
            assert_eq!(matched.id, expected_id, "path {path}");
            assert_eq!(matched.risk, expected_risk, "path {path}");
        }
    }

    #[test]
    fn subpath_classifier_separates_critical_data_from_safe_caches() {
        let common = classify_subpath("steamapps/common").expect("common");
        assert_eq!(common.0, RiskLevel::Alto);
        assert!(!common.2);

        let shader = classify_subpath("steamapps/shadercache").expect("shadercache");
        assert_eq!(shader.0, RiskLevel::Baixo);
        assert!(shader.2);

        let http = classify_subpath("appcache/httpcache").expect("httpcache");
        assert_eq!(http.0, RiskLevel::Baixo);
        assert!(http.2);

        let logs = classify_subpath("logs").expect("logs");
        assert_eq!(logs.0, RiskLevel::Baixo);
        assert!(logs.2);

        let userdata = classify_subpath("userdata").expect("userdata");
        assert_eq!(userdata.0, RiskLevel::Alto);
        assert!(!userdata.2);
    }
}
