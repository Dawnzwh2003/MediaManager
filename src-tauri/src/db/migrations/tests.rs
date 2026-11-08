use super::*;

const SECOND: Migration = Migration {
    version: 7,
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
    assert_eq!(schema_version(&connection).unwrap(), CURRENT_SCHEMA_VERSION);
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
    assert_eq!(schema_version(&connection).unwrap(), CURRENT_SCHEMA_VERSION);
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
    assert_eq!(schema_version(&connection).unwrap(), CURRENT_SCHEMA_VERSION);
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
    // Repair only the tampered checksum, then remove an interior ledger entry.
    // Updating every remaining version to 2 collides once v3 also exists.
    connection
        .execute(
            "UPDATE schema_migrations SET checksum=?1 WHERE version=1",
            [checksum(MIGRATIONS[0].sql)],
        )
        .unwrap();
    connection
        .execute("DELETE FROM schema_migrations WHERE version=2", [])
        .unwrap();
    let versions: Vec<u32> = connection
        .prepare("SELECT version FROM schema_migrations ORDER BY version")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        versions,
        (1..=CURRENT_SCHEMA_VERSION)
            .filter(|v| *v != 2)
            .collect::<Vec<_>>()
    );
    let schema_before: String = connection
        .query_row("SELECT group_concat(sql) FROM sqlite_schema", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert!(matches!(
        migrate(&mut connection, directory.path()),
        Err(AppError::Migration(_))
    ));
    assert!(!directory.path().join("backups").exists());
    let schema_after: String = connection
        .query_row("SELECT group_concat(sql) FROM sqlite_schema", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(schema_after, schema_before);
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
        Migration {
            version: 3,
            name: MIGRATIONS[2].name,
            sql: MIGRATIONS[2].sql,
        },
        Migration {
            version: 4,
            name: MIGRATIONS[3].name,
            sql: MIGRATIONS[3].sql,
        },
        Migration {
            version: 5,
            name: MIGRATIONS[4].name,
            sql: MIGRATIONS[4].sql,
        },
        Migration {
            version: 6,
            name: MIGRATIONS[5].name,
            sql: MIGRATIONS[5].sql,
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

#[test]
fn upgrade_preserves_playback_states_and_versioned_cache_without_play_events() {
    for recursive in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let mut connection = Connection::open(directory.path().join("playback.sqlite3")).unwrap();
        connection
            .pragma_update(None, "foreign_keys", true)
            .unwrap();
        connection
            .pragma_update(None, "recursive_triggers", recursive)
            .unwrap();
        migrate_with(&mut connection, directory.path(), &MIGRATIONS[..2]).unwrap();
        connection.execute_batch(
            "BEGIN;
             UPDATE db_sequence SET value=value+1 WHERE id=1;
             INSERT INTO media(id,file_path,file_name,media_type,file_size,modified_time,imported_at,last_seen_at,created_seq)
             SELECT 'm1','/synthetic.mp4','synthetic.mp4','video',42,1,1,1,value FROM db_sequence WHERE id=1;
             INSERT INTO playback_states VALUES('m1',1,9,100,0,1,1);
             COMMIT;",
        ).unwrap();
        let trigger_before: String = connection
            .query_row(
                "SELECT sql FROM sqlite_schema WHERE name='playback_cache_version'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let assert_cache = |connection: &Connection, expected: f64| {
            let (cache, source, plays, count): (f64, f64, i64, i64) = connection.query_row(
                "SELECT playback_position,
                 coalesce((SELECT position_seconds FROM playback_states WHERE media_id=media.id AND content_version_id=media.current_content_version_id),0),
                 (SELECT count(*) FROM play_events),play_count FROM media WHERE id='m1'",
                [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)),
            ).unwrap();
            assert_eq!(cache, expected, "recursive_triggers={recursive}");
            assert_eq!(cache, source);
            assert_eq!((plays, count), (0, 0));
        };
        assert_cache(&connection, 9.0);
        migrate(&mut connection, directory.path()).unwrap();
        assert_eq!(schema_version(&connection).unwrap(), CURRENT_SCHEMA_VERSION);
        assert_cache(&connection, 9.0);
        let trigger_after: String = connection
            .query_row(
                "SELECT sql FROM sqlite_schema WHERE name='playback_cache_version'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(trigger_after, trigger_before);
        let backups: Vec<_> = fs::read_dir(directory.path().join("backups"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(backups.len(), 1);
        let backup = Connection::open(&backups[0]).unwrap();
        assert_eq!(schema_version(&backup).unwrap(), 2);
        assert_cache(&backup, 9.0);

        connection
            .execute_batch(
                "INSERT INTO media_revisions VALUES('m1',2,'new',64,2);
            UPDATE media SET current_content_version_id=2 WHERE id='m1';",
            )
            .unwrap();
        assert_cache(&connection, 0.0);
        let old_position: f64 = connection.query_row(
            "SELECT position_seconds FROM playback_states WHERE media_id='m1' AND content_version_id=1", [], |r| r.get(0),
        ).unwrap();
        assert_eq!(old_position, 9.0);
        connection
            .execute_batch("INSERT INTO playback_states VALUES('m1',2,12,100,0,2,1);")
            .unwrap();
        assert_cache(&connection, 12.0);
        connection
            .execute_batch(
                "UPDATE playback_states SET position_seconds=13 WHERE content_version_id=2;",
            )
            .unwrap();
        assert_cache(&connection, 13.0);
        connection
            .execute_batch(
                "UPDATE playback_states SET position_seconds=5 WHERE content_version_id=1;",
            )
            .unwrap();
        assert_cache(&connection, 13.0);
        connection
            .execute_batch("UPDATE media SET current_content_version_id=1 WHERE id='m1';")
            .unwrap();
        assert_cache(&connection, 5.0);
        connection
            .execute_batch(
                "UPDATE media SET current_content_version_id=2 WHERE id='m1';
            DELETE FROM playback_states WHERE content_version_id=1;",
            )
            .unwrap();
        assert_cache(&connection, 13.0);
        connection
            .execute_batch("DELETE FROM playback_states WHERE content_version_id=2;")
            .unwrap();
        assert_cache(&connection, 0.0);
        {
            let transaction = connection.transaction().unwrap();
            transaction
                .execute_batch("INSERT INTO playback_states VALUES('m1',2,7,100,0,3,1);")
                .unwrap();
            assert_cache(&transaction, 7.0);
            transaction.rollback().unwrap();
        }
        assert_cache(&connection, 0.0);
        {
            let transaction = connection.transaction().unwrap();
            transaction
                .execute(
                    "UPDATE media SET current_content_version_id=99 WHERE id='m1'",
                    [],
                )
                .unwrap();
            assert!(transaction.commit().is_err());
        }
        assert_cache(&connection, 0.0);
        assert_eq!(
            connection
                .query_row(
                    "SELECT current_content_version_id FROM media WHERE id='m1'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            2
        );
        assert!(!connection
            .prepare("PRAGMA foreign_key_check")
            .unwrap()
            .exists([])
            .unwrap());
        drop(connection);
        let mut reopened = Connection::open(directory.path().join("playback.sqlite3")).unwrap();
        migrate(&mut reopened, directory.path()).unwrap();
        assert_cache(&reopened, 0.0);
    }
}
