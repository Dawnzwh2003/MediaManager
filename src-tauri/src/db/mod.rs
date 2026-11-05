pub mod migrations;
pub mod repositories;

use std::{
    fs::{File, OpenOptions},
    io,
    time::Duration,
};

use fs2::FileExt;
use rusqlite::Connection;

use crate::{
    domain::DatabaseStatus,
    error::{AppError, AppResult},
    platform::paths::{validate_managed_file, AppPaths},
};

pub const BUSY_TIMEOUT_MS: u32 = 2_000;

pub struct Database {
    connection: Connection,
    // Held for the entire connection lifetime; a second app cannot migrate/write.
    _instance_lock: File,
}

impl Database {
    pub fn repositories(&mut self) -> repositories::Repositories<'_> {
        repositories::Repositories {
            connection: &mut self.connection,
        }
    }

    pub fn open(paths: &AppPaths) -> AppResult<Self> {
        validate_managed_file(&paths.instance_lock)?;
        validate_managed_file(&paths.database)?;
        for suffix in ["-wal", "-shm", "-journal"] {
            let mut sidecar = paths.database.as_os_str().to_owned();
            sidecar.push(suffix);
            validate_managed_file(std::path::Path::new(&sidecar))?;
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&paths.instance_lock)?;
        FileExt::try_lock_exclusive(&lock).map_err(|error| {
            if error.kind() == io::ErrorKind::WouldBlock
                || error.raw_os_error() == fs2::lock_contended_error().raw_os_error()
            {
                AppError::Busy
            } else {
                AppError::Io(error)
            }
        })?;

        let mut connection = Connection::open(&paths.database)?;
        connection.busy_timeout(Duration::from_millis(u64::from(BUSY_TIMEOUT_MS)))?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        // Validate existing history before a persistent PRAGMA can alter its header.
        migrations::validate(&connection)?;
        let journal_mode: String =
            connection.query_row("PRAGMA journal_mode=WAL", [], |r| r.get(0))?;
        if !journal_mode.eq_ignore_ascii_case("wal") {
            return Err(AppError::Storage(
                "WAL is unavailable for application database".into(),
            ));
        }
        connection.pragma_update(None, "synchronous", "FULL")?;
        migrations::migrate(&mut connection, &paths.data)?;

        Ok(Self {
            connection,
            _instance_lock: lock,
        })
    }

    pub fn status(&self) -> AppResult<DatabaseStatus> {
        Ok(DatabaseStatus {
            schema_version: migrations::schema_version(&self.connection)?,
            sqlite_version: self
                .connection
                .query_row("SELECT sqlite_version()", [], |r| r.get(0))?,
            foreign_keys: self
                .connection
                .query_row("PRAGMA foreign_keys", [], |r| r.get::<_, u32>(0))?
                == 1,
            journal_mode: self
                .connection
                .query_row("PRAGMA journal_mode", [], |r| r.get(0))?,
            busy_timeout_ms: self
                .connection
                .query_row("PRAGMA busy_timeout", [], |r| r.get(0))?,
        })
    }
}
