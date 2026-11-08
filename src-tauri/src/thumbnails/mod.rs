//! Persistent, single-consumer background queue. All I/O runs outside SQLite transactions.
use crate::{
    db::{
        repositories::{derivatives::Item, DerivativeStatus},
        Database,
    },
    error::{AppError, AppResult},
    identity::{is_link, Evidence},
    metadata::{valid_thumbnail, Failure, Toolchain},
    platform::paths::{ensure_directory, validate_managed_file},
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

const GENERATOR_VERSION: i64 = 1;
#[derive(Debug, Default)]
pub struct Report {
    pub ready: u64,
    pub failed: u64,
    pub stale: u64,
    pub cancelled: bool,
}

/// Owns shutdown. Dropping signals cancellation and reaps the worker/subprocess before releasing the DB lock.
pub struct Worker {
    cancel: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    running: Arc<AtomicBool>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        self.running.store(false, Ordering::Release);
    }
}
impl Database {
    pub fn start_thumbnail_worker(&self, tools: Toolchain) -> AppResult<Worker> {
        let mut database = self.background_connection()?;
        if self
            .thumbnail_worker_running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(AppError::Busy);
        }
        if let Err(error) = database.derivative_store().recover(GENERATOR_VERSION) {
            self.thumbnail_worker_running
                .store(false, Ordering::Release);
            return Err(error);
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&cancel);
        let thread = std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                // Storage failures leave durable work for the next iteration/restart.
                if database.process_derivatives(&tools, &stop).is_err() {
                    let _ = database.derivative_store().recover(GENERATOR_VERSION);
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        });
        Ok(Worker {
            cancel,
            thread: Some(thread),
            running: Arc::clone(&self.thumbnail_worker_running),
        })
    }
    /// Snapshot/cache miss request. A failed item retries only when explicitly requested.
    /// Call on a blocking worker; opening UI repeatedly never regenerates a ready file.
    pub fn request_thumbnail(&mut self, id: &str, retry: bool) -> AppResult<DerivativeStatus> {
        let item = self
            .derivative_store()
            .item(Some(id))?
            .ok_or(AppError::NotFound)?;
        let state = self
            .derivative_store()
            .status(id)?
            .ok_or(AppError::NotFound)?;
        if state.status == "ready" {
            let expected = cache_path(&self.paths.thumbnails, &cache_key(&item));
            if state.thumbnail_path.as_deref() != expected.to_str() || !cached(&expected) {
                self.derivative_store().queue(id)?;
            }
        } else if retry && state.status == "failed" {
            self.derivative_store().queue(id)?;
        }
        self.derivative_store()
            .status(id)?
            .ok_or(AppError::NotFound)
    }
    /// Drain pending work using bounded memory and short per-file transactions. Background callers only.
    pub fn process_derivatives(
        &mut self,
        tools: &Toolchain,
        cancel: &AtomicBool,
    ) -> AppResult<Report> {
        let mut report = Report::default();
        while let Some(item) = self.derivative_store().item(None)? {
            if cancel.load(Ordering::Relaxed) {
                report.cancelled = true;
                break;
            }
            let key = cache_key(&item);
            if !self
                .derivative_store()
                .claim(&item, &key, GENERATOR_VERSION)?
            {
                continue;
            }
            let source = Path::new(&item.path);
            let before = read_source(source).and_then(|e| {
                if e.size != item.size
                    || e.modified != item.modified
                    || item.stat.as_ref().is_some_and(|s| s != &e.stat)
                {
                    Err(Failure::SourceChanged)
                } else {
                    Ok(e)
                }
            });
            let mut metadata = before
                .as_ref()
                .map_err(|e| *e)
                .and_then(|_| tools.probe(source, item.video, cancel));
            let mut thumbnail = metadata.as_ref().map_err(|e| *e).and_then(|m| {
                let bytes = tools.thumbnail(source, m, item.video, cancel)?;
                if before.as_ref().ok() != read_source(source).ok().as_ref() {
                    return Err(Failure::SourceChanged);
                }
                publish(&self.paths.thumbnails, &key, &bytes)
            });
            // Neither metadata nor a thumbnail from a changing source may be published.
            if before.is_ok() && before.as_ref().ok() != read_source(source).ok().as_ref() {
                metadata = Err(Failure::SourceChanged);
                thumbnail = Err(Failure::SourceChanged);
            }
            let applied = self
                .derivative_store()
                .finish(&item, &key, &metadata, &thumbnail)?;
            if !applied {
                report.stale += 1;
            } else if thumbnail.is_ok() {
                report.ready += 1;
            } else {
                report.failed += 1;
            }
            if matches!(thumbnail, Err(Failure::Cancelled)) {
                report.cancelled = true;
                break;
            }
        }
        Ok(report)
    }
}
fn read_source(path: &Path) -> Result<Evidence, Failure> {
    for ancestor in path.ancestors() {
        if fs::symlink_metadata(ancestor)
            .map_err(|_| Failure::SourceChanged)
            .map(|m| is_link(&m))?
        {
            return Err(Failure::UnsafePath);
        }
    }
    Evidence::read(path).map_err(|_| Failure::SourceChanged)
}
fn cache_key(item: &Item) -> String {
    let mut hash = Sha256::new();
    // Length-delimited JSON avoids ambiguities, and hashed IDs cannot escape the cache directory.
    hash.update(
        serde_json::to_vec(&(
            &item.id,
            item.version,
            &item.fingerprint,
            item.size,
            item.modified,
            "320-jpeg",
            GENERATOR_VERSION,
        ))
        .expect("primitive serialization"),
    );
    format!("{:x}", hash.finalize())
}
fn cache_path(root: &Path, key: &str) -> PathBuf {
    root.join(format!("{key}.jpg"))
}
fn cached(path: &Path) -> bool {
    path.ancestors()
        .all(|p| fs::symlink_metadata(p).is_ok_and(|m| !is_link(&m)))
        && fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() <= 1_048_576)
        && (|| {
            let mut bytes = Vec::new();
            fs::File::open(path)
                .ok()?
                .take(1_048_577)
                .read_to_end(&mut bytes)
                .ok()?;
            Some(valid_thumbnail(&bytes))
        })()
        .unwrap_or(false)
}
fn publish(root: &Path, key: &str, bytes: &[u8]) -> Result<String, Failure> {
    // Reject links in every ancestor, including replacements after AppPaths::prepare.
    for ancestor in root.ancestors() {
        if fs::symlink_metadata(ancestor).is_ok_and(|m| is_link(&m)) {
            return Err(Failure::UnsafePath);
        }
    }
    ensure_directory(root, false).map_err(|_| Failure::CacheIo)?;
    let destination = cache_path(root, key);
    validate_managed_file(&destination).map_err(|_| Failure::UnsafePath)?;
    if cached(&destination) {
        return destination
            .to_str()
            .map(str::to_owned)
            .ok_or(Failure::CacheIo);
    }
    let temporary = root.join(format!("{key}.part"));
    // Remove only our deterministic regular temporary file; never follow a link or recurse.
    validate_managed_file(&temporary).map_err(|_| Failure::UnsafePath)?;
    if temporary.exists() {
        fs::remove_file(&temporary).map_err(|_| Failure::CacheIo)?;
    }
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|_| Failure::CacheIo)?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| Failure::CacheIo)?;
        drop(file);
        if destination.exists() {
            fs::remove_file(&destination).map_err(|_| Failure::CacheIo)?;
        }
        fs::rename(&temporary, &destination).map_err(|_| Failure::CacheIo)?;
        destination
            .to_str()
            .map(str::to_owned)
            .ok_or(Failure::CacheIo)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keys_isolate_independent_ids_content_evidence_and_versions_but_not_paths() {
        let mut item = Item {
            id: "../../escape".into(),
            version: 1,
            fingerprint: Some("same".into()),
            path: "/one.png".into(),
            size: 10,
            modified: 1,
            video: false,
            stat: None,
        };
        let first = cache_key(&item);
        assert_eq!(first.len(), 64);
        assert!(first.chars().all(|c| c.is_ascii_hexdigit()));
        item.path = "/moved.png".into();
        assert_eq!(first, cache_key(&item));
        item.id = "another".into();
        assert_ne!(first, cache_key(&item));
        item.id = "../../escape".into();
        item.version = 2;
        assert_ne!(first, cache_key(&item));
        item.version = 1;
        item.modified = 2;
        assert_ne!(first, cache_key(&item));
        item.modified = 1;
        item.fingerprint = Some("changed".into());
        assert_ne!(first, cache_key(&item));
    }
    #[cfg(unix)]
    #[test]
    fn linked_cache_cannot_publish_or_remove_outside_files() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("keep"), b"keep").unwrap();
        let linked = root.path().join("thumbnails");
        std::os::unix::fs::symlink(outside.path(), &linked).unwrap();
        assert_eq!(
            publish(&linked, &"a".repeat(64), b"invalid").unwrap_err(),
            Failure::UnsafePath
        );
        assert_eq!(fs::read(outside.path().join("keep")).unwrap(), b"keep");
        assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 1);
    }
}
