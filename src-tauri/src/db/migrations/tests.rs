use super::*;

const SECOND: Migration = Migration {
    version: 3,
    name: "test_extension",
    sql: "CREATE TABLE test_extension (id INTEGER PRIMARY KEY, value TEXT NOT NULL) STRICT;",
};

#[test]
fn initialization_and_reopen_preserve_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("test.sqlite3");
    {
        let mut connection = Connection::open(&file).unwrap();
        migrate(&mut connection, directory.path()).unwrap();
        connection
            .execute("INSERT INTO app_metadata VALUES ('test','retained')", [])
            .unwrap();
    }
    let mut reopened = Connection::open(&file).unwrap();
    migrate(&mut reopened, directory.path()).unwrap();
    assert_eq!(schema_version(&reopened).unwrap(), CURRENT_SCHEMA_VERSION);
    let value: String = reopened
        .query_row("SELECT value FROM app_metadata WHERE key='test'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(value, "retained");
    assert!(!directory.path().join("backups").exists());
}

#[test]
fn upgrade_creates_a_consistent_backup_and_rejects_downgrade() {
    let directory = tempfile::tempdir().unwrap();
    let mut connection = Connection::open(directory.path().join("test.sqlite3")).unwrap();
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .unwrap();
    migrate_with(&mut connection, directory.path(), &MIGRATIONS[..1]).unwrap();
    connection
        .execute(
            "INSERT INTO app_metadata VALUES ('test','before-upgrade')",
            [],
        )
        .unwrap();
    migrate(&mut connection, directory.path()).unwrap();
    assert_eq!(schema_version(&connection).unwrap(), 2);
    let backups: Vec<_> = fs::read_dir(directory.path().join("backups"))
        .unwrap()
        .map(|r| r.unwrap().path())
        .collect();
    assert_eq!(backups.len(), 1);
    let backup = Connection::open(&backups[0]).unwrap();
    assert_eq!(schema_version(&backup).unwrap(), 1);
    assert_eq!(
        backup
            .query_row("SELECT value FROM app_metadata WHERE key='test'", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "before-upgrade"
    );
    assert!(matches!(
        migrate_with(&mut connection, directory.path(), &MIGRATIONS[..1]),
        Err(AppError::Migration(_))
    ));
    assert_eq!(schema_version(&connection).unwrap(), 2);
}

#[test]
fn failed_migration_rolls_back_ddl_and_ledger_and_can_resume() {
    let directory = tempfile::tempdir().unwrap();
    let mut connection = Connection::open(directory.path().join("test.sqlite3")).unwrap();
    migrate_with(&mut connection, directory.path(), &MIGRATIONS[..1]).unwrap();
    let broken = [
        Migration {
            version: 1,
            name: MIGRATIONS[0].name,
            sql: MIGRATIONS[0].sql,
        },
        Migration {
            version: 2,
            name: "broken",
            sql: "CREATE TABLE partial_result (id INTEGER); INSERT INTO no_such_table VALUES (1);",
        },
    ];
    assert!(migrate_with(&mut connection, directory.path(), &broken).is_err());
    assert_eq!(schema_version(&connection).unwrap(), 1);
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='partial_result')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!exists);
    migrate(&mut connection, directory.path()).unwrap();
    assert_eq!(schema_version(&connection).unwrap(), 2);
}

#[test]
fn changed_checksum_or_history_gap_is_rejected_without_new_ddl() {
    let directory = tempfile::tempdir().unwrap();
    let mut connection = Connection::open_in_memory().unwrap();
    migrate(&mut connection, directory.path()).unwrap();
    connection
        .execute(
            "UPDATE schema_migrations SET checksum='edited' WHERE version=1",
            [],
        )
        .unwrap();
    assert!(matches!(
        migrate(&mut connection, directory.path()),
        Err(AppError::Migration(_))
    ));
    connection
        .execute("DELETE FROM schema_migrations WHERE version=2", [])
        .unwrap();
    connection
        .execute(
            "UPDATE schema_migrations SET checksum=?1,version=2",
            [checksum(MIGRATIONS[0].sql)],
        )
        .unwrap();
    assert!(matches!(
        migrate(&mut connection, directory.path()),
        Err(AppError::Migration(_))
    ));
    assert!(!directory.path().join("backups").exists());
}

#[test]
fn untracked_database_is_not_adopted_or_recreated() {
    let directory = tempfile::tempdir().unwrap();
    let mut connection = Connection::open_in_memory().unwrap();
    // In GLOB '_' is literal; LIKE 'sqlite_%' would incorrectly hide this name.
    connection
        .execute_batch(
            "CREATE TABLE sqliteXuser (value TEXT); INSERT INTO sqliteXuser VALUES ('keep');",
        )
        .unwrap();
    assert!(matches!(
        migrate(&mut connection, directory.path()),
        Err(AppError::Migration(_))
    ));
    assert_eq!(
        connection
            .query_row("SELECT value FROM sqliteXuser", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "keep"
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_schema WHERE name='schema_migrations'",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
        0
    );
}

#[test]
fn failed_first_migration_leaves_a_retryable_empty_database() {
    let directory = tempfile::tempdir().unwrap();
    let mut connection = Connection::open_in_memory().unwrap();
    let broken = [Migration {
        version: 1,
        name: "broken",
        sql: "CREATE TABLE partial_result (id INTEGER); INVALID SQL;",
    }];
    assert!(migrate_with(&mut connection, directory.path(), &broken).is_err());
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_schema WHERE type='table'",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
        0
    );
    migrate(&mut connection, directory.path()).unwrap();
    assert_eq!(schema_version(&connection).unwrap(), CURRENT_SCHEMA_VERSION);
}

#[test]
fn newer_database_is_rejected_before_persistent_connection_changes() {
    let directory = tempfile::tempdir().unwrap();
    let paths = crate::platform::paths::AppPaths::prepare(directory.path()).unwrap();
    let mut connection = Connection::open(&paths.database).unwrap();
    let plan = [
        Migration {
            version: 1,
            name: MIGRATIONS[0].name,
            sql: MIGRATIONS[0].sql,
        },
        Migration {
            version: 2,
            name: MIGRATIONS[1].name,
            sql: MIGRATIONS[1].sql,
        },
        SECOND,
    ];
    migrate_with(&mut connection, &paths.data, &plan).unwrap();
    drop(connection);
    let before = fs::read(&paths.database).unwrap();
    assert!(matches!(
        crate::db::Database::open(&paths),
        Err(AppError::Migration(_))
    ));
    assert_eq!(fs::read(&paths.database).unwrap(), before);
}
