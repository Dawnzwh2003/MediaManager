//! Copy-import use case. All filesystem work is outside short repository transactions.
pub mod storage;
mod worker;
use crate::{
    db::{
        repositories::imports::{Entry, JobContext, Physical, Plan, Publication},
        Database,
    },
    domain::{
        importing::*,
        library::{Media, MediaType, NewMedia},
    },
    error::{AppError, AppResult},
    identity::{is_link, Evidence},
    scanner::{self, fingerprint},
    services::activity::{ActivityClock, SystemClock},
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
use storage::{ImportStorage, LocalStorage};
pub use worker::{spawn, Worker};

pub(crate) fn now() -> i64 {
    SystemClock.now_ms()
}
fn valid_id(id: &str) -> AppResult<()> {
    if id.len() != 32 || !id.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(AppError::Validation("invalid import ID".into()));
    }
    Ok(())
}
pub fn storage_settings(db: &mut Database) -> AppResult<ImportStorageSettings> {
    let selected = db
        .repositories()
        .settings()
        .get("media_storage.v1")?
        .and_then(|v| v.as_str().map(PathBuf::from))
        .unwrap_or_else(|| db.paths.data.join("media"));
    let root = storage::prepare_directory(&selected)?;
    let root_text = storage::text(&root)?;
    db.import_store().root(&root_text, now())?;
    Ok(ImportStorageSettings {
        media_root: root_text,
        available_bytes: LocalStorage.available(&root)?.min(9_007_199_254_740_991),
    })
}
pub fn set_storage(db: &mut Database, path: &Path) -> AppResult<ImportStorageSettings> {
    let root = storage::checked_directory(path)?;
    for source in db.import_store().sources()? {
        if storage::overlap(&root, Path::new(&source)) {
            return Err(AppError::Validation("source_storage_overlap".into()));
        }
    }
    for scan in db.import_store().scan_roots()? {
        if root != Path::new(&scan) && storage::overlap(&root, Path::new(&scan)) {
            return Err(AppError::Validation("scan_storage_overlap".into()));
        }
    }
    let probe = root.join(format!(".import-probe-{}", db.import_store().id()?));
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)?;
    file.sync_all()?;
    drop(file);
    fs::remove_file(&probe)?;
    let text = storage::text(&root)?;
    db.import_store().root(&text, now())?;
    db.repositories()
        .settings()
        .set("media_storage.v1", &serde_json::json!(text), now())?;
    storage_settings(db)
}
fn validate_source(db: &mut Database, path: &Path) -> AppResult<PathBuf> {
    let source =
        storage::checked_directory(path).map_err(|_| AppError::Import("source_unavailable"))?;
    for root in db.import_store().roots()? {
        if storage::overlap(&source, Path::new(&root)) {
            return Err(AppError::Import("source_storage_overlap"));
        }
    }
    Ok(source)
}
pub fn create_preview(db: &mut Database, path: &Path) -> AppResult<ImportJob> {
    let root = storage_settings(db)?.media_root;
    let source = validate_source(db, path)?;
    let root_id = db.import_store().root(&root, now())?;
    let id = db.import_store().create(
        &storage::text(&source)?,
        source
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("Source"),
        &root_id,
        now(),
    )?;
    get_job(db, Some(&id))?.ok_or(AppError::Internal)
}
pub fn get_job(db: &mut Database, id: Option<&str>) -> AppResult<Option<ImportJob>> {
    let id = match id {
        Some(id) => {
            valid_id(id)?;
            Some(id.to_owned())
        }
        None => db.import_store().latest()?,
    };
    id.map(|id| db.import_store().snapshot(&id)).transpose()
}
pub fn start_import(
    db: &mut Database,
    id: &str,
    resume: bool,
    io: &dyn ImportStorage,
) -> AppResult<ImportJob> {
    valid_id(id)?;
    let job = db.import_store().context(id)?;
    validate_source(db, Path::new(&job.source))?;
    storage::checked_directory(Path::new(&job.root))?;
    let snapshot = db.import_store().snapshot(id)?;
    if !resume && io.available(Path::new(&job.root))? < snapshot.estimated_storage_required as u64 {
        return Err(AppError::Import("insufficient_storage"));
    }
    db.import_store().start(id, resume)?;
    db.import_store().snapshot(id)
}
pub fn cancel_import(db: &mut Database, id: &str) -> AppResult<()> {
    valid_id(id)?;
    db.import_store().cancel(id)
}
pub fn source_nodes(
    db: &mut Database,
    source: Option<&str>,
    parent: Option<&str>,
    after: &str,
) -> AppResult<SourcePage> {
    for id in [source, parent].into_iter().flatten() {
        valid_id(id)?;
    }
    if !after.is_empty() {
        valid_id(after)?;
    }
    db.import_store().source_nodes(source, parent, after)
}
fn directories(relative: &str) -> Vec<String> {
    let mut result = vec![];
    let mut path = String::new();
    let parts: Vec<_> = relative.split('/').collect();
    for part in parts.iter().take(parts.len().saturating_sub(1)) {
        if !path.is_empty() {
            path.push('/');
        }
        path.push_str(part);
        result.push(path.clone());
    }
    result
}
fn unchanged(db: &mut Database, m: &Media) -> AppResult<Evidence> {
    let e = storage::evidence(Path::new(&m.file_path))?;
    if e.size != m.file_size
        || e.modified != m.modified_time
        || db
            .import_store()
            .evidence(&m.id)?
            .is_some_and(|s| s != e.stat)
    {
        return Err(AppError::StaleState);
    }
    Ok(e)
}
fn matches(db: &mut Database, m: &Media, hash: &str, cancel: &AtomicBool) -> AppResult<bool> {
    match unchanged(db, m) {
        Ok(e) => {
            let actual = fingerprint::full(Path::new(&m.file_path), cancel)?;
            Ok(actual == hash && storage::evidence(Path::new(&m.file_path))? == e)
        }
        Err(_) => Ok(!Path::new(&m.file_path).exists()
            && db.import_store().managed(&m.id)?.is_some()
            && db
                .import_store()
                .full_hash(&m.id, m.content_version_id)?
                .as_deref()
                == Some(hash)),
    }
}
fn plan_file(
    db: &mut Database,
    job: &JobContext,
    item: &Entry,
    cancel: &AtomicBool,
) -> AppResult<()> {
    let path = storage::join(Path::new(&job.source), &item.relative)?;
    let before = storage::evidence(&path)?;
    let hash = fingerprint::full(&path, cancel)?;
    if storage::evidence(&path)? != before {
        return Err(AppError::StaleState);
    }
    let mut after = String::new();
    let mut found = None;
    loop {
        let candidates = db
            .import_store()
            .candidates(&item.name, before.size, &after)?;
        if candidates.is_empty() {
            break;
        }
        for id in candidates {
            after = id.clone();
            let m = db
                .repositories()
                .media()
                .get(&id)?
                .ok_or(AppError::Internal)?;
            if cancel.load(Ordering::Relaxed) {
                return Err(AppError::Busy);
            }
            if matches(db, &m, &hash, cancel).unwrap_or(false) {
                found = Some((
                    id,
                    db.import_store().managed(&m.id)?.is_none()
                        || !Path::new(&m.file_path).exists(),
                ));
                break;
            }
        }
        if found.is_some() {
            break;
        }
    }
    let (media_id, kind, copy, conflict) = if let Some((id, copy)) = found {
        (id, "existing", copy, false)
    } else if let Some(id) =
        db.import_store()
            .planned_match(&job.id, &item.name, before.size, &hash)?
    {
        (id, "duplicate", false, false)
    } else {
        (
            db.import_store().id()?,
            "new",
            true,
            db.import_store().name_exists(&job.id, &item.name)?,
        )
    };
    db.import_store().plan(
        &job.id,
        Plan {
            entry: item.id,
            size: before.size,
            stat: &before.stat,
            hash: &hash,
            media_id: &media_id,
            kind,
            copy,
            conflict,
            directories: &directories(&item.relative),
        },
    )
}
fn scan_preview(db: &mut Database, job: &JobContext, cancel: &AtomicBool) -> AppResult<()> {
    while let Some(dir) = db.import_store().next(&job.id, "dir", true)? {
        if cancel.load(Ordering::Relaxed) || db.import_store().cancelled(&job.id)? {
            return Ok(());
        }
        let result = (|| -> AppResult<()> {
            let path = if dir.relative.is_empty() {
                PathBuf::from(&job.source)
            } else {
                storage::join(Path::new(&job.source), &dir.relative)?
            };
            storage::checked_directory(&path)?;
            let mut rows = vec![];
            for entry in fs::read_dir(&path)? {
                if cancel.load(Ordering::Relaxed) {
                    return Err(AppError::Busy);
                }
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(_) => {
                        db.import_store().fail_entry(dir.id, "source_unavailable")?;
                        continue;
                    }
                };
                let path = entry.path();
                let meta = fs::symlink_metadata(&path);
                if meta.as_ref().is_ok_and(is_link) {
                    continue;
                }
                let kind = if meta.as_ref().is_ok_and(|m| m.is_dir()) {
                    "dir"
                } else if scanner::kind(&path).is_some() {
                    "file"
                } else {
                    continue;
                };
                let rel = storage::relative(
                    path.strip_prefix(&job.source)
                        .map_err(|_| AppError::Internal)?,
                )?;
                rows.push((
                    rel,
                    kind.to_owned(),
                    entry
                        .file_name()
                        .into_string()
                        .map_err(|_| AppError::Internal)?,
                ));
                if rows.len() == 100 {
                    db.import_store().enqueue(&job.id, &rows)?;
                    rows.clear();
                }
            }
            db.import_store().enqueue(&job.id, &rows)?;
            Ok(())
        })();
        match result {
            Ok(()) => db.import_store().finish_directory(dir.id)?,
            Err(_) if cancel.load(Ordering::Relaxed) => return Ok(()),
            Err(_) => {
                db.import_store().fail_entry(dir.id, "source_unavailable")?;
                if dir.relative.is_empty() {
                    return Err(AppError::Import("source_unavailable"));
                }
            }
        }
    }
    while let Some(item) = db.import_store().next(&job.id, "file", true)? {
        if cancel.load(Ordering::Relaxed) || db.import_store().cancelled(&job.id)? {
            return Ok(());
        }
        db.import_store().current(&job.id, &item.relative)?;
        if let Err(e) = plan_file(db, job, &item, cancel) {
            if cancel.load(Ordering::Relaxed) {
                return Ok(());
            }
            db.import_store().fail_entry(item.id, error_code(&e))?;
        }
    }
    if !cancel.load(Ordering::Relaxed) && !db.import_store().cancelled(&job.id)? {
        db.import_store().preview_ready(&job.id)?;
    }
    Ok(())
}
fn error_code(e: &AppError) -> &'static str {
    match e {
        AppError::MediaUnavailable(s) if s == "insufficient_storage" => "insufficient_storage",
        AppError::StaleState => "source_changed",
        AppError::Storage(_) => "unsafe_path",
        AppError::Database(_) => "database_error",
        _ => "copy_or_read_failed",
    }
}
fn publish_file(
    db: &mut Database,
    job: &JobContext,
    item: &Entry,
    cancel: &AtomicBool,
    io: &dyn ImportStorage,
) -> AppResult<()> {
    let id = item.media_id.as_ref().ok_or(AppError::Internal)?;
    // Scanner IDs use UUID spelling with hyphens; import job IDs use compact hex.
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err(AppError::Storage("unsafe media storage component".into()));
    }
    let hash = item.hash.as_ref().ok_or(AppError::Internal)?;
    let existing = db.repositories().media().get(id)?;
    if let Some(m) = &existing {
        if m.file_name != item.name
            || m.file_size != item.size
            || m.state == "superseded"
            || !matches(db, m, hash, cancel)?
        {
            return Err(AppError::StaleState);
        }
    }
    let managed = db.import_store().managed(id)?;
    let needs_copy = existing.is_none()
        || managed.is_none()
        || existing
            .as_ref()
            .is_some_and(|m| !Path::new(&m.file_path).exists());
    let (stat, physical, new_media) = if needs_copy {
        let (root_id, root) = managed.unwrap_or((job.root_id.clone(), job.root.clone()));
        let root_path = storage::checked_directory(Path::new(&root))?;
        if storage::overlap(&root_path, Path::new(&job.source)) {
            return Err(AppError::Storage("source_storage_overlap".into()));
        }
        let relative = format!("{id}/{}", item.name);
        let target = storage::join(&root_path, &relative)?;
        // An existing final file is usable only through this entry's durable reservation.
        if target.exists() && item.target.as_deref() != target.to_str() {
            return Err(AppError::Storage("target already exists".into()));
        }
        db.import_store()
            .reserve(item.id, &storage::text(&target)?)?;
        if !target.exists() {
            if io.available(&root_path)? < item.size as u64 {
                return Err(AppError::MediaUnavailable("insufficient_storage".into()));
            }
            let source = storage::join(Path::new(&job.source), &item.relative)?;
            let before = storage::evidence(&source)?;
            if Some(&before.stat) != item.stat.as_ref() {
                return Err(AppError::StaleState);
            }
            let parent = target.parent().ok_or(AppError::Internal)?;
            storage::prepare_directory(parent)?;
            let part = parent.join(format!(".import-{}.part", job.id));
            storage::remove_part(&part, &job.id)?;
            let result = (|| -> AppResult<()> {
                let copied = io.copy(&source, &part, cancel)?;
                if copied != *hash || storage::evidence(&source)? != before {
                    return Err(AppError::StaleState);
                }
                if cancel.load(Ordering::Relaxed) {
                    return Err(AppError::Busy);
                }
                storage::set_modified(
                    &part,
                    existing
                        .as_ref()
                        .map_or(before.modified, |m| m.modified_time),
                )?;
                storage::promote(&part, &target)?;
                Ok(())
            })();
            if result.is_err() {
                storage::remove_part(&part, &job.id)?;
            }
            result?;
        }
        let e = storage::evidence(&target)?;
        if e.size != item.size || fingerprint::full(&target, cancel)? != *hash {
            return Err(AppError::StaleState);
        }
        let new = if existing.is_none() {
            Some(NewMedia {
                id: id.clone(),
                fingerprint: Some(fingerprint::quick(&target, item.size as u64, cancel)?),
                file_path: storage::text(&target)?,
                file_name: item.name.clone(),
                media_type: if scanner::kind(&target) == Some("video") {
                    MediaType::Video
                } else {
                    MediaType::Image
                },
                file_size: item.size,
                created_time: e.created,
                modified_time: e.modified,
                imported_at: now(),
                last_seen_at: now(),
                width: None,
                height: None,
                duration: None,
            })
        } else {
            None
        };
        (
            e.stat.clone(),
            Some(Physical {
                root_id,
                root,
                path: storage::text(&target)?,
                relative,
                evidence: e,
            }),
            new,
        )
    } else {
        let source = storage::join(Path::new(&job.source), &item.relative)?;
        let before = storage::evidence(&source)?;
        if Some(&before.stat) != item.stat.as_ref()
            || fingerprint::full(&source, cancel)? != *hash
            || storage::evidence(&source)? != before
        {
            return Err(AppError::StaleState);
        }
        (
            unchanged(db, existing.as_ref().ok_or(AppError::Internal)?)?.stat,
            None,
            None,
        )
    };
    let publication = Publication {
        media_id: id.clone(),
        new_media,
        expected_path: existing.as_ref().map(|m| m.file_path.clone()),
        expected_version: existing.as_ref().map_or(1, |m| m.content_version_id),
        physical,
        hash: hash.clone(),
        stat,
        directories: directories(&item.relative),
        now: now(),
    };
    let gate = db.activity_gate.clone();
    let _permit = gate.lock().map_err(|_| AppError::Internal)?;
    db.import_store().publish(job, item, publication)
}
/// Dedicated worker/test entry. One job at a time; no frontend database/filesystem access.
pub fn run_job(
    db: &mut Database,
    id: &str,
    cancel: &AtomicBool,
    io: &dyn ImportStorage,
) -> AppResult<ImportJob> {
    let job = db.import_store().context(id)?;
    let result = if job.state == "scanning" {
        scan_preview(db, &job, cancel)
    } else if job.state == "importing" {
        (|| -> AppResult<()> {
            while let Some(item) = db.import_store().next(id, "file", false)? {
                if cancel.load(Ordering::Relaxed) || db.import_store().cancelled(id)? {
                    break;
                }
                db.import_store().current(id, &item.relative)?;
                if let Err(e) = publish_file(db, &job, &item, cancel, io) {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    db.import_store().fail_entry(item.id, error_code(&e))?;
                }
            }
            if !cancel.load(Ordering::Relaxed) && !db.import_store().cancelled(id)? {
                db.import_store().set_state(id, "completed", None)?;
            }
            Ok(())
        })()
    } else {
        return db.import_store().snapshot(id);
    };
    if cancel.load(Ordering::Relaxed) || db.import_store().cancelled(id)? {
        db.import_store().set_state(id, "cancelled", None)?;
    } else if let Err(e) = result {
        db.import_store()
            .set_state(id, "failed", Some(error_code(&e)))?;
    }
    db.import_store().snapshot(id)
}
