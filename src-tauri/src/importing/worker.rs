use super::*;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
type Active = Arc<Mutex<Option<(String, Arc<AtomicBool>)>>>;
pub struct Worker {
    stop: Arc<AtomicBool>,
    active: Active,
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
    let flag = stop.clone();
    let active: Active = Arc::new(Mutex::new(None));
    let current = active.clone();
    // Snapshot identity, not wall time: recovery must tolerate clock changes and same-ms job creation.
    let boundary = db.import_store().recovery_boundary()?;
    let thread = std::thread::spawn(move || {
        let mut recovering = true;
        while !flag.load(Ordering::Relaxed) {
            if recovering {
                if db.import_store().recover(boundary).is_err() {
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }
                recovering = false;
            }
            if let Ok(Some(id)) = db.import_store().work() {
                let cancelled = Arc::new(AtomicBool::new(false));
                if let Ok(mut slot) = current.lock() {
                    *slot = Some((id.clone(), cancelled.clone()));
                }
                let _ = run_job(&mut db, &id, &cancelled, &LocalStorage);
                if let Ok(mut slot) = current.lock() {
                    *slot = None;
                }
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    });
    Ok(Worker {
        stop,
        active,
        thread: Some(thread),
    })
}
