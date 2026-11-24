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
    pub fn is_protected_normalized(&self, path: &Path) -> bool {
        if has_relative_components(path) {
            return self.matches_protected(&normalize_path(path));
        }
        self.matches_protected(path)
    }

    #[must_use]
    pub fn is_protected_path(&self, path: &Path) -> bool {
        if self.is_protected_normalized(path) {
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
        let owned;
        let target = if has_relative_components(path) {
            owned = normalize_path(path);
            owned.as_path()
        } else {
            path
        };

        let last_component = target.components().rev().find_map(|c| match c {
            Component::Normal(part) => Some(part),
            _ => None,
        })?;

        let mut best: Option<(usize, &KnownRule)> = None;
        for rule in &self.rules {
            for pattern in &rule.patterns {
                let Some(last_pat) = pattern.last() else {
                    continue;
                };
                if !os_str_eq_lower(last_component, last_pat) {
                    continue;
                }
                if pattern.len() <= best.map_or(0, |(len, _)| len) {
                    continue;
                }
                if path_ends_with_pattern(target, pattern) {
                    best = Some((pattern.len(), rule));
                }
            }
        }

        best.map(|(_, rule)| rule)
    }

    #[must_use]
    pub fn is_protected_or_ancestor(&self, path: &Path) -> bool {
        let normalized = normalize_path(path);
        if self.matches_protected(&normalized) || self.matches_protected_ancestor(&normalized) {
            return true;
        }

        if let Ok(canonical) = fs::canonicalize(path) {
            if self.matches_protected(&canonical) || self.matches_protected_ancestor(&canonical) {
                return true;
            }
        }

        false
    }

    fn matches_protected(&self, candidate: &Path) -> bool {
        if candidate == Path::new("/") || paths_equal_platform(candidate, &self.home_dir) {
            return true;
        }

        self.protected_prefixes
            .iter()
            .any(|prefix| path_starts_with_platform(candidate, prefix))
    }

    fn matches_protected_ancestor(&self, candidate: &Path) -> bool {
        self.protected_prefixes
            .iter()
            .any(|prefix| path_starts_with_platform(prefix, candidate))
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
            "cacheddata" | "cachedextensionvsixs" | "crashreporter" => {
                return Some((
                    RiskLevel::Baixo,
                    "Cache temporário ou relatórios de falha recriados pelo app.",
                    true,
                ));
            }
            "node_modules" | "target" | ".venv" | "deriveddata" => {
                return Some((
                    RiskLevel::Baixo,
                    "Dependências ou artefatos de build regeneráveis pelo projeto.",
                    true,
                ));
            }
            "ios devicesupport" | "watchos devicesupport" | "tvos devicesupport" => {
                return Some((
                    RiskLevel::Baixo,
                    "Símbolos de depuração antigos do Xcode; recriados ao reconectar o aparelho.",
                    true,
                ));
            }
            _ => {}
        }
    }

    None
}

fn has_relative_components(path: &Path) -> bool {
    path.components()
        .any(|c| matches!(c, Component::CurDir | Component::ParentDir))
}

#[cfg(target_os = "macos")]
fn component_eq_platform(a: Component<'_>, b: Component<'_>) -> bool {
    match (a, b) {
        (Component::Normal(sa), Component::Normal(sb)) => {
            if let (Some(ta), Some(tb)) = (sa.to_str(), sb.to_str()) {
                if ta.is_ascii() && tb.is_ascii() {
                    return ta.eq_ignore_ascii_case(tb);
                }
                return ta.to_lowercase() == tb.to_lowercase();
            }
            sa.to_string_lossy().to_lowercase() == sb.to_string_lossy().to_lowercase()
        }
        _ => a == b,
    }
}

#[cfg(not(target_os = "macos"))]
fn component_eq_platform(a: Component<'_>, b: Component<'_>) -> bool {
    a == b
}

pub(crate) fn path_starts_with_platform(path: &Path, prefix: &Path) -> bool {
    let mut path_iter = path.components();
    for pref_comp in prefix.components() {
        let Some(path_comp) = path_iter.next() else {
            return false;
        };
        if !component_eq_platform(path_comp, pref_comp) {
            return false;
        }
    }
    true
}

pub(crate) fn paths_equal_platform(a: &Path, b: &Path) -> bool {
    let mut iter_a = a.components();
    let mut iter_b = b.components();
    loop {
        match (iter_a.next(), iter_b.next()) {
            (Some(ca), Some(cb)) => {
                if !component_eq_platform(ca, cb) {
                    return false;
                }
            }
            (None, None) => return true,
            _ => return false,
        }
    }
}

fn os_str_eq_lower(segment: &std::ffi::OsStr, lower_pat: &str) -> bool {
    if let Some(s) = segment.to_str() {
        if s.is_ascii() && lower_pat.is_ascii() {
            return s.eq_ignore_ascii_case(lower_pat);
        }
        return s.to_lowercase() == lower_pat;
    }
    segment.to_string_lossy().to_lowercase() == lower_pat
}

fn path_ends_with_pattern(path: &Path, pattern: &[String]) -> bool {
    let mut path_iter = path.components().rev().filter_map(|c| match c {
        Component::Normal(part) => Some(part),
        _ => None,
    });
    for pat in pattern.iter().rev() {
        let Some(seg) = path_iter.next() else {
            return false;
        };
        if !os_str_eq_lower(seg, pat) {
            return false;
        }
    }
    true
}

fn split_pattern_components(pattern: &str) -> Vec<String> {
    pattern
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(str::to_lowercase)
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

    #[test]
    fn protected_paths_are_case_insensitive_on_macos_and_block_ancestors() {
        let engine = test_engine();
        assert!(engine.is_protected_or_ancestor(Path::new("/Users/tester/Library")));
        assert!(engine.is_protected_or_ancestor(Path::new("/Users/tester/Library/Application Support")));
        assert!(engine.is_protected_or_ancestor(Path::new("/Users")));
        assert!(engine.is_protected_or_ancestor(Path::new("/usr")));
        assert!(engine.is_protected_or_ancestor(Path::new("/Users/tester/.ssh")));
        assert!(engine.is_protected_or_ancestor(Path::new("/Users/tester/Library/Keychains")));

        #[cfg(target_os = "macos")]
        {
            assert!(engine.is_protected_normalized(Path::new("/Users/tester/library/MAIL")));
            assert!(engine.is_protected_normalized(Path::new(
                "/Users/tester/Library/Application Support/MOBILESYNC"
            )));
        }
    }

    #[test]
    fn anchored_rules_do_not_hijack_nested_unrelated_folders() {
        let engine = test_engine();
        assert!(engine
            .classify_path(Path::new(
                "/Users/tester/Library/Application Support/GameApp/assets/uv"
            ))
            .is_none());
        assert!(engine
            .classify_path(Path::new(
                "/Users/tester/Library/Application Support/SomeApp/node_modules/npm"
            ))
            .is_none());
        assert_eq!(
            engine
                .classify_path(Path::new("/Users/tester/Library/pnpm/store"))
                .map(|r| r.id.as_str()),
            Some("pnpm")
        );
        assert_eq!(
            engine
                .classify_path(Path::new("/Users/tester/.cache/uv"))
                .map(|r| r.id.as_str()),
            Some("uv-cache")
        );
    }
}
