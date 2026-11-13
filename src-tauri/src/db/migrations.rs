use std::{
    fs::{self, OpenOptions},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{backup::NO_PROGRESS, Connection, TransactionBehavior, MAIN_DB};
use sha2::{Digest, Sha256};

use crate::{
    error::{AppError, AppResult},
    platform::paths::ensure_directory,
};

struct Migration {
    version: u32,
    name: &'static str,
    sql: &'static str,
}

pub const CURRENT_SCHEMA_VERSION: u32 = 11;

const MIGRATIONS: &[Migration] = &[
    // Append only: applied SQL checksums are immutable.
    Migration {
        version: 1,
        name: "app_metadata",
        sql: include_str!("../../migrations/0001_app_metadata.sql"),
    },
    Migration {
        version: 2,
        name: "library_model",
        sql: include_str!("../../migrations/0002_library_model.sql"),
    },
    Migration {
        version: 3,
        name: "scanning",
        sql: include_str!("../../migrations/0003_scanning.sql"),
    },
    Migration {
        version: 4,
        name: "media_derivatives",
        sql: include_str!("../../migrations/0004_media_derivatives.sql"),
    },
    Migration {
        version: 5,
        name: "library_queries",
        sql: include_str!("../../migrations/0005_library_queries.sql"),
    },
    Migration {
        version: 6,
        name: "playback_owners",
        sql: include_str!("../../migrations/0006_playback_owners.sql"),
    },
    Migration {
        version: 7,
        name: "activity",
        sql: include_str!("../../migrations/0007_activity.sql"),
    },
    Migration {
        version: 8,
        name: "library_search",
        sql: include_str!("../../migrations/0008_library_search.sql"),
    },
    Migration {
        version: 9,
        name: "quick_browse",
        sql: include_str!("../../migrations/0009_quick_browse.sql"),
    },
    Migration {
        version: 10,
        name: "reports",
        sql: include_str!("../../migrations/0010_reports.sql"),
    },
    Migration {
        version: 11,
        name: "scanner_indexes",
        sql: include_str!("../../migrations/0011_scanner_indexes.sql"),
    },
];

const CREATE_LEDGER: &str = "
CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY CHECK(version > 0),
    name TEXT NOT NULL,
    checksum TEXT NOT NULL,
    applied_at TEXT NOT NULL
) STRICT;";

pub fn schema_version(connection: &Connection) -> AppResult<u32> {
    Ok(connection.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |r| r.get(0),
    )?)
}

pub fn migrate(connection: &mut Connection, data_directory: &Path) -> AppResult<()> {
    migrate_with(connection, data_directory, MIGRATIONS)
}

pub fn validate(connection: &Connection) -> AppResult<()> {
    validate_history(connection, MIGRATIONS).map(|_| ())
}

fn checksum(sql: &str) -> String {
    format!("{:x}", Sha256::digest(sql.as_bytes()))
}

fn migrate_with(
    connection: &mut Connection,
    data_directory: &Path,
    migrations: &[Migration],
) -> AppResult<()> {
    let applied_count = validate_history(connection, migrations)?;
    if applied_count == migrations.len() {
        return Ok(());
    }
    if applied_count != 0 {
        backup_before_upgrade(connection, data_directory, applied_count as u32)?;
    }
    for migration in &migrations[applied_count..] {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        // The migration ledger is the runner's only bootstrap DDL.
        transaction.execute_batch(CREATE_LEDGER)?;
        transaction.execute_batch(migration.sql).map_err(|error| {
            AppError::Migration(format!("migration {} failed: {error}", migration.version))
        })?;
        transaction.execute(
            "INSERT INTO schema_migrations(version,name,checksum,applied_at) VALUES (?1,?2,?3,strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
            rusqlite::params![migration.version, migration.name, checksum(migration.sql)],
        )?;
        transaction.commit()?;
    }
    Ok(())
}

fn validate_history(connection: &Connection, migrations: &[Migration]) -> AppResult<usize> {
    for (index, migration) in migrations.iter().enumerate() {
        if migration.version as usize != index + 1 {
            return Err(AppError::Migration(
                "migration versions are not contiguous".into(),
            ));
        }
    }
    let has_ledger: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='schema_migrations')",
        [], |r| r.get(0),
    )?;
    let applied: Vec<(u32, String, String)> = if has_ledger {
        let mut statement = connection
            .prepare("SELECT version, name, checksum FROM schema_migrations ORDER BY version")?;
        let rows = statement.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        rows.collect::<Result<_, _>>()?
    } else {
        let user_objects: u32 = connection.query_row(
            "SELECT COUNT(*) FROM sqlite_schema WHERE name NOT GLOB 'sqlite_*'",
            [],
            |r| r.get(0),
        )?;
        if user_objects != 0 {
            return Err(AppError::Migration(
                "refusing an untracked existing database".into(),
            ));
        }
        Vec::new()
    };
    for (index, (version, name, recorded_checksum)) in applied.iter().enumerate() {
        let expected = migrations
            .get(index)
            .ok_or_else(|| AppError::Migration("database is newer than this application".into()))?;
        if *version != expected.version
            || name != expected.name
            || *recorded_checksum != checksum(expected.sql)
        {
            return Err(AppError::Migration(
                "migration history, name or checksum mismatch".into(),
            ));
        }
    }
    Ok(applied.len())
}

fn backup_before_upgrade(
    connection: &Connection,
    data_directory: &Path,
    old_version: u32,
) -> AppResult<()> {
    let directory = data_directory.join("backups");
    ensure_directory(&directory, false)?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AppError::Internal)?
        .as_nanos();
    let filename = format!(
        "before-v{old_version}-{timestamp}-{}.sqlite3",
        std::process::id()
    );
    let destination = directory.join(&filename);
    let partial = directory.join(format!("{filename}.partial"));
    // Reserve a fresh file. Never overwrite an earlier recovery point.
    drop(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial)?,
    );
    connection.backup(MAIN_DB, &partial, NO_PROGRESS)?;
    fs::rename(partial, destination)?;
    Ok(())
}

#[cfg(test)]
mod tests;
