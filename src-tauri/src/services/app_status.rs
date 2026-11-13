use std::{path::PathBuf, sync::Mutex};

use crate::{
    db::Database,
    domain::AppStatus,
    error::{AppError, AppResult},
    platform::paths::AppPaths,
};

struct Runtime {
    _imports: crate::importing::Worker,
    _quick_browse: crate::services::quick_browse::Worker,
    _statistics: crate::services::statistics::Worker,
    _thumbnails: crate::thumbnails::Worker,
    paths: AppPaths,
    database: Database,
}

pub struct AppState {
    #[cfg(feature = "desktop")]
    pub(crate) viewers: crate::viewer::ViewerService,
    #[cfg(feature = "desktop")]
    random_catalog: Mutex<Option<Database>>,
    root: PathBuf,
    runtime: Mutex<Option<Runtime>>,
}

impl AppState {
    #[cfg(feature = "desktop")]
    pub(crate) fn cancel_import(&self, id: &str) -> AppResult<()> {
        let slot = self.runtime.lock().map_err(|_| AppError::Internal)?;
        if let Some(runtime) = slot.as_ref() {
            runtime._imports.cancel(id);
        }
        Ok(())
    }
    #[cfg(feature = "desktop")]
    pub(crate) fn cancel_browse(&self, id: &str) -> AppResult<()> {
        let slot = self.runtime.lock().map_err(|_| AppError::Internal)?;
        if let Some(runtime) = slot.as_ref() {
            runtime._quick_browse.cancel(id);
        }
        Ok(())
    }
    #[cfg(feature = "desktop")]
    pub(crate) fn catalog<T>(
        &self,
        random: bool,
        work: impl FnOnce(&mut Database) -> AppResult<T>,
    ) -> AppResult<T> {
        if !random {
            return work(&mut self.database()?);
        }
        let mut slot = self.random_catalog.lock().map_err(|_| AppError::Internal)?;
        if slot.is_none() {
            *slot = Some(self.database()?);
        }
        work(slot.as_mut().ok_or(AppError::Internal)?)
    }
    #[cfg(feature = "desktop")]
    pub(crate) fn database(&self) -> AppResult<Database> {
        self.status()?;
        let slot = self.runtime.lock().map_err(|_| AppError::Internal)?;
        slot.as_ref()
            .ok_or(AppError::Internal)?
            .database
            .background_connection()
    }
    pub fn new(root: PathBuf) -> Self {
        Self {
            #[cfg(feature = "desktop")]
            viewers: crate::viewer::ViewerService::default(),
            #[cfg(feature = "desktop")]
            random_catalog: Mutex::new(None),
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
            let imports = crate::importing::spawn(&database)?;
            let quick_browse = crate::services::quick_browse::spawn(&database)?;
            let statistics = crate::services::statistics::spawn(&database)?;
            let thumbnails =
                database.start_thumbnail_worker(crate::metadata::Toolchain::default())?;
            *slot = Some(Runtime {
                _imports: imports,
                _quick_browse: quick_browse,
                _statistics: statistics,
                _thumbnails: thumbnails,
                paths,
                database,
            });
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
