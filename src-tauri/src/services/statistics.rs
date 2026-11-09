use crate::{
    db::Database,
    domain::activity::StatisticsJob,
    error::{AppError, AppResult},
};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
fn guarded<T>(
    db: &mut Database,
    work: impl FnOnce(&mut crate::db::repositories::statistics::StatisticsStore<'_>) -> AppResult<T>,
) -> AppResult<T> {
    let gate = db.activity_gate.clone();
    let _permit = gate.lock().map_err(|_| AppError::Internal)?;
    work(&mut db.statistics_store())
}
pub fn rebuild_statistics(
    db: &mut Database,
    request: &str,
    kind: &str,
    media: Option<&str>,
) -> AppResult<StatisticsJob> {
    if request.is_empty()
        || request.len() > 160
        || !matches!(kind, "all" | "like" | "view" | "play")
    {
        return Err(AppError::Validation("invalid rebuild request".into()));
    }
    guarded(db, |store| store.start(request, kind, media))
}
pub fn rebuild_like_count(db: &mut Database, id: &str, request: &str) -> AppResult<StatisticsJob> {
    rebuild_statistics(db, request, "like", Some(id))
}
pub fn rebuild_all_like_counts(db: &mut Database, request: &str) -> AppResult<StatisticsJob> {
    rebuild_statistics(db, request, "like", None)
}
pub fn rebuild_view_count(db: &mut Database, id: &str, request: &str) -> AppResult<StatisticsJob> {
    rebuild_statistics(db, request, "view", Some(id))
}
pub fn rebuild_all_view_counts(db: &mut Database, request: &str) -> AppResult<StatisticsJob> {
    rebuild_statistics(db, request, "view", None)
}
pub fn rebuild_play_count(db: &mut Database, id: &str, request: &str) -> AppResult<StatisticsJob> {
    rebuild_statistics(db, request, "play", Some(id))
}
pub fn rebuild_all_play_counts(db: &mut Database, request: &str) -> AppResult<StatisticsJob> {
    rebuild_statistics(db, request, "play", None)
}
pub fn step(db: &mut Database, id: &str) -> AppResult<StatisticsJob> {
    guarded(db, |store| store.step(id))
}
pub fn status(db: &mut Database, id: &str) -> AppResult<StatisticsJob> {
    db.statistics_store().status(id)
}
pub fn cancel(db: &mut Database, id: &str) -> AppResult<()> {
    guarded(db, |store| store.cancel(id))
}
pub struct Worker {
    cancel: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
pub fn spawn(db: &Database) -> AppResult<Worker> {
    let mut db = db.background_connection()?;
    let cancel = Arc::new(AtomicBool::new(false));
    let stop = cancel.clone();
    let startup = super::activity::ActivityClock::now_ms(&super::activity::SystemClock);
    let thread = std::thread::spawn(move || {
        let mut recovering = true;
        while !stop.load(Ordering::Relaxed) {
            if recovering {
                recovering = super::activity::recover_batch(&mut db, startup).unwrap_or(true);
            }
            if let Ok(Some(id)) = db.statistics_store().active() {
                if step(&mut db, &id).is_err() {
                    let _ = db.statistics_store().fail(&id);
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    });
    Ok(Worker {
        cancel,
        thread: Some(thread),
    })
}
