//! Parallel filesystem scanner using jwalk and rayon for macOS directories.

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Instant, SystemTime};

use jwalk::{WalkDir, WalkDirGeneric};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use sysinfo::Disks;

use crate::rules::{
    classify_subpath, normalize_path, path_starts_with_platform, paths_equal_platform, RiskLevel,
    RuleEngine,
};

const BYTES_PER_MIB: u64 = 1024 * 1024;
const SECONDS_PER_DAY: u64 = 86_400;
const MAX_SUBITEMS: usize = 8;
const MAX_EXTENSIONS: usize = 5;

const SCAN_ROOT_SUFFIXES: &[&str] = &[
    "Library/Caches",
    "Library/Application Support",
    "Library/Developer",
    ".cache",
];

const DIRECT_TARGET_SUFFIXES: &[&str] = &[
    ".cargo/registry",
    ".npm",
    ".ollama/models",
    ".gradle/caches",
    ".m2/repository",
    ".orbstack",
    "Library/pnpm/store",
    "Library/Logs",
    "Library/Containers/com.docker.docker",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskHealth {
    pub volume_name: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    pub used_percent: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSubItem {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub file_count: u64,
    pub days_since_modified: u64,
    pub is_dir: bool,
    pub risk: RiskLevel,
    pub reason: String,
    pub safe_to_clean: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionStat {
    pub ext: String,
    pub size_bytes: u64,
    pub file_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanItem {
    pub id: String,
    pub name: String,
    pub path: String,
    pub category: String,
    pub size_bytes: u64,
    pub file_count: u64,
    pub days_since_modified: u64,
    pub risk: RiskLevel,
    pub reason: String,
    pub rule_id: Option<String>,
    pub ai_classified: bool,
    pub selected: bool,
    pub subitems: Vec<ScanSubItem>,
    pub cleanable_bytes: u64,
    pub top_extensions: Vec<ExtensionStat>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    pub items: Vec<ScanItem>,
    pub total_bytes: u64,
    pub safe_bytes: u64,
    pub item_count: usize,
    pub skipped_permission_count: u64,
    pub duration_ms: u64,
    pub ai_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashFailure {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashResult {
    pub trashed_paths: Vec<String>,
    pub failed_paths: Vec<TrashFailure>,
    pub freed_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "event",
    content = "data"
)]
pub enum ScanEvent {
    Started {
        total_targets: usize,
    },
    Progress {
        current_path: String,
        scanned_bytes: u64,
    },
    Finished {
        summary: ScanSummary,
    },
}

pub(crate) struct RawScanOutput {
    pub items: Vec<ScanItem>,
    pub skipped_permission_count: u64,
    pub started_at: Instant,
}

#[derive(Default)]
struct ChildStats {
    size_bytes: u64,
    file_count: u64,
    latest_modified: Option<SystemTime>,
    is_dir: bool,
}

impl ChildStats {
    fn record(&mut self, size_bytes: u64, modified: Option<SystemTime>, is_dir: bool) {
        self.size_bytes = self.size_bytes.saturating_add(size_bytes);
        self.file_count = self.file_count.saturating_add(1);
        self.is_dir |= is_dir;
        if let Some(mtime) = modified {
            self.latest_modified = Some(match self.latest_modified {
                Some(current) => current.max(mtime),
                None => mtime,
            });
        }
    }

    fn merge(&mut self, other: Self) {
        self.size_bytes = self.size_bytes.saturating_add(other.size_bytes);
        self.file_count = self.file_count.saturating_add(other.file_count);
        self.is_dir |= other.is_dir;
        if let Some(mtime) = other.latest_modified {
            self.latest_modified = Some(match self.latest_modified {
                Some(current) => current.max(mtime),
                None => mtime,
            });
        }
    }
}

#[derive(Default)]
struct DirStats {
    size_bytes: u64,
    file_count: u64,
    latest_modified: Option<SystemTime>,
    depth1: HashMap<OsString, ChildStats>,
    depth2: HashMap<OsString, HashMap<OsString, ChildStats>>,
    extensions: HashMap<String, (u64, u64)>,
}

impl DirStats {
    fn record_extension(&mut self, file_path: &Path, size_bytes: u64) {
        let Some(ext) = file_path.extension().and_then(|e| e.to_str()) else {
            return;
        };
        if ext.is_empty() || ext.len() > 16 {
            return;
        }
        if ext.is_ascii() {
            let mut buf = [0_u8; 17];
            buf[0] = b'.';
            for (dst, src) in buf[1..=ext.len()].iter_mut().zip(ext.bytes()) {
                *dst = src.to_ascii_lowercase();
            }
            if let Ok(key) = std::str::from_utf8(&buf[..=ext.len()]) {
                if let Some(entry) = self.extensions.get_mut(key) {
                    entry.0 = entry.0.saturating_add(size_bytes);
                    entry.1 = entry.1.saturating_add(1);
                } else {
                    self.extensions.insert(key.to_owned(), (size_bytes, 1));
                }
                return;
            }
        }
        let key = format!(".{}", ext.to_lowercase());
        let entry = self.extensions.entry(key).or_insert((0, 0));
        entry.0 = entry.0.saturating_add(size_bytes);
        entry.1 = entry.1.saturating_add(1);
    }

    fn record_file(
        &mut self,
        bucket: &Path,
        file_path: &Path,
        size_bytes: u64,
        modified: Option<SystemTime>,
    ) {
        self.size_bytes = self.size_bytes.saturating_add(size_bytes);
        self.file_count = self.file_count.saturating_add(1);
        if let Some(mtime) = modified {
            self.latest_modified = Some(match self.latest_modified {
                Some(current) => current.max(mtime),
                None => mtime,
            });
        }

        self.record_extension(file_path, size_bytes);

        let Ok(rel) = file_path.strip_prefix(bucket) else {
            return;
        };
        let mut comps = rel.components().filter_map(|c| match c {
            Component::Normal(part) => Some(part),
            _ => None,
        });

        let Some(first) = comps.next() else {
            return;
        };
        let second = comps.next();
        let third = comps.next();

        if let Some(d1) = self.depth1.get_mut(first) {
            d1.record(size_bytes, modified, second.is_some());
        } else {
            let mut stats = ChildStats::default();
            stats.record(size_bytes, modified, second.is_some());
            self.depth1.insert(first.to_os_string(), stats);
        }

        if let Some(second_part) = second {
            let d2_map = if let Some(map) = self.depth2.get_mut(first) {
                map
            } else {
                self.depth2.entry(first.to_os_string()).or_default()
            };
            if let Some(d2) = d2_map.get_mut(second_part) {
                d2.record(size_bytes, modified, third.is_some());
            } else {
                let mut stats = ChildStats::default();
                stats.record(size_bytes, modified, third.is_some());
                d2_map.insert(second_part.to_os_string(), stats);
            }
        }
    }

    fn merge(&mut self, other: Self) {
        self.size_bytes = self.size_bytes.saturating_add(other.size_bytes);
        self.file_count = self.file_count.saturating_add(other.file_count);
        if let Some(mtime) = other.latest_modified {
            self.latest_modified = Some(match self.latest_modified {
                Some(current) => current.max(mtime),
                None => mtime,
            });
        }

        for (d1_name, stats) in other.depth1 {
            self.depth1.entry(d1_name).or_default().merge(stats);
        }
        for (d1_name, d2_map) in other.depth2 {
            let target = self.depth2.entry(d1_name).or_default();
            for (d2_name, stats) in d2_map {
                target.entry(d2_name).or_default().merge(stats);
            }
        }
        for (ext, (bytes, count)) in other.extensions {
            let entry = self.extensions.entry(ext).or_insert((0, 0));
            entry.0 = entry.0.saturating_add(bytes);
            entry.1 = entry.1.saturating_add(count);
        }
    }
}

#[must_use]
pub(crate) fn read_disk_health() -> DiskHealth {
    let disks = Disks::new_with_refreshed_list();
    let chosen = disks
        .iter()
        .find(|disk| disk.mount_point() == Path::new("/System/Volumes/Data"))
        .or_else(|| {
            disks
                .iter()
                .find(|disk| disk.mount_point() == Path::new("/"))
        })
        .or_else(|| disks.iter().find(|disk| disk.total_space() > 0));

    let Some(disk) = chosen else {
        return DiskHealth {
            volume_name: "Macintosh HD".to_string(),
            total_bytes: 0,
            available_bytes: 0,
            used_bytes: 0,
            used_percent: 0,
        };
    };

    let raw_name = disk.name().to_string_lossy();
    let volume_name = if raw_name.trim().is_empty() || raw_name == "Data" {
        "Macintosh HD".to_string()
    } else {
        raw_name.into_owned()
    };

    let total_bytes = disk.total_space();
    let available_bytes = disk.available_space();
    let used_bytes = total_bytes.saturating_sub(available_bytes);
    let used_percent = if total_bytes == 0 {
        0
    } else {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "percentage is clamped to 0.0..=100.0 before u8 cast"
        )]
        let pct = ((used_bytes as f64 / total_bytes as f64) * 100.0)
            .round()
            .clamp(0.0, 100.0) as u8;
        pct
    };

    DiskHealth {
        volume_name,
        total_bytes,
        available_bytes,
        used_bytes,
        used_percent,
    }
}

pub(crate) fn default_scan_roots(home_dir: &Path) -> Vec<PathBuf> {
    SCAN_ROOT_SUFFIXES
        .iter()
        .map(|suffix| normalize_path(&home_dir.join(suffix)))
        .collect()
}

pub(crate) fn default_direct_targets(home_dir: &Path) -> Vec<PathBuf> {
    DIRECT_TARGET_SUFFIXES
        .iter()
        .map(|suffix| normalize_path(&home_dir.join(suffix)))
        .collect()
}

pub(crate) fn scan_with_engine(
    engine: &RuleEngine,
    min_mb: u64,
    on_event: &(impl Fn(ScanEvent) + Sync),
) -> RawScanOutput {
    let roots = default_scan_roots(engine.home_dir());
    let direct_targets = default_direct_targets(engine.home_dir());
    scan_roots_and_direct(engine, &roots, &direct_targets, min_mb, on_event)
}

pub(crate) fn scan_roots_and_direct(
    engine: &RuleEngine,
    roots: &[PathBuf],
    direct_targets: &[PathBuf],
    min_mb: u64,
    on_event: &(impl Fn(ScanEvent) + Sync),
) -> RawScanOutput {
    let started_at = Instant::now();
    let min_bytes = min_mb.saturating_mul(BYTES_PER_MIB);

    let active_roots: Vec<PathBuf> = roots
        .iter()
        .map(|root| normalize_path(root))
        .filter(|root| {
            fs::symlink_metadata(root).is_ok_and(|meta| meta.file_type().is_dir())
                && !engine.is_protected_path(root)
        })
        .collect();

    let active_direct: Vec<PathBuf> = direct_targets
        .iter()
        .map(|target| normalize_path(target))
        .filter(|target| {
            fs::symlink_metadata(target).is_ok_and(|meta| meta.file_type().is_dir())
                && !engine.is_protected_path(target)
                && !active_roots
                    .iter()
                    .any(|root| path_starts_with_platform(target, root))
        })
        .collect();

    on_event(ScanEvent::Started {
        total_targets: active_roots.len() + active_direct.len(),
    });

    let skipped_counter = Arc::new(AtomicU64::new(0));
    let scanned_bytes_counter = Arc::new(AtomicU64::new(0));

    let (root_maps, direct_maps) = rayon::join(
        || -> Vec<HashMap<PathBuf, DirStats>> {
            active_roots
                .par_iter()
                .map(|root| {
                    scan_single_root(
                        engine,
                        root,
                        Arc::clone(&skipped_counter),
                        Arc::clone(&scanned_bytes_counter),
                        on_event,
                    )
                })
                .collect()
        },
        || -> Vec<(PathBuf, DirStats)> {
            active_direct
                .par_iter()
                .map(|target| {
                    let stats = scan_direct_target(
                        engine,
                        target,
                        Arc::clone(&skipped_counter),
                        Arc::clone(&scanned_bytes_counter),
                        on_event,
                    );
                    (target.clone(), stats)
                })
                .collect()
        },
    );

    let mut aggregated: HashMap<PathBuf, DirStats> = HashMap::new();
    for map in root_maps {
        for (dir_path, stats) in map {
            aggregated.entry(dir_path).or_default().merge(stats);
        }
    }
    for (dir_path, stats) in direct_maps {
        if stats.size_bytes > 0 {
            aggregated.entry(dir_path).or_default().merge(stats);
        }
    }

    let now = SystemTime::now();
    let mut items: Vec<ScanItem> = aggregated
        .into_par_iter()
        .filter(|(_, stats)| stats.size_bytes > 0 && stats.size_bytes >= min_bytes)
        .map(|(dir_path, stats)| build_scan_item(engine, &dir_path, &stats, now))
        .collect();

    let item_paths: Vec<PathBuf> = items.iter().map(|i| PathBuf::from(&i.path)).collect();
    for (idx, item) in items.iter_mut().enumerate() {
        let candidate = &item_paths[idx];
        let has_split_child = item_paths.iter().enumerate().any(|(other_idx, other)| {
            other_idx != idx
                && path_starts_with_platform(other, candidate)
                && !paths_equal_platform(other, candidate)
        });
        if has_split_child {
            item.risk = RiskLevel::Alto;
            item.selected = false;
            item.cleanable_bytes = 0;
        }
    }

    items.sort_by(|a, b| {
        b.size_bytes
            .cmp(&a.size_bytes)
            .then_with(|| a.name.cmp(&b.name))
    });

    RawScanOutput {
        items,
        skipped_permission_count: skipped_counter.load(Ordering::Relaxed),
        started_at,
    }
}

pub(crate) fn summarize_scan(
    items: Vec<ScanItem>,
    skipped_permission_count: u64,
    started_at: Instant,
    ai_available: bool,
) -> ScanSummary {
    let total_bytes = items.iter().map(|item| item.size_bytes).sum();
    let safe_bytes = items.iter().map(|item| item.cleanable_bytes).sum();
    let item_count = items.len();

    ScanSummary {
        items,
        total_bytes,
        safe_bytes,
        item_count,
        skipped_permission_count,
        duration_ms: u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX),
        ai_available,
    }
}

fn is_within_cleanable_scope(engine: &RuleEngine, candidate: &Path) -> bool {
    let resolved = fs::canonicalize(candidate).unwrap_or_else(|_| normalize_path(candidate));
    let home = fs::canonicalize(engine.home_dir())
        .unwrap_or_else(|_| normalize_path(engine.home_dir()));

    if !path_starts_with_platform(&resolved, &home) || paths_equal_platform(&resolved, &home) {
        return false;
    }

    let roots = default_scan_roots(&home);
    for root in &roots {
        let canonical_root = fs::canonicalize(root).unwrap_or_else(|_| root.clone());
        if path_starts_with_platform(&resolved, &canonical_root)
            && !paths_equal_platform(&resolved, &canonical_root)
        {
            return true;
        }
    }

    let direct_targets = default_direct_targets(&home);
    for target in &direct_targets {
        let canonical_target = fs::canonicalize(target).unwrap_or_else(|_| target.clone());
        if path_starts_with_platform(&resolved, &canonical_target) {
            return true;
        }
    }

    false
}

pub(crate) fn move_items_to_trash(
    engine: &RuleEngine,
    paths: &[String],
    known_sizes: &HashMap<String, u64>,
) -> TrashResult {
    let mut trashed_paths = Vec::new();
    let mut failed_paths = Vec::new();
    let mut freed_bytes = 0_u64;

    let mut normalized_inputs: Vec<(String, PathBuf)> = paths
        .iter()
        .map(|raw| (raw.clone(), normalize_path(Path::new(raw))))
        .collect();
    normalized_inputs.sort_by_key(|(_, p)| p.components().count());

    let mut accepted_paths: Vec<PathBuf> = Vec::new();

    for (raw_path, normalized) in normalized_inputs {
        let candidate = Path::new(&raw_path);
        if !candidate.is_absolute() {
            failed_paths.push(TrashFailure {
                path: raw_path,
                reason: "Caminho protegido pela política do sistema.".to_string(),
            });
            continue;
        }

        if engine.is_protected_or_ancestor(candidate) {
            failed_paths.push(TrashFailure {
                path: raw_path,
                reason: "Caminho protegido pela política do sistema.".to_string(),
            });
            continue;
        }

        let meta = match fs::symlink_metadata(candidate) {
            Ok(meta) => meta,
            Err(_) => {
                failed_paths.push(TrashFailure {
                    path: raw_path,
                    reason: "Item não encontrado no disco.".to_string(),
                });
                continue;
            }
        };

        if meta.file_type().is_symlink() {
            failed_paths.push(TrashFailure {
                path: raw_path,
                reason: "Caminho protegido pela política do sistema.".to_string(),
            });
            continue;
        }

        if !is_within_cleanable_scope(engine, candidate) {
            failed_paths.push(TrashFailure {
                path: raw_path,
                reason: "Caminho protegido pela política do sistema.".to_string(),
            });
            continue;
        }

        if accepted_paths
            .iter()
            .any(|parent| path_starts_with_platform(&normalized, parent))
        {
            continue;
        }

        match trash::delete(candidate) {
            Ok(()) => {
                freed_bytes =
                    freed_bytes.saturating_add(known_sizes.get(&raw_path).copied().unwrap_or(0));
                accepted_paths.push(normalized);
                trashed_paths.push(raw_path);
            }
            Err(err) => {
                failed_paths.push(TrashFailure {
                    path: raw_path,
                    reason: err.to_string(),
                });
            }
        }
    }

    TrashResult {
        trashed_paths,
        failed_paths,
        freed_bytes,
    }
}

#[must_use]
pub(crate) fn sanitize_relative_path(path: &Path, home_dir: &Path) -> String {
    if let Ok(stripped) = path.strip_prefix(home_dir) {
        return format!("~/{}", stripped.to_string_lossy());
    }
    path.to_string_lossy().into_owned()
}

#[cfg(target_os = "macos")]
fn effective_file_size(meta: &std::fs::Metadata) -> u64 {
    use std::os::macos::fs::MetadataExt;
    const SF_DATALESS: u32 = 0x4000_0000;
    const UF_COMPRESSED: u32 = 0x0000_0020;
    const SMALL_INLINE_THRESHOLD: u64 = 64 * 1024;

    if (meta.st_flags() & SF_DATALESS) != 0 {
        return 0;
    }
    let apparent = meta.st_size();
    let allocated = meta.st_blocks().saturating_mul(512);
    if allocated > 0 {
        apparent.min(allocated)
    } else if (meta.st_flags() & UF_COMPRESSED) != 0 || apparent <= SMALL_INLINE_THRESHOLD {
        apparent
    } else {
        0
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn effective_file_size(meta: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    const SMALL_INLINE_THRESHOLD: u64 = 64 * 1024;
    let apparent = meta.len();
    let allocated = meta.blocks().saturating_mul(512);
    if allocated > 0 {
        apparent.min(allocated)
    } else if apparent <= SMALL_INLINE_THRESHOLD {
        apparent
    } else {
        0
    }
}

#[cfg(not(unix))]
fn effective_file_size(meta: &std::fs::Metadata) -> u64 {
    meta.len()
}

fn scan_single_root(
    engine: &RuleEngine,
    root: &Path,
    skipped_counter: Arc<AtomicU64>,
    scanned_bytes_counter: Arc<AtomicU64>,
    on_event: &(impl Fn(ScanEvent) + Sync),
) -> HashMap<PathBuf, DirStats> {
    let engine_ref = engine.clone();
    let skipped_in_walk = Arc::clone(&skipped_counter);
    let root_path = root.to_path_buf();

    let walker = WalkDirGeneric::<(Option<Arc<PathBuf>>, Option<Arc<PathBuf>>)>::new(root)
        .follow_links(false)
        .skip_hidden(false)
        .process_read_dir(move |_depth, dir_path, read_dir_state, children| {
            if dir_path == root_path.as_path() {
                *read_dir_state = None;
            } else if read_dir_state.is_none() || engine_ref.classify_path(dir_path).is_some() {
                *read_dir_state = Some(Arc::new(dir_path.to_path_buf()));
            }

            let current_bucket = read_dir_state.as_ref();

            children.retain_mut(|child_result| {
                let Ok(entry) = child_result else {
                    if let Err(err) = child_result {
                        if err
                            .io_error()
                            .is_some_and(|io_err| is_ignorable_io_error(io_err.kind()))
                        {
                            skipped_in_walk.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    return false;
                };

                if entry.file_type().is_symlink() {
                    return false;
                }

                if engine_ref.is_protected_normalized(&entry.path()) {
                    return false;
                }

                if entry.file_type().is_file() {
                    let Some(bucket) = current_bucket else {
                        return false;
                    };
                    entry.client_state = Some(Arc::clone(bucket));
                }

                true
            });
        });

    let mut buckets: HashMap<PathBuf, DirStats> = HashMap::new();
    let mut total_root_bytes = 0_u64;

    for entry_result in walker {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(err) => {
                if err
                    .io_error()
                    .is_some_and(|io_err| is_ignorable_io_error(io_err.kind()))
                {
                    skipped_counter.fetch_add(1, Ordering::Relaxed);
                }
                continue;
            }
        };

        if let Some(ref read_err) = entry.read_children_error {
            if read_err
                .io_error()
                .is_some_and(|io_err| is_ignorable_io_error(io_err.kind()))
            {
                skipped_counter.fetch_add(1, Ordering::Relaxed);
            }
        }

        if !entry.file_type().is_file() {
            continue;
        }

        let Some(bucket) = entry.client_state.as_ref() else {
            continue;
        };

        let meta = match entry.metadata() {
            Ok(meta) => meta,
            Err(err) => {
                if err
                    .io_error()
                    .is_some_and(|io_err| is_ignorable_io_error(io_err.kind()))
                {
                    skipped_counter.fetch_add(1, Ordering::Relaxed);
                }
                continue;
            }
        };

        let size = effective_file_size(&meta);
        let mtime = meta.modified().ok();
        let file_path = entry.path();
        total_root_bytes = total_root_bytes.saturating_add(size);

        if let Some(stats) = buckets.get_mut(bucket.as_path()) {
            stats.record_file(bucket.as_path(), &file_path, size, mtime);
        } else {
            let mut stats = DirStats::default();
            stats.record_file(bucket.as_path(), &file_path, size, mtime);
            buckets.insert((**bucket).clone(), stats);
        }
    }

    let total_scanned =
        scanned_bytes_counter.fetch_add(total_root_bytes, Ordering::Relaxed) + total_root_bytes;
    on_event(ScanEvent::Progress {
        current_path: sanitize_relative_path(root, engine.home_dir()),
        scanned_bytes: total_scanned,
    });

    buckets
}

fn scan_direct_target(
    engine: &RuleEngine,
    target: &Path,
    skipped_counter: Arc<AtomicU64>,
    scanned_bytes_counter: Arc<AtomicU64>,
    on_event: &(impl Fn(ScanEvent) + Sync),
) -> DirStats {
    let engine_ref = engine.clone();
    let skipped_in_walk = Arc::clone(&skipped_counter);

    let walker = WalkDir::new(target)
        .follow_links(false)
        .skip_hidden(false)
        .process_read_dir(move |_depth, _dir_path, _state, children| {
            children.retain(|child_result| {
                let Ok(entry) = child_result else {
                    if let Err(err) = child_result {
                        if err
                            .io_error()
                            .is_some_and(|io_err| is_ignorable_io_error(io_err.kind()))
                        {
                            skipped_in_walk.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    return false;
                };

                if entry.file_type().is_symlink() {
                    return false;
                }

                !engine_ref.is_protected_normalized(&entry.path())
            });
        });

    let mut stats = DirStats::default();
    for entry_result in walker {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(err) => {
                if err
                    .io_error()
                    .is_some_and(|io_err| is_ignorable_io_error(io_err.kind()))
                {
                    skipped_counter.fetch_add(1, Ordering::Relaxed);
                }
                continue;
            }
        };

        if let Some(ref read_err) = entry.read_children_error {
            if read_err
                .io_error()
                .is_some_and(|io_err| is_ignorable_io_error(io_err.kind()))
            {
                skipped_counter.fetch_add(1, Ordering::Relaxed);
            }
        }

        if !entry.file_type().is_file() {
            continue;
        }

        let meta = match entry.metadata() {
            Ok(meta) => meta,
            Err(err) => {
                if err
                    .io_error()
                    .is_some_and(|io_err| is_ignorable_io_error(io_err.kind()))
                {
                    skipped_counter.fetch_add(1, Ordering::Relaxed);
                }
                continue;
            }
        };

        let size = effective_file_size(&meta);
        stats.record_file(target, &entry.path(), size, meta.modified().ok());
    }

    let total_scanned =
        scanned_bytes_counter.fetch_add(stats.size_bytes, Ordering::Relaxed) + stats.size_bytes;
    on_event(ScanEvent::Progress {
        current_path: sanitize_relative_path(target, engine.home_dir()),
        scanned_bytes: total_scanned,
    });

    stats
}

fn build_scan_item(
    engine: &RuleEngine,
    dir_path: &Path,
    stats: &DirStats,
    now: SystemTime,
) -> ScanItem {
    let path_str = dir_path.to_string_lossy().into_owned();
    let fallback_name = dir_path.file_name().map_or_else(
        || path_str.clone(),
        |name| name.to_string_lossy().into_owned(),
    );
    let days_since_modified = days_since(stats.latest_modified, now);

    let matched_rule = engine.classify_path(dir_path);
    let subitems = build_subitems(
        dir_path,
        stats,
        now,
        matched_rule.map(|r| (r.risk, r.reason.as_str())),
    );
    let top_extensions = build_top_extensions(stats);

    if let Some(rule) = matched_rule {
        let has_high_sub = subitems.iter().any(|s| matches!(s.risk, RiskLevel::Alto));
        let risk = if has_high_sub && matches!(rule.risk, RiskLevel::Baixo) {
            RiskLevel::Medio
        } else {
            rule.risk
        };
        let selected = rule.selected_by_default && !has_high_sub;
        let cleanable_bytes = compute_cleanable_bytes(risk, stats.size_bytes, &subitems);
        return ScanItem {
            id: format!("{}:{}", rule.id, path_str),
            name: rule.name.clone(),
            path: path_str,
            category: rule.category.clone(),
            size_bytes: stats.size_bytes,
            file_count: stats.file_count,
            days_since_modified,
            risk,
            reason: rule.reason.clone(),
            rule_id: Some(rule.id.clone()),
            ai_classified: false,
            selected,
            subitems,
            cleanable_bytes,
            top_extensions,
        };
    }

    let cleanable_bytes = compute_cleanable_bytes(RiskLevel::Alto, stats.size_bytes, &subitems);
    ScanItem {
        id: format!("unclassified:{path_str}"),
        name: fallback_name,
        path: path_str,
        category: infer_category(dir_path),
        size_bytes: stats.size_bytes,
        file_count: stats.file_count,
        days_since_modified,
        risk: RiskLevel::Alto,
        reason: summarize_unknown_subitems(&subitems),
        rule_id: None,
        ai_classified: false,
        selected: false,
        subitems,
        cleanable_bytes,
        top_extensions,
    }
}

fn build_subitems(
    bucket: &Path,
    stats: &DirStats,
    now: SystemTime,
    parent_rule: Option<(RiskLevel, &str)>,
) -> Vec<ScanSubItem> {
    let mut candidates: Vec<(PathBuf, &ChildStats)> = Vec::new();

    for (d1_name, d1_stats) in &stats.depth1 {
        if d1_stats.size_bytes == 0 {
            continue;
        }
        let d1_path = bucket.join(d1_name);
        let expanded = d1_stats.is_dir
            && d1_stats.size_bytes.saturating_mul(2) >= stats.size_bytes
            && stats
                .depth2
                .get(d1_name)
                .is_some_and(|map| map.values().filter(|s| s.size_bytes > 0).count() >= 2);

        if expanded {
            if let Some(d2_map) = stats.depth2.get(d1_name) {
                for (d2_name, d2_stats) in d2_map {
                    if d2_stats.size_bytes > 0 {
                        candidates.push((d1_path.join(d2_name), d2_stats));
                    }
                }
                continue;
            }
        }

        candidates.push((d1_path, d1_stats));
    }

    candidates.sort_by(|(p_a, s_a), (p_b, s_b)| {
        s_b.size_bytes
            .cmp(&s_a.size_bytes)
            .then_with(|| p_a.cmp(p_b))
    });

    let has_mib_children = candidates
        .iter()
        .any(|(_, s)| s.size_bytes >= BYTES_PER_MIB);
    let filtered = candidates
        .into_iter()
        .filter(|(_, s)| !has_mib_children || s.size_bytes >= BYTES_PER_MIB)
        .take(MAX_SUBITEMS);

    filtered
        .map(|(sub_path, sub_stats)| {
            let rel_name = sub_path.strip_prefix(bucket).map_or_else(
                |_| sub_path.to_string_lossy().into_owned(),
                |rel| rel.to_string_lossy().replace('\\', "/"),
            );

            let (risk, reason, safe_to_clean) = match classify_subpath(&rel_name) {
                Some((r, why, safe)) => (r, why.to_string(), safe),
                None => match parent_rule {
                    Some((RiskLevel::Baixo, parent_reason)) => {
                        (RiskLevel::Baixo, parent_reason.to_string(), true)
                    }
                    _ => (
                        RiskLevel::Alto,
                        "Subpasta sem regra conhecida — revise antes de remover.".to_string(),
                        false,
                    ),
                },
            };

            ScanSubItem {
                name: rel_name,
                path: sub_path.to_string_lossy().into_owned(),
                size_bytes: sub_stats.size_bytes,
                file_count: sub_stats.file_count,
                days_since_modified: days_since(sub_stats.latest_modified, now),
                is_dir: sub_stats.is_dir,
                risk,
                reason,
                safe_to_clean,
            }
        })
        .collect()
}

fn build_top_extensions(stats: &DirStats) -> Vec<ExtensionStat> {
    let mut entries: Vec<ExtensionStat> = stats
        .extensions
        .iter()
        .map(|(ext, (size_bytes, file_count))| ExtensionStat {
            ext: ext.clone(),
            size_bytes: *size_bytes,
            file_count: *file_count,
        })
        .collect();

    entries.sort_by(|a, b| {
        b.size_bytes
            .cmp(&a.size_bytes)
            .then_with(|| a.ext.cmp(&b.ext))
    });
    entries.truncate(MAX_EXTENSIONS);
    entries
}

pub(crate) fn compute_cleanable_bytes(
    parent_risk: RiskLevel,
    total_bytes: u64,
    subitems: &[ScanSubItem],
) -> u64 {
    if matches!(parent_risk, RiskLevel::Baixo)
        && !subitems
            .iter()
            .any(|sub| matches!(sub.risk, RiskLevel::Alto))
    {
        return total_bytes;
    }
    subitems
        .iter()
        .filter(|sub| sub.safe_to_clean)
        .map(|sub| sub.size_bytes)
        .sum()
}

fn summarize_unknown_subitems(subitems: &[ScanSubItem]) -> String {
    if subitems.is_empty() {
        return "Pasta sem regra conhecida.".to_string();
    }
    let summary_parts: Vec<String> = subitems
        .iter()
        .take(2)
        .map(|sub| format!("{} ({})", sub.name, format_human_bytes(sub.size_bytes)))
        .collect();
    format!("Principais subpastas: {}.", summary_parts.join(", "))
}

fn format_human_bytes(bytes: u64) -> String {
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    #[expect(clippy::cast_precision_loss, reason = "display formatting only")]
    let b = bytes as f64;
    if b >= GIB {
        format!("{:.1} GB", b / GIB)
    } else {
        format!("{:.0} MB", (b / MIB).max(1.0))
    }
}

fn days_since(latest_modified: Option<SystemTime>, now: SystemTime) -> u64 {
    latest_modified
        .and_then(|mtime| now.duration_since(mtime).ok())
        .map_or(0, |duration| duration.as_secs() / SECONDS_PER_DAY)
}

fn infer_category(path: &Path) -> String {
    let text = path.to_string_lossy().to_lowercase();
    if text.contains("library/developer") {
        "developer".to_string()
    } else if text.contains("library/caches") || text.contains(".cache") {
        "caches".to_string()
    } else if text.contains("library/logs") {
        "logs".to_string()
    } else {
        "apps".to_string()
    }
}

fn is_ignorable_io_error(kind: ErrorKind) -> bool {
    matches!(kind, ErrorKind::PermissionDenied | ErrorKind::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::time::UNIX_EPOCH;

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("macleaner_test_{label}_{nanos}"));
        fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    #[test]
    fn scanner_extracts_subfolders_extensions_and_cleanable_bytes() {
        let root = unique_temp_dir("steam_hierarchy");
        let steam = root.join("Library/Application Support/Steam");
        let common = steam.join("steamapps/common/Hades");
        let shader = steam.join("steamapps/shadercache/1145360");
        let appcache = steam.join("appcache");

        for dir in [&common, &shader, &appcache] {
            fs::create_dir_all(dir).expect("create steam subdirs");
        }

        fs::write(common.join("game.pak"), vec![0_u8; 4 * 1024 * 1024]).expect("write game");
        fs::write(shader.join("dx.shader"), vec![0_u8; 2 * 1024 * 1024]).expect("write shader");
        fs::write(appcache.join("http.bin"), vec![0_u8; 1_200_000]).expect("write appcache");

        let engine = RuleEngine::from_toml_str(include_str!("../rules/known.toml"), &root)
            .expect("parse rules");
        let output = scan_with_engine(&engine, 1, &|_| {});
        let _ = fs::remove_dir_all(&root);

        assert_eq!(output.items.len(), 1);
        let steam_item = &output.items[0];
        assert_eq!(steam_item.name, "Steam");
        assert!(steam_item
            .subitems
            .iter()
            .any(|s| s.name == "steamapps/common" && !s.safe_to_clean));
        assert!(steam_item
            .subitems
            .iter()
            .any(|s| s.name == "steamapps/shadercache" && s.safe_to_clean));
        assert!(steam_item
            .subitems
            .iter()
            .any(|s| s.name == "appcache" && s.safe_to_clean));
        assert_eq!(steam_item.cleanable_bytes, 2 * 1024 * 1024 + 1_200_000);
        assert_eq!(steam_item.top_extensions[0].ext, ".pak");
    }

    #[test]
    fn scanner_filters_by_min_mb_classifies_rules_and_skips_permission_denied() {
        let root = unique_temp_dir("jwalk_scan");
        let caches = root.join("Library/Caches");
        let dev = root.join("Library/Developer");
        let app_support = root.join("Library/Application Support");
        let dot_cache = root.join(".cache");
        let cargo_reg = root.join(".cargo/registry");

        let brew = caches.join("Homebrew");
        let small_cache = caches.join("TinyCache");
        let locked_dir = caches.join("LockedSipLike");
        let derived = dev.join("Xcode/DerivedData/MyApp");
        let slack_cache = app_support.join("Slack/Cache");
        let hf_hub = dot_cache.join("huggingface/hub");
        let mail_tcc = root.join("Library/Mail");

        for dir in [
            &brew,
            &small_cache,
            &locked_dir,
            &derived,
            &slack_cache,
            &hf_hub,
            &mail_tcc,
            &cargo_reg,
        ] {
            fs::create_dir_all(dir).expect("create dir");
        }

        fs::write(brew.join("pkg.tar.gz"), vec![0_u8; 2 * 1024 * 1024]).expect("write brew");
        fs::write(derived.join("build.o"), vec![0_u8; 3 * 1024 * 1024]).expect("write derived");
        fs::write(slack_cache.join("img.bin"), vec![0_u8; 1_500_000]).expect("write slack");
        fs::write(hf_hub.join("weights.bin"), vec![0_u8; 2_500_000]).expect("write hf");
        fs::write(cargo_reg.join("crate.crate"), vec![0_u8; 1_800_000]).expect("write cargo");
        fs::write(small_cache.join("tiny.bin"), vec![0_u8; 200_000]).expect("write tiny");
        fs::write(mail_tcc.join("mail.mbox"), vec![0_u8; 5 * 1024 * 1024]).expect("write mail");

        let mut perms = fs::metadata(&locked_dir).expect("meta").permissions();
        perms.set_mode(0o000);
        fs::set_permissions(&locked_dir, perms).expect("lock dir");

        let toml = include_str!("../rules/known.toml");
        let engine = RuleEngine::from_toml_str(toml, &root).expect("parse rules");
        let output = scan_with_engine(&engine, 1, &|_| {});

        let mut restore = fs::metadata(&locked_dir).expect("meta").permissions();
        restore.set_mode(0o755);
        let _ = fs::set_permissions(&locked_dir, restore);
        let _ = fs::remove_dir_all(&root);

        assert!(output.skipped_permission_count >= 1);
        let ids: Vec<&str> = output
            .items
            .iter()
            .filter_map(|item| item.rule_id.as_deref())
            .collect();
        assert_eq!(
            ids,
            vec![
                "xcode-derived-data",
                "huggingface-hub",
                "homebrew",
                "cargo-registry",
                "slack-cache"
            ]
        );
        assert!(output
            .items
            .iter()
            .all(|item| item.size_bytes >= BYTES_PER_MIB));
        assert_eq!(output.items[0].days_since_modified, 0);
    }

    #[test]
    fn disk_health_reads_valid_macos_volume() {
        let health = read_disk_health();
        assert!(health.total_bytes > 0);
        assert!(health.available_bytes <= health.total_bytes);
        assert!(health.used_percent <= 100);
    }

    #[test]
    fn trash_refuses_protected_paths() {
        let root = unique_temp_dir("protected");
        let engine =
            RuleEngine::from_toml_str(include_str!("../rules/known.toml"), &root).expect("parse");

        let result =
            move_items_to_trash(&engine, &["/System/Library".to_string()], &HashMap::new());
        let _ = fs::remove_dir_all(&root);

        assert!(result.trashed_paths.is_empty());
        assert_eq!(result.failed_paths.len(), 1);
    }

    #[test]
    fn trash_refuses_ancestors_outside_home_relative_and_symlinks() {
        let root = unique_temp_dir("trash_boundaries");
        let caches = root.join("Library/Caches");
        let mail = root.join("Library/Mail");
        let desktop = root.join("Desktop/important.txt");
        fs::create_dir_all(&caches).expect("create caches");
        fs::create_dir_all(&mail).expect("create mail");
        fs::create_dir_all(desktop.parent().expect("desktop dir")).expect("create desktop");
        fs::write(&desktop, b"keep").expect("write desktop file");

        let evil_link = caches.join("evil_link");
        std::os::unix::fs::symlink(&mail, &evil_link).expect("create symlink");

        let engine =
            RuleEngine::from_toml_str(include_str!("../rules/known.toml"), &root).expect("parse");

        let targets = vec![
            root.join("Library").to_string_lossy().into_owned(),
            caches.to_string_lossy().into_owned(),
            desktop.to_string_lossy().into_owned(),
            evil_link.to_string_lossy().into_owned(),
            "../relative/path".to_string(),
        ];

        let result = move_items_to_trash(&engine, &targets, &HashMap::new());
        let _ = fs::remove_dir_all(&root);

        assert!(result.trashed_paths.is_empty());
        assert_eq!(result.failed_paths.len(), targets.len());
    }

    #[test]
    fn nested_rule_split_clamps_parent_and_sparse_zero_block_is_zero() {
        let root = unique_temp_dir("split_and_sparse");
        let xcode = root.join("Library/Developer/Xcode");
        let derived = xcode.join("DerivedData/App");
        let archives = xcode.join("Archives/2026");
        let userdata = xcode.join("UserData");
        let sparse_dir = root.join("Library/Caches/SparseTest");

        for dir in [&derived, &archives, &userdata, &sparse_dir] {
            fs::create_dir_all(dir).expect("create dir");
        }
        fs::write(derived.join("a.o"), vec![1_u8; 2 * 1024 * 1024]).expect("write derived");
        fs::write(archives.join("b.xcarchive"), vec![1_u8; 2 * 1024 * 1024])
            .expect("write archives");
        fs::write(userdata.join("c.plist"), vec![1_u8; 2 * 1024 * 1024]).expect("write userdata");

        let sparse_file = fs::File::create(sparse_dir.join("Docker.raw")).expect("create sparse");
        sparse_file
            .set_len(100 * 1024 * 1024)
            .expect("set sparse length");
        drop(sparse_file);

        let engine =
            RuleEngine::from_toml_str(include_str!("../rules/known.toml"), &root).expect("parse");
        let output = scan_with_engine(&engine, 1, &|_| {});
        let _ = fs::remove_dir_all(&root);

        let xcode_parent = output
            .items
            .iter()
            .find(|item| item.path.ends_with("/Library/Developer/Xcode"))
            .expect("xcode parent present");
        assert_eq!(xcode_parent.risk, RiskLevel::Alto);
        assert!(!xcode_parent.selected);
        assert_eq!(xcode_parent.cleanable_bytes, 0);

        assert!(
            !output
                .items
                .iter()
                .any(|item| item.path.ends_with("/SparseTest")),
            "0-block sparse file should not inflate scan size"
        );
    }

    #[test]
    fn stress_scan_handles_thousands_of_nested_files_fast() {
        let root = unique_temp_dir("stress_2k");
        let cache_root = root.join("Library/Caches/HeavyStressApp");
        for bucket_idx in 0..20 {
            let sub = cache_root.join(format!("shard_{bucket_idx}/nested"));
            fs::create_dir_all(&sub).expect("create shard");
            for file_idx in 0..100 {
                fs::write(sub.join(format!("f_{file_idx}.bin")), [0_u8; 1024]).expect("write file");
            }
        }

        let engine =
            RuleEngine::from_toml_str(include_str!("../rules/known.toml"), &root).expect("parse");
        let output = scan_with_engine(&engine, 1, &|_| {});
        let _ = fs::remove_dir_all(&root);

        assert_eq!(output.items.len(), 1);
        assert_eq!(output.items[0].file_count, 2000);
        assert_eq!(output.items[0].size_bytes, 2000 * 1024);
    }
}
