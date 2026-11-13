//! Blocking scan worker. Call on a dedicated background thread, never on the UI thread.
pub mod fingerprint;
use crate::{
    db::{
        repositories::scanning::{Observation, Publication, ScanRun, ScanStore, SCAN_BATCH},
        Database,
    },
    error::{AppError, AppResult},
    identity::{is_link, Evidence},
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Default)]
pub struct ScanReport {
    pub new: u64,
    pub unchanged: u64,
    pub modified: u64,
    pub moved: u64,
    pub renamed: u64,
    pub missing: u64,
    pub restored: u64,
    pub failed: u64,
    pub fingerprinted: u64,
    pub cancelled: bool,
}
/// Single worker and disk-backed queue bound both content I/O concurrency and memory.
/// Database ownership keeps scan work off the application status connection and enforces one writer.
pub fn spawn(
    mut database: Database,
    root: PathBuf,
    cancel: Arc<AtomicBool>,
) -> std::thread::JoinHandle<(Database, AppResult<ScanReport>)> {
    std::thread::spawn(move || {
        let result = database.scan(&root, &cancel);
        (database, result)
    })
}
fn text(path: &Path) -> AppResult<&str> {
    path.to_str()
        .ok_or_else(|| AppError::Validation("path is not valid Unicode".into()))
}
fn absent(path: &str) -> bool {
    fs::symlink_metadata(path).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
}
pub(crate) fn kind(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" | "png" | "webp" | "gif" | "bmp" => Some("image"),
        "mp4" | "webm" | "mov" | "mkv" | "avi" => Some("video"),
        _ => None,
    }
}
pub(crate) fn run(
    store: &mut ScanStore<'_>,
    root: &Path,
    cancel: &AtomicBool,
) -> AppResult<ScanReport> {
    // Reject reparse points in every ancestor, not just the final component.
    for parent in root.ancestors() {
        if is_link(&fs::symlink_metadata(parent)?) {
            return Err(AppError::Validation("linked scan root".into()));
        }
    }
    let root = fs::canonicalize(root)?;
    let root_text = text(&root)?;
    for registered in store.roots()? {
        let registered = Path::new(&registered);
        if registered != root && (registered.starts_with(&root) || root.starts_with(registered)) {
            return Err(AppError::Validation("overlapping roots".into()));
        }
    }
    let generation = store.begin(root_text)?;
    let result = work(store, &root, generation, cancel);
    let state = match &result {
        Ok(r) if r.cancelled => "interrupted",
        Ok(r) if r.failed > 0 => "unavailable",
        Ok(_) => "complete",
        Err(_) => "interrupted",
    };
    store.state(root_text, state)?;
    result
}
fn work(
    store: &mut ScanStore<'_>,
    root: &Path,
    generation: i64,
    cancel: &AtomicBool,
) -> AppResult<ScanReport> {
    let mut report = ScanReport::default();
    let root_text = text(root)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AppError::Internal)?
        .as_millis() as i64;
    while let Some(directory) = store.next(root_text, "dir")? {
        if cancel.load(Ordering::Relaxed) {
            report.cancelled = true;
            return Ok(report);
        }
        let path = Path::new(&directory);
        match fs::symlink_metadata(path) {
            Ok(m) if !is_link(&m) && m.is_dir() => {}
            _ => {
                report.failed += 1;
                store.done(root_text, &directory)?;
                continue;
            }
        }
        let parent = if path == root {
            None
        } else {
            Some(text(path.parent().unwrap())?)
        };
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("data");
        store.directory(root_text, &directory, parent, name, now)?;
        let mut queued = Vec::with_capacity(SCAN_BATCH);
        match fs::read_dir(path) {
            Err(_) => report.failed += 1,
            Ok(entries) => {
                for entry in entries {
                    if cancel.load(Ordering::Relaxed) {
                        report.cancelled = true;
                        return Ok(report);
                    }
                    let entry = match entry {
                        Ok(e) => e,
                        Err(_) => {
                            report.failed += 1;
                            continue;
                        }
                    };
                    let path = entry.path();
                    let meta = match fs::symlink_metadata(&path) {
                        Ok(m) => m,
                        Err(_) => {
                            report.failed += 1;
                            continue;
                        }
                    };
                    if is_link(&meta) {
                        continue;
                    }
                    let Some(path_text) = path.to_str() else {
                        report.failed += 1;
                        continue;
                    };
                    if meta.is_dir() {
                        queued.push((path_text.to_owned(), "dir"));
                    } else if meta.is_file() && kind(&path).is_some() {
                        queued.push((path_text.to_owned(), "file"));
                    }
                    if queued.len() == SCAN_BATCH {
                        store.enqueue_batch(root_text, &queued)?;
                        queued.clear();
                    }
                }
            }
        }
        if !queued.is_empty() {
            store.enqueue_batch(root_text, &queued)?;
        }
        store.done(root_text, &directory)?;
    }
    loop {
        let paths = store.files(root_text)?;
        if paths.is_empty() {
            break;
        }
        let mut staged = Vec::with_capacity(SCAN_BATCH);
        for path in paths {
            if cancel.load(Ordering::Relaxed) {
                report.cancelled = true;
                return Ok(report);
            }
            let observed = (|| -> AppResult<Observation> {
                let evidence = Evidence::read(Path::new(&path))?;
                let old = store.by_path(&path)?;
                let fingerprint = if let Some(old) =
                    old.filter(|old| old.stat == evidence.stat && evidence.key.is_some())
                {
                    old.fingerprint
                } else {
                    report.fingerprinted += 1;
                    fingerprint::quick(Path::new(&path), evidence.size as u64, cancel)?
                };
                if Evidence::read(Path::new(&path))? != evidence {
                    return Err(AppError::Validation(
                        "file changed during observation".into(),
                    ));
                }
                Ok(Observation {
                    path: path.clone(),
                    evidence,
                    fingerprint,
                    kind: kind(Path::new(&path)).unwrap().into(),
                })
            })();
            match observed {
                Ok(o) => staged.push((path, Some(o))),
                Err(_) => {
                    report.failed += 1;
                    staged.push((path, None));
                }
            }
        }
        // All filesystem reads and hashes above have finished before this short write.
        store.stage_batch(root_text, &staged)?;
    }
    let mut after = String::new();
    let run = ScanRun {
        root: root_text,
        generation,
        observed_at: now,
    };
    let mut publications = Vec::with_capacity(SCAN_BATCH);
    while let Some(o) = store.observation(root_text, &after)? {
        after = o.path.clone();
        if cancel.load(Ordering::Relaxed) {
            flush(store, &run, &mut publications)?;
            report.cancelled = true;
            return Ok(report);
        }
        // Recheck evidence just before the short transaction; hashes never run in a transaction.
        if Evidence::read(Path::new(&o.path)).ok().as_ref() != Some(&o.evidence) {
            report.failed += 1;
            continue;
        }
        let candidates = match &o.evidence.key {
            Some(key) if store.identity_count(root_text, key)? == 1 => {
                store.candidates(root_text, key)?
            }
            _ => vec![],
        };
        let matched = if candidates.len() == 1 {
            candidates.into_iter().next().filter(|old| {
                old.fingerprint == o.fingerprint && (old.path == o.path || absent(&old.path))
            })
        } else {
            None
        };
        let same_path = store.by_path(&o.path)?;
        let reason = if let Some(old) = &matched {
            if old.missing {
                report.restored += 1;
                "RESTORED"
            } else if old.path != o.path {
                let old_path = Path::new(&old.path);
                let new_path = Path::new(&o.path);
                let moved = old_path.parent() != new_path.parent();
                let renamed = old_path.file_name() != new_path.file_name();
                if moved {
                    report.moved += 1;
                }
                if renamed {
                    report.renamed += 1;
                }
                if moved && renamed {
                    "MOVED+RENAMED"
                } else if moved {
                    "MOVED"
                } else {
                    "RENAMED"
                }
            } else {
                report.unchanged += 1;
                "UNCHANGED"
            }
        } else if same_path.is_some() {
            report.modified += 1;
            "MODIFIED: conservative replacement"
        } else {
            report.new += 1;
            "NEW: no unique continuity evidence"
        };
        let mut tags = Vec::new();
        let mut parent = Path::new(&o.path).parent();
        while let Some(path) = parent {
            tags.push(text(path)?.to_owned());
            if path == root {
                break;
            }
            parent = path.parent();
        }
        publications.push(Publication {
            observation: o,
            id: matched.as_ref().map(|m| m.id.clone()),
            reason,
            tags,
        });
        if publications.len() == SCAN_BATCH {
            flush(store, &run, &mut publications)?;
        }
    }
    flush(store, &run, &mut publications)?;
    // Any incomplete enumeration/read suppresses missing publication for this generation.
    if report.failed == 0 {
        let mut after = String::new();
        let mut missing = Vec::with_capacity(SCAN_BATCH);
        while let Some((id, path)) = store.unseen(root_text, generation, &after)? {
            if cancel.load(Ordering::Relaxed) {
                if !missing.is_empty() {
                    store.missing_batch(&missing)?;
                }
                report.cancelled = true;
                return Ok(report);
            }
            after = id.clone();
            if absent(&path) {
                missing.push(id);
                report.missing += 1;
                if missing.len() == SCAN_BATCH {
                    store.missing_batch(&missing)?;
                    missing.clear();
                }
            }
        }
        if !missing.is_empty() {
            store.missing_batch(&missing)?;
        }
    }
    Ok(report)
}
fn flush(
    store: &mut ScanStore<'_>,
    run: &ScanRun<'_>,
    publications: &mut Vec<Publication>,
) -> AppResult<()> {
    if !publications.is_empty() {
        store.persist_batch(run, publications)?;
        publications.clear();
    }
    Ok(())
}
