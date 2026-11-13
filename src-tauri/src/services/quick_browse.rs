use crate::{
    db::{
        repositories::{browse::BrowseStore, browse_spool::BrowseSpool},
        Database,
    },
    domain::{
        library_query::{LibrarySort, RandomOrder},
        quick_browse::*,
    },
    error::{AppError, AppResult},
    platform::paths::{ensure_directory, validate_managed_file},
    services::activity::{ActivityClock, SystemClock},
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
pub struct BuildLimits {
    pub spool_bytes: u64,
    pub read_timeout: Duration,
}
impl Default for BuildLimits {
    fn default() -> Self {
        Self {
            spool_bytes: 512 * 1024 * 1024,
            read_timeout: Duration::from_secs(120),
        }
    }
}

pub(crate) fn write<T>(
    db: &mut Database,
    work: impl FnOnce(&mut BrowseStore<'_>) -> AppResult<T>,
) -> AppResult<T> {
    let gate = db.activity_gate.clone();
    let _permit = gate.lock().map_err(|_| AppError::Internal)?;
    work(&mut db.browse_store())
}

pub(crate) fn validate_id(id: &str) -> AppResult<()> {
    if id.is_empty()
        || id.len() > 80
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
    {
        return Err(AppError::Validation(
            "invalid browse request identity".into(),
        ));
    }
    Ok(())
}

pub fn create(db: &mut Database, request: &QuickBrowseRequest) -> AppResult<BrowseSession> {
    validate_id(&request.request_id)?;
    db.repositories()
        .library()
        .validate_filter(&request.filter)?;
    write(db, |store| {
        if store.find(&request.request_id)? {
            let old = store.get(&request.request_id)?;
            if old.parent_session_id.is_some()
                || store.original_request(&request.request_id)? != serde_json::to_string(request)?
            {
                return Err(AppError::Validation(
                    "browse request identity conflict".into(),
                ));
            }
            return Ok(old);
        }
        store.insert(request, None, SystemClock.now_ms())?;
        store.get(&request.request_id)
    })
}

pub fn reshuffle(db: &mut Database, parent: &str, request_id: &str) -> AppResult<BrowseSession> {
    validate_id(request_id)?;
    write(db, |store| {
        if store.find(request_id)? {
            let existing = store.get(request_id)?;
            if existing.parent_session_id.as_deref() != Some(parent) {
                return Err(AppError::Validation(
                    "shuffle request identity conflict".into(),
                ));
            }
            return Ok(existing);
        }
        let old = store.get(parent)?;
        if old.state != "ready" || old.ended_at.is_some() {
            return Err(AppError::StaleState);
        }
        let mut filter = old.filter;
        filter.sort = LibrarySort::Random;
        filter.random = Some(RandomOrder {
            seed: store.new_seed()?,
            version: 1,
        });
        let request = QuickBrowseRequest {
            request_id: request_id.into(),
            filter,
            autoplay: old.autoplay,
            muted: old.muted,
        };
        store.insert(&request, Some(parent), SystemClock.now_ms())?;
        store.get(request_id)
    })
}

pub fn get(db: &mut Database, id: &str) -> AppResult<BrowseSession> {
    db.browse_store().get(id)
}
pub fn settings(db: &mut Database) -> AppResult<BrowseSettings> {
    let preferences: BrowsePreferences = db
        .repositories()
        .settings()
        .get("quick_browse.v1")?
        .map(serde_json::from_value)
        .transpose()?
        .unwrap_or_default();
    let mut bindings = Vec::new();
    for binding in preferences.bindings {
        if let Some(tag) = db.repositories().custom_tags().get(&binding.tag_id)? {
            bindings.push(ResolvedBinding {
                digit: binding.digit,
                tag_id: tag.id,
                name: tag.name,
            });
        }
    }
    bindings.sort_by_key(|binding| binding.digit);
    Ok(BrowseSettings {
        autoplay: preferences.autoplay,
        muted: preferences.muted,
        bindings,
    })
}
pub fn save_settings(
    db: &mut Database,
    preferences: &BrowsePreferences,
    session: Option<&str>,
) -> AppResult<BrowseSettings> {
    let digits = preferences
        .bindings
        .iter()
        .map(|binding| binding.digit)
        .collect::<std::collections::BTreeSet<_>>();
    if preferences.bindings.len() > 9
        || digits.len() != preferences.bindings.len()
        || preferences.bindings.iter().any(|binding| {
            !(1..=9).contains(&binding.digit)
                || binding.tag_id.is_empty()
                || binding.tag_id.len() > 256
        })
    {
        return Err(AppError::Validation(
            "invalid quick browse preferences".into(),
        ));
    }
    write(db, |store| {
        store.save_preferences(preferences, session, SystemClock.now_ms())
    })?;
    settings(db)
}
pub fn items(db: &mut Database, id: &str, after: i64, limit: u32) -> AppResult<Vec<BrowseItem>> {
    if after < -1 {
        return Err(AppError::Validation("invalid browse ordinal".into()));
    }
    let session = db.browse_store().get(id)?;
    if session.state != "ready" {
        return Err(AppError::StaleState);
    }
    db.browse_store().items(id, after, limit)
}
pub fn select(db: &mut Database, id: &str, ordinal: i64) -> AppResult<()> {
    write(db, |store| store.select(id, ordinal))
}
pub fn close(db: &mut Database, id: &str) -> AppResult<()> {
    write(db, |store| store.close(id, SystemClock.now_ms()))
}

fn spool_path(db: &Database, id: &str) -> AppResult<PathBuf> {
    let directory = db.paths.cache.join("browse");
    for ancestor in directory.ancestors() {
        if fs::symlink_metadata(ancestor).is_ok_and(|metadata| crate::identity::is_link(&metadata))
        {
            return Err(AppError::Storage("linked browse cache".into()));
        }
    }
    ensure_directory(&directory, false)?;
    let key = format!("{:x}", Sha256::digest(id.as_bytes()));
    let path = directory.join(format!("{key}.sqlite3"));
    validate_managed_file(&path)?;
    Ok(path)
}
fn remove_spool(path: &Path) -> AppResult<()> {
    validate_managed_file(path)?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

/// Blocking worker/test entry. The source read snapshot ends before main-database publication.
pub fn build_session(
    db: &mut Database,
    id: &str,
    cancelled: &(dyn Fn() -> bool + Sync),
    limits: BuildLimits,
) -> AppResult<BrowseSession> {
    let session = db.browse_store().get(id)?;
    if session.state != "building" || session.phase != "pending" {
        return Err(AppError::StaleState);
    }
    let path = match spool_path(db, id).and_then(|path| {
        remove_spool(&path)?;
        Ok(path)
    }) {
        Ok(path) => path,
        Err(error) => {
            write(db, |store| store.fail(id, "snapshot_failed"))?;
            return Err(error);
        }
    };
    let deadline = Instant::now();
    let stop = || cancelled() || deadline.elapsed() > limits.read_timeout;
    let mut progress = db.background_connection()?;
    let result = (|| {
        if stop() || db.browse_store().cancelled(id)? {
            return Err(AppError::Validation("snapshot_cancelled".into()));
        }
        write(db, |store| store.spooling(id))?;
        let mut spool = BrowseSpool::create(&path, limits.spool_bytes)?;
        let interrupt = db.connection_interrupt();
        let (count, generation) = std::thread::scope(|scope| {
            let (done, finished) = std::sync::mpsc::channel::<()>();
            let stop = &stop;
            scope.spawn(move || {
                while finished
                    .recv_timeout(Duration::from_millis(50))
                    .is_err_and(|error| matches!(error, std::sync::mpsc::RecvTimeoutError::Timeout))
                {
                    if stop() {
                        interrupt.interrupt();
                        break;
                    }
                }
            });
            let result = spool.write(|transaction| {
                let mut insert = transaction.prepare("INSERT INTO snapshot VALUES(?1,?2,?3)")?;
                db.repositories()
                    .library()
                    .visit_browse_snapshot(&session.filter, &stop, |item| {
                        BrowseSpool::insert(&mut insert, &item)?;
                        if (item.ordinal + 1) % 1000 == 0 {
                            if progress.browse_store().cancelled(id)? {
                                return Err(AppError::Validation("snapshot_cancelled".into()));
                            }
                            write(&mut progress, |store| store.progress(id, item.ordinal + 1))?;
                        }
                        Ok(())
                    })
            });
            drop(done);
            result
        })?;
        drop(spool);
        if stop() || db.browse_store().cancelled(id)? {
            return Err(AppError::Validation("snapshot_cancelled".into()));
        }
        write(db, |store| store.publishing(id, count, generation))?;
        let spool = BrowseSpool::reopen(&path)?;
        let mut after = -1;
        loop {
            if cancelled() {
                return Err(AppError::Validation("snapshot_cancelled".into()));
            }
            let batch = spool.page(after)?;
            if batch.is_empty() {
                break;
            }
            after = batch.last().ok_or(AppError::Internal)?.ordinal;
            write(db, |store| store.add_batch(id, &batch))?;
            std::thread::yield_now();
        }
        drop(spool);
        write(db, |store| store.publish(id, SystemClock.now_ms()))?;
        Ok(())
    })();
    let removed = remove_spool(&path);
    if let Err(error) = result {
        let reason = if deadline.elapsed() > limits.read_timeout {
            "snapshot_timeout"
        } else if cancelled() || db.browse_store().cancelled(id).unwrap_or(false) {
            "snapshot_cancelled"
        } else {
            "snapshot_failed"
        };
        write(db, |store| store.fail(id, reason))?;
        return Err(error);
    }
    removed?;
    db.browse_store().get(id)
}

pub fn cleanup_step(db: &mut Database, id: &str) -> AppResult<bool> {
    let done = write(db, |store| store.cleanup_batch(id, SystemClock.now_ms()))?;
    let path = spool_path(db, id)?;
    remove_spool(&path)?;
    Ok(done)
}

type ActiveBuild = Arc<Mutex<Option<(String, Arc<AtomicBool>)>>>;
pub struct Worker {
    stop: Arc<AtomicBool>,
    active: ActiveBuild,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Worker {
    pub fn cancel(&self, id: &str) {
        if let Ok(active) = self.active.lock() {
            if let Some((current, flag)) = active.as_ref() {
                if current == id {
                    flag.store(true, Ordering::Relaxed);
                }
            }
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Ok(active) = self.active.lock() {
            if let Some((_, flag)) = active.as_ref() {
                flag.store(true, Ordering::Relaxed);
            }
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
pub fn spawn(db: &Database) -> AppResult<Worker> {
    let mut db = db.background_connection()?;
    let stop = Arc::new(AtomicBool::new(false));
    let active: ActiveBuild = Arc::new(Mutex::new(None));
    let stopped = stop.clone();
    let current = active.clone();
    let thread = std::thread::spawn(move || {
        let mut recovering = true;
        while !stopped.load(Ordering::Relaxed) {
            if recovering {
                if db.browse_store().recover().is_err() {
                    std::thread::sleep(Duration::from_millis(50));
                    continue;
                }
                recovering = false;
            }
            if let Ok(Some(id)) = db.browse_store().next_job() {
                let flag = Arc::new(AtomicBool::new(false));
                if let Ok(mut active) = current.lock() {
                    *active = Some((id.clone(), flag.clone()));
                }
                if let Ok(session) = db.browse_store().get(&id) {
                    if session.phase == "cleaning" {
                        let _ = cleanup_step(&mut db, &id);
                    } else if build_session(
                        &mut db,
                        &id,
                        &|| flag.load(Ordering::Relaxed) || stopped.load(Ordering::Relaxed),
                        BuildLimits::default(),
                    )
                    .is_err()
                        && db
                            .browse_store()
                            .get(&id)
                            .is_ok_and(|session| session.phase != "cleaning")
                    {
                        let _ = write(&mut db, |store| store.fail(&id, "snapshot_failed"));
                    }
                }
                if let Ok(mut active) = current.lock() {
                    *active = None;
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    });
    Ok(Worker {
        stop,
        active,
        thread: Some(thread),
    })
}
