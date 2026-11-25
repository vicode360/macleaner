//! Dev project build artifact scanner (`Kondo`-style) with strict root and manifest validation.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime};

use jwalk::WalkDir;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::rules::{
    has_unsafe_raw_path_syntax, normalize_path, path_starts_with_platform, paths_equal_platform,
    RuleEngine,
};
use crate::scanner::{days_since, dir_disk_size, TrashFailure};

const MAX_SCAN_DEPTH: usize = 6;
const MAX_MTIME_SCAN_DEPTH: usize = 5;
const MAX_MTIME_ENTRIES: usize = 512;

const DEFAULT_ROOT_NAMES: &[&str] = &["Projects", "Developer", "Code", "workspace", "Repos"];

const FORBIDDEN_HOME_SUBDIRS: &[&str] = &[
    "Library",
    ".ssh",
    ".gnupg",
    ".aws",
    ".kube",
    "Applications",
    ".Trash",
];

const ARTIFACT_LEAVES: &[&str] = &[
    "node_modules",
    ".next",
    ".nuxt",
    ".turbo",
    ".svelte-kit",
    ".parcel-cache",
    "target",
    ".venv",
    "venv",
    ".build",
    "Pods",
    ".dart_tool",
    "build",
    ".gradle",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevArtifact {
    pub path: String,
    pub name: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevProject {
    pub id: String,
    pub name: String,
    pub path: String,
    pub kind: String,
    pub total_artifact_bytes: u64,
    pub inactive_days: u32,
    pub artifacts: Vec<DevArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevProjectsReport {
    pub roots: Vec<String>,
    pub projects: Vec<DevProject>,
    pub total_reclaimable_bytes: u64,
    pub scan_duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevTrashResult {
    pub trashed_paths: Vec<String>,
    pub freed_bytes: u64,
    pub failed: Vec<TrashFailure>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectKind {
    Rust,
    NodeJs,
    Python,
    Flutter,
    Swift,
    IosCocoaPods,
    JavaGradle,
    JavaMaven,
}

impl ProjectKind {
    fn label(self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::NodeJs => "Node.js",
            Self::Python => "Python",
            Self::Flutter => "Flutter",
            Self::Swift => "Swift",
            Self::IosCocoaPods => "iOS (CocoaPods)",
            Self::JavaGradle => "Java / Gradle",
            Self::JavaMaven => "Java / Maven",
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct ParentManifests(u16);

impl ParentManifests {
    const PACKAGE_JSON: u16 = 1 << 0;
    const CARGO_TOML: u16 = 1 << 1;
    const POM_XML: u16 = 1 << 2;
    const PYTHON_MANIFEST: u16 = 1 << 3;
    const PY_FILE: u16 = 1 << 4;
    const PACKAGE_SWIFT: u16 = 1 << 5;
    const PODFILE: u16 = 1 << 6;
    const PUBSPEC_YAML: u16 = 1 << 7;
    const GRADLE: u16 = 1 << 8;

    fn has(self, flag: u16) -> bool {
        (self.0 & flag) != 0
    }

    fn record_file_name(&mut self, name: &OsStr) {
        let Some(text) = name.to_str() else {
            return;
        };

        if name_eq_platform(text, "package.json") {
            self.0 |= Self::PACKAGE_JSON;
        } else if name_eq_platform(text, "Cargo.toml") {
            self.0 |= Self::CARGO_TOML;
        } else if name_eq_platform(text, "pom.xml") {
            self.0 |= Self::POM_XML;
        } else if name_eq_platform(text, "pyproject.toml")
            || name_eq_platform(text, "requirements.txt")
            || name_eq_platform(text, "Pipfile")
            || name_eq_platform(text, "setup.py")
        {
            self.0 |= Self::PYTHON_MANIFEST;
        } else if name_eq_platform(text, "Package.swift") {
            self.0 |= Self::PACKAGE_SWIFT;
        } else if name_eq_platform(text, "Podfile") {
            self.0 |= Self::PODFILE;
        } else if name_eq_platform(text, "pubspec.yaml") {
            self.0 |= Self::PUBSPEC_YAML;
        } else if name_eq_platform(text, "build.gradle")
            || name_eq_platform(text, "build.gradle.kts")
            || name_eq_platform(text, "settings.gradle")
            || name_eq_platform(text, "settings.gradle.kts")
        {
            self.0 |= Self::GRADLE;
        }

        if has_py_extension(text) {
            self.0 |= Self::PY_FILE;
        }
    }

    fn from_dir(dir: &Path) -> Self {
        let mut manifests = Self::default();
        let Ok(entries) = fs::read_dir(dir) else {
            return manifests;
        };
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|ft| ft.is_file()) {
                manifests.record_file_name(&entry.file_name());
            }
        }
        manifests
    }
}

struct DiscoveredArtifact {
    project_dir: PathBuf,
    artifact_path: PathBuf,
    name: String,
    kind: ProjectKind,
}

struct MeasuredArtifact {
    project_dir: PathBuf,
    artifact: DevArtifact,
    kind: ProjectKind,
}

fn detect_default_roots_in(home_dir: &Path) -> Vec<String> {
    let mut canonical_roots: Vec<PathBuf> = Vec::new();

    for name in DEFAULT_ROOT_NAMES {
        let candidate = home_dir.join(name);
        let Ok(meta) = fs::symlink_metadata(&candidate) else {
            continue;
        };
        if !meta.file_type().is_dir() {
            continue;
        }
        let Ok(canonical) = fs::canonicalize(&candidate) else {
            continue;
        };
        if !canonical_roots
            .iter()
            .any(|existing| paths_equal_platform(existing, &canonical))
        {
            canonical_roots.push(canonical);
        }
    }

    canonical_roots
        .into_iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
}

/// Returns the active dev roots, using validated configured roots once initialized or defaults otherwise.
#[must_use]
pub fn effective_roots(
    configured: &[String],
    initialized: bool,
    engine: &RuleEngine,
) -> Vec<String> {
    if !initialized {
        return detect_default_roots_in(engine.home_dir());
    }
    canonical_validated_roots(configured, engine)
        .into_iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
}

/// Validates that `candidate` is a safe, non-symlink directory strictly inside `$HOME` suitable as a dev root.
///
/// # Errors
/// Returns an explanatory error string if `candidate` is relative, missing, a symlink, not a directory,
/// outside `$HOME`, or inside a protected or sensitive directory.
pub fn validate_dev_root(candidate: &Path, engine: &RuleEngine) -> Result<PathBuf, String> {
    if !candidate.is_absolute() || has_unsafe_raw_path_syntax(candidate) {
        return Err("Dev project root must be a clean absolute path.".to_string());
    }

    let normalized = normalize_path(candidate);
    if engine.is_protected_or_ancestor(&normalized) {
        return Err("Path is protected by system security policy.".to_string());
    }

    let meta = fs::symlink_metadata(&normalized)
        .map_err(|_| "Dev project root does not exist on disk.".to_string())?;
    if meta.file_type().is_symlink() {
        return Err("Symlinks are not allowed as dev project roots.".to_string());
    }
    if !meta.file_type().is_dir() {
        return Err("Dev project root must be a directory.".to_string());
    }

    let canonical_home = fs::canonicalize(engine.home_dir())
        .map_err(|_| "Failed to resolve user home directory.".to_string())?;
    let canonical = fs::canonicalize(&normalized)
        .map_err(|_| "Failed to canonicalize dev project root.".to_string())?;

    if !path_starts_with_platform(&canonical, &canonical_home)
        || paths_equal_platform(&canonical, &canonical_home)
    {
        return Err(
            "Dev project root must be strictly inside the user home directory.".to_string(),
        );
    }

    if engine.is_protected_or_ancestor(&canonical) {
        return Err("Path is protected by system security policy.".to_string());
    }

    for subdir in FORBIDDEN_HOME_SUBDIRS {
        let forbidden = canonical_home.join(subdir);
        if path_starts_with_platform(&canonical, &forbidden) {
            return Err(
                "Selected directory is restricted and cannot be scanned as a dev root.".to_string(),
            );
        }
    }

    Ok(canonical)
}

/// Validates that `candidate` is an allowlisted build artifact directory with a matching sibling project manifest
/// inside one of the validated `roots`.
///
/// # Errors
/// Returns an explanatory error string if `candidate` fails boundary, symlink, allowlist, or manifest checks.
pub fn validate_dev_artifact_target(
    candidate: &Path,
    roots: &[String],
    engine: &RuleEngine,
) -> Result<PathBuf, String> {
    if !candidate.is_absolute() || has_unsafe_raw_path_syntax(candidate) {
        return Err("Artifact path must be a clean absolute path.".to_string());
    }

    let normalized = normalize_path(candidate);
    if engine.is_protected_or_ancestor(&normalized) {
        return Err("Path is protected by system security policy.".to_string());
    }

    let meta = fs::symlink_metadata(&normalized)
        .map_err(|_| "Artifact directory not found on disk.".to_string())?;
    if meta.file_type().is_symlink() {
        return Err("Symlinks are not allowed as cleanable dev artifacts.".to_string());
    }
    if !meta.file_type().is_dir() {
        return Err("Dev artifact target must be a directory.".to_string());
    }

    let raw_parent = normalized
        .parent()
        .ok_or_else(|| "Artifact path has no parent directory.".to_string())?;
    let canonical_parent = fs::canonicalize(raw_parent)
        .map_err(|_| "Failed to resolve artifact parent directory.".to_string())?;
    let canonical_target = fs::canonicalize(&normalized)
        .map_err(|_| "Failed to resolve artifact directory.".to_string())?;

    if engine.is_protected_or_ancestor(&canonical_target) {
        return Err("Path is protected by system security policy.".to_string());
    }

    if !path_starts_with_platform(&canonical_target, &canonical_parent)
        || paths_equal_platform(&canonical_target, &canonical_parent)
        || !canonical_target
            .parent()
            .is_some_and(|parent| paths_equal_platform(parent, &canonical_parent))
    {
        return Err("Artifact escapes its parent directory.".to_string());
    }

    let canonical_roots = canonical_validated_roots(roots, engine);
    let inside_root = canonical_roots
        .iter()
        .any(|root| path_starts_with_platform(&canonical_parent, root));
    if !inside_root {
        return Err("Artifact is outside configured dev project roots.".to_string());
    }

    let leaf = canonical_target
        .file_name()
        .ok_or_else(|| "Artifact path has no directory name.".to_string())?;
    let manifests = ParentManifests::from_dir(&canonical_parent);
    if match_artifact_kind(leaf, &canonical_target, manifests).is_none() {
        return Err(
            "Directory is not a recognized build artifact with a matching project manifest."
                .to_string(),
        );
    }

    Ok(canonical_target)
}

/// Validates that `candidate` can be safely revealed in Finder (either a valid dev artifact or a project directory
/// strictly inside a configured dev root).
///
/// # Errors
/// Returns an explanatory error string if `candidate` is outside allowed roots, a symlink, or protected.
pub fn validate_dev_reveal_target(
    candidate: &Path,
    roots: &[String],
    engine: &RuleEngine,
) -> Result<PathBuf, String> {
    if !candidate.is_absolute() || has_unsafe_raw_path_syntax(candidate) {
        return Err("Target path must be a clean absolute path.".to_string());
    }

    if let Ok(canonical) = validate_dev_artifact_target(candidate, roots, engine) {
        return Ok(canonical);
    }

    let normalized = normalize_path(candidate);
    if engine.is_protected_or_ancestor(&normalized) {
        return Err("Path is protected by system security policy.".to_string());
    }

    let meta = fs::symlink_metadata(&normalized)
        .map_err(|_| "Directory not found on disk.".to_string())?;
    if meta.file_type().is_symlink() {
        return Err("Symlinks cannot be revealed through dev projects.".to_string());
    }
    if !meta.file_type().is_dir() {
        return Err("Reveal target must be a directory.".to_string());
    }

    let canonical = fs::canonicalize(&normalized)
        .map_err(|_| "Failed to resolve target directory.".to_string())?;
    if engine.is_protected_or_ancestor(&canonical) {
        return Err("Path is protected by system security policy.".to_string());
    }

    let canonical_roots = canonical_validated_roots(roots, engine);
    let strictly_inside_root = canonical_roots.iter().any(|root| {
        path_starts_with_platform(&canonical, root) && !paths_equal_platform(&canonical, root)
    });
    if !strictly_inside_root {
        return Err("Target directory is outside configured dev project roots.".to_string());
    }

    Ok(canonical)
}

/// Scans validated dev roots for regenerable build artifacts, pruning `.git`, ignored paths, and matched artifacts.
#[must_use]
pub fn scan_dev_projects(
    roots: &[String],
    ignored_paths: &[String],
    engine: &RuleEngine,
) -> DevProjectsReport {
    let started_at = Instant::now();
    let canonical_roots = canonical_validated_roots(roots, engine);
    let report_roots: Vec<String> = canonical_roots
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();

    if canonical_roots.is_empty() {
        return DevProjectsReport {
            roots: report_roots,
            projects: Vec::new(),
            total_reclaimable_bytes: 0,
            scan_duration_ms: u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX),
        };
    }

    let normalized_ignored = Arc::new(build_ignored_paths(ignored_paths));
    let discovered_raw: Vec<DiscoveredArtifact> = canonical_roots
        .par_iter()
        .flat_map(|root| discover_artifacts_in_root(root, Arc::clone(&normalized_ignored), engine))
        .collect();

    let mut unique_artifacts: Vec<DiscoveredArtifact> = Vec::with_capacity(discovered_raw.len());
    for item in discovered_raw {
        let already_seen = unique_artifacts
            .iter()
            .any(|existing| paths_equal_platform(&existing.artifact_path, &item.artifact_path));
        if !already_seen {
            unique_artifacts.push(item);
        }
    }

    let measured: Vec<MeasuredArtifact> = unique_artifacts
        .into_par_iter()
        .map(|item| {
            let size_bytes = dir_disk_size(&item.artifact_path);
            MeasuredArtifact {
                project_dir: item.project_dir,
                artifact: DevArtifact {
                    path: item.artifact_path.to_string_lossy().into_owned(),
                    name: item.name,
                    size_bytes,
                },
                kind: item.kind,
            }
        })
        .collect();

    let mut grouped: HashMap<PathBuf, Vec<(DevArtifact, ProjectKind)>> = HashMap::new();
    for entry in measured {
        grouped
            .entry(entry.project_dir)
            .or_default()
            .push((entry.artifact, entry.kind));
    }

    let now = SystemTime::now();
    let mut projects: Vec<DevProject> = grouped
        .into_par_iter()
        .map(|(project_dir, mut entries)| {
            entries.sort_by(|(a, _), (b, _)| {
                b.size_bytes
                    .cmp(&a.size_bytes)
                    .then_with(|| a.name.cmp(&b.name))
            });

            let kind = entries
                .first()
                .map_or("Unknown", |(_, k)| k.label())
                .to_string();
            let artifacts: Vec<DevArtifact> = entries.into_iter().map(|(a, _)| a).collect();
            let total_artifact_bytes = artifacts
                .iter()
                .fold(0_u64, |acc, art| acc.saturating_add(art.size_bytes));
            let path_str = project_dir.to_string_lossy().into_owned();
            let name = project_dir
                .file_name()
                .map_or_else(|| path_str.clone(), |n| n.to_string_lossy().into_owned());
            let inactive_days = compute_project_inactive_days(&project_dir, now);

            DevProject {
                id: format!("dev:{path_str}"),
                name,
                path: path_str,
                kind,
                total_artifact_bytes,
                inactive_days,
                artifacts,
            }
        })
        .collect();

    projects.sort_by(|a, b| {
        b.total_artifact_bytes
            .cmp(&a.total_artifact_bytes)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.path.cmp(&b.path))
    });

    let total_reclaimable_bytes = projects.iter().fold(0_u64, |acc, proj| {
        acc.saturating_add(proj.total_artifact_bytes)
    });

    DevProjectsReport {
        roots: report_roots,
        projects,
        total_reclaimable_bytes,
        scan_duration_ms: u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX),
    }
}

/// Validates and moves selected dev project artifact directories to the macOS Trash.
#[must_use]
pub fn trash_dev_artifacts<S: std::hash::BuildHasher>(
    paths: &[String],
    roots: &[String],
    known_sizes: &HashMap<String, u64, S>,
    engine: &RuleEngine,
) -> DevTrashResult {
    let mut trashed_paths = Vec::new();
    let mut failed = Vec::new();
    let mut freed_bytes = 0_u64;

    let mut normalized_inputs: Vec<(String, PathBuf)> = paths
        .iter()
        .map(|raw| (raw.clone(), normalize_path(Path::new(raw))))
        .collect();
    normalized_inputs.sort_by_key(|(_, path)| path.components().count());

    let mut accepted_normalized: Vec<PathBuf> = Vec::new();
    let mut accepted_canonical: Vec<PathBuf> = Vec::new();

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

        let canonical_path = match validate_dev_artifact_target(raw_candidate, roots, engine) {
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
            .any(|parent| path_starts_with_platform(&canonical_path, parent))
        {
            continue;
        }

        let estimated_size = known_sizes
            .get(&raw_path)
            .copied()
            .unwrap_or_else(|| dir_disk_size(&canonical_path));

        match trash::delete(&canonical_path) {
            Ok(()) => {
                freed_bytes = freed_bytes.saturating_add(estimated_size);
                accepted_normalized.push(normalized);
                accepted_canonical.push(canonical_path);
                trashed_paths.push(raw_path);
            }
            Err(err) => {
                failed.push(TrashFailure {
                    path: raw_path,
                    reason: err.to_string(),
                });
            }
        }
    }

    DevTrashResult {
        trashed_paths,
        freed_bytes,
        failed,
    }
}

fn canonical_validated_roots(roots: &[String], engine: &RuleEngine) -> Vec<PathBuf> {
    let mut validated: Vec<PathBuf> = Vec::new();
    for raw in roots {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(canonical) = validate_dev_root(Path::new(trimmed), engine) else {
            continue;
        };
        if !validated
            .iter()
            .any(|existing| paths_equal_platform(existing, &canonical))
        {
            validated.push(canonical);
        }
    }
    validated
}

fn build_ignored_paths(ignored_paths: &[String]) -> Vec<PathBuf> {
    let mut results = Vec::new();
    for raw in ignored_paths {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let path = Path::new(trimmed);
        if !path.is_absolute() {
            continue;
        }
        let normalized = normalize_path(path);
        results.push(normalized.clone());
        if let Ok(canonical) = fs::canonicalize(path) {
            if !paths_equal_platform(&canonical, &normalized) {
                results.push(canonical);
            }
        }
    }
    results
}

fn is_ignored_path(candidate: &Path, ignored_paths: &[PathBuf]) -> bool {
    ignored_paths
        .iter()
        .any(|ignored| path_starts_with_platform(candidate, ignored))
}

fn discover_artifacts_in_root(
    root: &Path,
    ignored_paths: Arc<Vec<PathBuf>>,
    engine: &RuleEngine,
) -> Vec<DiscoveredArtifact> {
    let engine_ref = engine.clone();
    let discovered: Arc<Mutex<Vec<DiscoveredArtifact>>> = Arc::new(Mutex::new(Vec::new()));
    let discovered_in_walk = Arc::clone(&discovered);

    let walker = WalkDir::new(root)
        .max_depth(MAX_SCAN_DEPTH)
        .follow_links(false)
        .skip_hidden(false)
        .process_read_dir(move |_depth, dir_path, _state, children| {
            let mut manifests = ParentManifests::default();
            for entry in children.iter().flatten() {
                if entry.file_type().is_file() {
                    manifests.record_file_name(entry.file_name());
                }
            }

            let mut matched_here: Vec<DiscoveredArtifact> = Vec::new();

            children.retain(|child_result| {
                let Ok(entry) = child_result else {
                    return false;
                };

                if entry.file_type().is_symlink() || !entry.file_type().is_dir() {
                    return false;
                }

                let file_name = entry.file_name();
                if file_name
                    .to_str()
                    .is_some_and(|name| name_eq_platform(name, ".git"))
                {
                    return false;
                }

                let entry_path = entry.path();
                if engine_ref.is_protected_normalized(&entry_path)
                    || is_ignored_path(&entry_path, &ignored_paths)
                {
                    return false;
                }

                if let Some(kind) = match_artifact_kind(file_name, &entry_path, manifests) {
                    matched_here.push(DiscoveredArtifact {
                        project_dir: dir_path.to_path_buf(),
                        name: file_name.to_string_lossy().into_owned(),
                        artifact_path: entry_path,
                        kind,
                    });
                    return false;
                }

                !file_name
                    .to_str()
                    .is_some_and(|name| name_eq_platform(name, "node_modules"))
            });

            if !matched_here.is_empty() {
                discovered_in_walk
                    .lock()
                    .expect("discovered lock poisoned")
                    .extend(matched_here);
            }
        });

    for _ in walker {}

    Arc::try_unwrap(discovered).map_or_else(
        |arc| std::mem::take(&mut *arc.lock().expect("discovered lock poisoned")),
        |mutex| mutex.into_inner().expect("discovered lock poisoned"),
    )
}

fn match_artifact_kind(
    leaf_name: &OsStr,
    artifact_path: &Path,
    manifests: ParentManifests,
) -> Option<ProjectKind> {
    let raw = leaf_name.to_str()?;

    if name_eq_platform(raw, "node_modules")
        || name_eq_platform(raw, ".next")
        || name_eq_platform(raw, ".nuxt")
        || name_eq_platform(raw, ".turbo")
        || name_eq_platform(raw, ".svelte-kit")
        || name_eq_platform(raw, ".parcel-cache")
    {
        return manifests
            .has(ParentManifests::PACKAGE_JSON)
            .then_some(ProjectKind::NodeJs);
    }

    if name_eq_platform(raw, "target") {
        if manifests.has(ParentManifests::CARGO_TOML) {
            return Some(ProjectKind::Rust);
        }
        if manifests.has(ParentManifests::POM_XML) {
            return Some(ProjectKind::JavaMaven);
        }
        return None;
    }

    if name_eq_platform(raw, ".venv") || name_eq_platform(raw, "venv") {
        return (manifests.has(ParentManifests::PYTHON_MANIFEST) || has_pyvenv_cfg(artifact_path))
            .then_some(ProjectKind::Python);
    }

    if name_eq_platform(raw, ".build") {
        return manifests
            .has(ParentManifests::PACKAGE_SWIFT)
            .then_some(ProjectKind::Swift);
    }

    if name_eq_platform(raw, "Pods") {
        return manifests
            .has(ParentManifests::PODFILE)
            .then_some(ProjectKind::IosCocoaPods);
    }

    if name_eq_platform(raw, ".dart_tool") {
        return manifests
            .has(ParentManifests::PUBSPEC_YAML)
            .then_some(ProjectKind::Flutter);
    }

    if name_eq_platform(raw, "build") {
        if manifests.has(ParentManifests::PUBSPEC_YAML) {
            return Some(ProjectKind::Flutter);
        }
        if manifests.has(ParentManifests::GRADLE) {
            return Some(ProjectKind::JavaGradle);
        }
        return None;
    }

    if name_eq_platform(raw, ".gradle") {
        return manifests
            .has(ParentManifests::GRADLE)
            .then_some(ProjectKind::JavaGradle);
    }

    if name_eq_platform(raw, "__pycache__")
        || name_eq_platform(raw, ".pytest_cache")
        || name_eq_platform(raw, ".mypy_cache")
        || name_eq_platform(raw, ".ruff_cache")
    {
        return (manifests.has(ParentManifests::PYTHON_MANIFEST)
            || manifests.has(ParentManifests::PY_FILE))
        .then_some(ProjectKind::Python);
    }

    None
}

fn is_known_artifact_leaf_name(name: &OsStr) -> bool {
    let Some(raw) = name.to_str() else {
        return false;
    };
    ARTIFACT_LEAVES
        .iter()
        .any(|candidate| name_eq_platform(raw, candidate))
}

fn has_pyvenv_cfg(venv_dir: &Path) -> bool {
    if fs::symlink_metadata(venv_dir.join("pyvenv.cfg"))
        .is_ok_and(|meta| meta.file_type().is_file())
    {
        return true;
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(entries) = fs::read_dir(venv_dir) {
            for entry in entries.flatten() {
                if entry.file_type().is_ok_and(|ft| ft.is_file())
                    && entry
                        .file_name()
                        .to_str()
                        .is_some_and(|name| name.eq_ignore_ascii_case("pyvenv.cfg"))
                {
                    return true;
                }
            }
        }
    }
    false
}

fn compute_project_inactive_days(project_dir: &Path, now: SystemTime) -> u32 {
    let mut latest_mtime: Option<SystemTime> = None;

    for git_signal in [".git/index", ".git/logs/HEAD"] {
        let signal_path = project_dir.join(git_signal);
        if let Ok(meta) = fs::symlink_metadata(&signal_path) {
            if meta.file_type().is_file() {
                if let Ok(mtime) = meta.modified() {
                    latest_mtime = Some(match latest_mtime {
                        Some(current) => current.max(mtime),
                        None => mtime,
                    });
                }
            }
        }
    }

    let walker = WalkDir::new(project_dir)
        .max_depth(MAX_MTIME_SCAN_DEPTH)
        .parallelism(jwalk::Parallelism::Serial)
        .follow_links(false)
        .skip_hidden(false)
        .process_read_dir(|_depth, _dir_path, _state, children| {
            children.retain(|child_result| {
                let Ok(entry) = child_result else {
                    return false;
                };
                if entry.file_type().is_symlink() {
                    return false;
                }
                let name = entry.file_name();
                if entry.file_type().is_dir() {
                    if name.to_str().is_some_and(|n| name_eq_platform(n, ".git"))
                        || is_known_artifact_leaf_name(name)
                    {
                        return false;
                    }
                } else if name
                    .to_str()
                    .is_some_and(|n| name_eq_platform(n, ".DS_Store"))
                {
                    return false;
                }
                true
            });
        });

    let mut visited = 0_usize;

    for entry_result in walker {
        let Ok(entry) = entry_result else {
            continue;
        };
        if !entry.file_type().is_file() {
            continue;
        }
        if entry
            .file_name()
            .to_str()
            .is_some_and(|n| name_eq_platform(n, ".DS_Store"))
        {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            if let Ok(mtime) = meta.modified() {
                latest_mtime = Some(match latest_mtime {
                    Some(current) => current.max(mtime),
                    None => mtime,
                });
            }
        }
        visited += 1;
        if visited >= MAX_MTIME_ENTRIES {
            break;
        }
    }

    if latest_mtime.is_none() {
        latest_mtime = fs::metadata(project_dir)
            .ok()
            .and_then(|meta| meta.modified().ok());
    }

    let days = days_since(latest_mtime, now);
    u32::try_from(days).unwrap_or(u32::MAX)
}

#[cfg(target_os = "macos")]
fn name_eq_platform(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

#[cfg(not(target_os = "macos"))]
fn name_eq_platform(a: &str, b: &str) -> bool {
    a == b
}

#[cfg(target_os = "macos")]
fn has_py_extension(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() > 3 && bytes[bytes.len() - 3..].eq_ignore_ascii_case(b".py")
}

#[cfg(not(target_os = "macos"))]
fn has_py_extension(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() > 3 && &bytes[bytes.len() - 3..] == b".py"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    fn unique_fake_home(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("macleaner_dev_test_{label}_{nanos}"));
        fs::create_dir_all(&dir).expect("create fake home");
        dir
    }

    fn engine_for_home(home: &Path) -> RuleEngine {
        RuleEngine::from_toml_str(include_str!("../rules/known.toml"), home)
            .expect("parse known rules")
    }

    #[test]
    fn validate_dev_root_enforces_home_and_sensitive_boundaries() {
        let home = unique_fake_home("root_validation");
        let projects = home.join("Projects");
        let docs_projects = home.join("Documents/Projects");
        let library_dev = home.join("Library/Developer");
        let ssh_dir = home.join(".ssh");
        let link_projects = home.join("LinkedProjects");
        let outside = std::env::temp_dir().join("macleaner_outside_dir");

        for dir in [&projects, &docs_projects, &library_dev, &ssh_dir, &outside] {
            fs::create_dir_all(dir).expect("create test dirs");
        }
        std::os::unix::fs::symlink(&projects, &link_projects).expect("create symlink");

        let engine = engine_for_home(&home);

        assert!(validate_dev_root(&projects, &engine).is_ok());
        assert!(validate_dev_root(&docs_projects, &engine).is_ok());
        assert!(validate_dev_root(&home, &engine).is_err());
        assert!(validate_dev_root(Path::new("/"), &engine).is_err());
        assert!(validate_dev_root(&library_dev, &engine).is_err());
        assert!(validate_dev_root(&ssh_dir, &engine).is_err());
        assert!(validate_dev_root(&link_projects, &engine).is_err());
        assert!(validate_dev_root(&outside, &engine).is_err());
        assert!(validate_dev_root(Path::new("Projects"), &engine).is_err());

        let _ = fs::remove_dir_all(&home);
        let _ = fs::remove_dir_all(&outside);
    }

    #[test]
    fn validate_dev_artifact_rejects_symlinks_missing_manifests_and_outside_roots() {
        let home = unique_fake_home("artifact_validation");
        let root = home.join("Projects");
        let outside_root = home.join("Other");

        let rust_proj = root.join("rust-app");
        let rust_target = rust_proj.join("target");
        let bare_target = root.join("not-rust/target");

        let node_proj = root.join("web-app");
        let node_modules = node_proj.join("node_modules");

        let venv_proj = root.join("py-app");
        let venv_dir = venv_proj.join(".venv");

        let bare_build = root.join("docs-site/build");
        let gradle_proj = root.join("android-app");
        let gradle_build = gradle_proj.join("build");

        let outside_proj = outside_root.join("external-node");
        let outside_nm = outside_proj.join("node_modules");

        for dir in [
            &rust_target,
            &bare_target,
            &node_modules,
            &venv_dir,
            &bare_build,
            &gradle_build,
            &outside_nm,
        ] {
            fs::create_dir_all(dir).expect("create dirs");
        }

        fs::write(rust_proj.join("Cargo.toml"), "[package]\nname = \"a\"\n").expect("cargo");
        fs::write(node_proj.join("package.json"), "{}").expect("pkg");
        fs::write(venv_dir.join("pyvenv.cfg"), "home = /usr/bin\n").expect("pyvenv");
        fs::write(gradle_proj.join("build.gradle.kts"), "").expect("gradle");
        fs::write(outside_proj.join("package.json"), "{}").expect("outside pkg");

        let symlink_nm = root.join("sym-app");
        fs::create_dir_all(&symlink_nm).expect("create sym-app");
        fs::write(symlink_nm.join("package.json"), "{}").expect("sym pkg");
        let sym_target = symlink_nm.join("node_modules");
        std::os::unix::fs::symlink(&node_modules, &sym_target).expect("create symlink");

        let engine = engine_for_home(&home);
        let roots = vec![root.to_string_lossy().into_owned()];

        assert!(validate_dev_artifact_target(&rust_target, &roots, &engine).is_ok());
        assert!(validate_dev_artifact_target(&node_modules, &roots, &engine).is_ok());
        assert!(validate_dev_artifact_target(&venv_dir, &roots, &engine).is_ok());
        assert!(validate_dev_artifact_target(&gradle_build, &roots, &engine).is_ok());

        assert!(validate_dev_artifact_target(&bare_target, &roots, &engine).is_err());
        assert!(validate_dev_artifact_target(&bare_build, &roots, &engine).is_err());
        assert!(validate_dev_artifact_target(&sym_target, &roots, &engine).is_err());
        assert!(validate_dev_artifact_target(&outside_nm, &roots, &engine).is_err());

        assert!(validate_dev_reveal_target(&rust_proj, &roots, &engine).is_ok());
        assert!(validate_dev_reveal_target(&rust_target, &roots, &engine).is_ok());
        assert!(validate_dev_reveal_target(&root, &roots, &engine).is_err());
        assert!(validate_dev_reveal_target(&outside_proj, &roots, &engine).is_err());

        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn scan_dev_projects_prunes_nested_node_modules_and_groups_by_project() {
        let home = unique_fake_home("scan_prune");
        let root = home.join("Projects");

        let web = root.join("my-web");
        let outer_nm = web.join("node_modules");
        let nested_pkg = outer_nm.join("some-pkg");
        let nested_nm = nested_pkg.join("node_modules");
        let next_cache = web.join(".next");

        let rust = root.join("my-rust");
        let rust_target = rust.join("target");

        let ignored_proj = root.join("ignored-app");
        let ignored_nm = ignored_proj.join("node_modules");

        for dir in [&nested_nm, &next_cache, &rust_target, &ignored_nm] {
            fs::create_dir_all(dir).expect("create dirs");
        }

        fs::write(web.join("package.json"), "{}").expect("web pkg");
        fs::write(nested_pkg.join("package.json"), "{}").expect("nested pkg");
        fs::write(nested_nm.join("dep.js"), vec![1_u8; 8192]).expect("nested file");
        fs::write(next_cache.join("chunk.js"), vec![2_u8; 4096]).expect("next file");

        fs::write(rust.join("Cargo.toml"), "[package]\nname = \"my-rust\"\n").expect("cargo");
        fs::write(rust_target.join("debug.o"), vec![3_u8; 16384]).expect("rust obj");

        fs::write(ignored_proj.join("package.json"), "{}").expect("ignored pkg");
        fs::write(ignored_nm.join("lib.js"), vec![4_u8; 32768]).expect("ignored file");

        let engine = engine_for_home(&home);
        let roots = vec![root.to_string_lossy().into_owned()];
        let ignored = vec![ignored_proj.to_string_lossy().into_owned()];

        let report = scan_dev_projects(&roots, &ignored, &engine);
        let _ = fs::remove_dir_all(&home);

        assert_eq!(report.projects.len(), 2);
        assert_eq!(report.projects[0].name, "my-rust");
        assert_eq!(report.projects[0].kind, "Rust");
        assert_eq!(report.projects[0].artifacts.len(), 1);
        assert_eq!(report.projects[0].artifacts[0].name, "target");

        assert_eq!(report.projects[1].name, "my-web");
        assert_eq!(report.projects[1].kind, "Node.js");
        assert_eq!(report.projects[1].artifacts.len(), 2);

        let all_artifact_paths: Vec<&str> = report
            .projects
            .iter()
            .flat_map(|p| p.artifacts.iter().map(|a| a.path.as_str()))
            .collect();
        assert!(
            !all_artifact_paths
                .iter()
                .any(|p| p.contains("some-pkg/node_modules")),
            "nested node_modules must never be emitted: {all_artifact_paths:?}"
        );
        assert!(report.total_reclaimable_bytes > 0);
    }

    #[test]
    fn detect_default_roots_in_returns_existing_real_dirs_only() {
        let home = unique_fake_home("default_roots");
        let projects = home.join("Projects");
        let code = home.join("Code");
        let link_repos = home.join("Repos");

        fs::create_dir_all(&projects).expect("create projects");
        fs::create_dir_all(&code).expect("create code");
        std::os::unix::fs::symlink(&projects, &link_repos).expect("create symlink");

        let detected = detect_default_roots_in(&home);
        let _ = fs::remove_dir_all(&home);

        assert_eq!(detected.len(), 2);
        assert!(detected.iter().any(|p| p.ends_with("/Projects")));
        assert!(detected.iter().any(|p| p.ends_with("/Code")));
    }

    #[test]
    fn trash_dev_artifacts_rejects_invalid_targets() {
        let home = unique_fake_home("trash_reject");
        let root = home.join("Projects");
        let bare_target = root.join("plain-dir/target");
        fs::create_dir_all(&bare_target).expect("create bare target");

        let engine = engine_for_home(&home);
        let roots = vec![root.to_string_lossy().into_owned()];
        let targets = vec![
            bare_target.to_string_lossy().into_owned(),
            "/System/Library".to_string(),
        ];

        let result = trash_dev_artifacts(&targets, &roots, &HashMap::new(), &engine);
        let _ = fs::remove_dir_all(&home);

        assert_eq!(result.trashed_paths, Vec::<String>::new());
        assert_eq!(result.freed_bytes, 0);
        assert_eq!(result.failed.len(), 2);
    }

    #[test]
    fn has_py_extension_handles_multibyte_utf8_without_panicking() {
        assert!(!has_py_extension("versão2"));
        assert!(!has_py_extension("café.c"));
        assert!(!has_py_extension("ação01"));
        assert!(!has_py_extension("🔥1"));
        assert!(!has_py_extension(".py"));
        assert!(!has_py_extension(""));
        assert!(has_py_extension("café.py"));
        assert!(has_py_extension("a.py"));
        #[cfg(target_os = "macos")]
        {
            assert!(has_py_extension("script.PY"));
            assert!(has_py_extension("ação.PY"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn dev_validators_reject_trailing_slash_symlinks_and_parent_dir() {
        let home = unique_fake_home("trailing_slash");
        let root = home.join("Projects");
        let proj = root.join("app");
        let real_nm = proj.join("node_modules");
        fs::create_dir_all(&real_nm).expect("create real nm");
        fs::write(proj.join("package.json"), "{}").expect("write pkg");

        let link_root = home.join("LinkRoot");
        std::os::unix::fs::symlink(&root, &link_root).expect("symlink root");

        let sym_proj = root.join("sym-app");
        fs::create_dir_all(&sym_proj).expect("create sym-app");
        fs::write(sym_proj.join("package.json"), "{}").expect("write sym pkg");
        let link_nm = sym_proj.join("node_modules");
        std::os::unix::fs::symlink(&real_nm, &link_nm).expect("symlink nm");

        let link_reveal = root.join("link-proj");
        std::os::unix::fs::symlink(&proj, &link_reveal).expect("symlink proj");

        let engine = engine_for_home(&home);
        let roots = vec![root.to_string_lossy().into_owned()];

        let link_root_slash = PathBuf::from(format!("{}/", link_root.to_string_lossy()));
        let link_nm_slash = PathBuf::from(format!("{}/", link_nm.to_string_lossy()));
        let link_reveal_slash = PathBuf::from(format!("{}/", link_reveal.to_string_lossy()));

        assert!(validate_dev_root(&link_root_slash, &engine).is_err());
        assert!(validate_dev_artifact_target(&link_nm_slash, &roots, &engine).is_err());
        assert!(validate_dev_reveal_target(&link_reveal_slash, &roots, &engine).is_err());
        assert!(validate_dev_root(&root.join("../Projects"), &engine).is_err());

        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn scan_dev_projects_prunes_orphan_node_modules_without_walking_inside() {
        let home = unique_fake_home("orphan_nm");
        let root = home.join("Projects");
        let orphan_nested_proj = root.join("orphan-container/node_modules/nested-rust");
        let nested_target = orphan_nested_proj.join("target");
        fs::create_dir_all(&nested_target).expect("create nested target");
        fs::write(
            orphan_nested_proj.join("Cargo.toml"),
            "[package]\nname = \"x\"\n",
        )
        .expect("write cargo");
        fs::write(nested_target.join("a.o"), vec![1_u8; 4096]).expect("write obj");

        let engine = engine_for_home(&home);
        let roots = vec![root.to_string_lossy().into_owned()];
        let report = scan_dev_projects(&roots, &[], &engine);
        let _ = fs::remove_dir_all(&home);

        assert!(
            report.projects.is_empty(),
            "orphan node_modules should be pruned without traversing inside"
        );
    }

    #[test]
    fn compute_project_inactive_days_checks_git_index_and_skips_ds_store() {
        let home = unique_fake_home("inactive_days");
        let proj = home.join("repo");
        let git_dir = proj.join(".git");
        fs::create_dir_all(&git_dir).expect("create .git");
        fs::write(git_dir.join("index"), b"git-index").expect("write git index");
        fs::write(proj.join(".DS_Store"), b"finder").expect("write ds_store");

        let days = compute_project_inactive_days(&proj, SystemTime::now());
        let _ = fs::remove_dir_all(&home);
        assert_eq!(days, 0);
    }
}
