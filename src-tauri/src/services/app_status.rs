use std::{path::PathBuf, sync::Mutex};

use crate::{
    db::Database,
    domain::AppStatus,
    error::{AppError, AppResult},
    platform::paths::AppPaths,
};

struct Runtime {
    paths: AppPaths,
    database: Database,
}

pub struct AppState {
    root: PathBuf,
    runtime: Mutex<Option<Runtime>>,
}

impl AppState {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            runtime: Mutex::new(None),
        }
    }

    // Called only on a blocking worker, never on the WebView/UI thread.
    pub fn status(&self) -> AppResult<AppStatus> {
        let mut slot = self.runtime.lock().map_err(|_| AppError::Internal)?;
        if slot.is_none() {
            let paths = AppPaths::prepare(&self.root)?;
            let database = Database::open(&paths)?;
            *slot = Some(Runtime { paths, database });
        }
        let runtime = slot.as_ref().ok_or(AppError::Internal)?;
        Ok(AppStatus {
            app_name: "Media Manager",
            app_version: env!("CARGO_PKG_VERSION"),
            phase: 1,
            database: runtime.database.status()?,
            storage: runtime.paths.status(),
        })
    }
}
