//! Typed storage access only. No SQL connection is exposed over IPC.
pub(crate) mod activity;
mod catalog_query;
mod events;
mod library;
mod playback;
mod random_order;
mod reports;
pub use reports::ReportsRepository;
pub(crate) mod statistics;
pub use playback::PlaybackRepository;
mod media;
pub use library::LibraryRepository;
pub(crate) mod browse;
pub(crate) mod browse_spool;
pub(crate) mod quick_tags;
mod sessions;
mod settings;
mod tags;

pub use events::EventRepository;
pub use media::MediaRepository;
pub use sessions::QuickBrowseRepository;
pub use settings::SettingsRepository;
pub use tags::{CustomTagRepository, SystemTagRepository};

use crate::error::{AppError, AppResult};
use rusqlite::Connection;

pub struct Repositories<'a> {
    pub(crate) connection: &'a mut Connection,
}

impl Repositories<'_> {
    pub fn reports(&mut self) -> ReportsRepository<'_> {
        ReportsRepository {
            connection: self.connection,
        }
    }
    pub fn playback(&mut self) -> PlaybackRepository<'_> {
        PlaybackRepository {
            connection: self.connection,
        }
    }
    pub fn library(&mut self) -> LibraryRepository<'_> {
        LibraryRepository {
            connection: self.connection,
        }
    }
    pub fn media(&mut self) -> MediaRepository<'_> {
        MediaRepository {
            connection: self.connection,
        }
    }
    pub fn custom_tags(&mut self) -> CustomTagRepository<'_> {
        CustomTagRepository {
            connection: self.connection,
        }
    }
    pub fn system_tags(&mut self) -> SystemTagRepository<'_> {
        SystemTagRepository {
            connection: self.connection,
        }
    }
    pub fn events(&self) -> EventRepository<'_> {
        EventRepository {
            connection: self.connection,
        }
    }
    pub fn quick_browse(&mut self) -> QuickBrowseRepository<'_> {
        QuickBrowseRepository {
            connection: self.connection,
        }
    }
    pub fn settings(&mut self) -> SettingsRepository<'_> {
        SettingsRepository {
            connection: self.connection,
        }
    }
}

fn page_size(limit: u32) -> AppResult<u32> {
    if !(1..=200).contains(&limit) {
        return Err(AppError::Validation(
            "page size must be between 1 and 200".into(),
        ));
    }
    Ok(limit)
}

fn nonempty(value: &str) -> AppResult<()> {
    if value.trim().is_empty() {
        return Err(AppError::Validation("value cannot be blank".into()));
    }
    Ok(())
}

fn changed(count: usize) -> AppResult<()> {
    if count == 0 {
        Err(AppError::NotFound)
    } else {
        Ok(())
    }
}

pub(crate) mod derivatives;
pub(crate) mod scanning;
pub use derivatives::DerivativeStatus;
