use super::*;

const SECOND: Migration = Migration {
    version: CURRENT_SCHEMA_VERSION + 1,
    name: "test_extension",
    sql: "CREATE TABLE test_extension (id INTEGER PRIMARY KEY, value TEXT NOT NULL) STRICT;",
};

#[test]
fn folder_import_upgrade_from_v11_preserves_existing_facts_and_creates_only_empty_import_tables() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = Connection::open(dir.path().join("library.sqlite3")).unwrap();
    c.pragma_update(None, "foreign_keys", true).unwrap();
    migrate_with(&mut c, dir.path(), &MIGRATIONS[..11]).unwrap();
    c.execute_batch("INSERT INTO media(id,file_path,file_name,media_type,file_size,modified_time,imported_at,last_seen_at,created_seq) VALUES('keep','/existing/中文.mp4','中文.mp4','video',1,1,42,1,1);
    INSERT INTO like_events(id,request_id,ingest_seq,media_id,occurred_at,recorded_at) VALUES('like','like',2,'keep',1,1);
    INSERT INTO view_events(id,request_id,ingest_seq,media_id,activation_id,occurred_at,recorded_at) VALUES('view','view',3,'keep','activation',1,1);
    INSERT INTO play_events(id,request_id,ingest_seq,media_id,content_version_id,player_session_id,started_at,qualified_at,start_position,end_position,watched_seconds,completed) VALUES('play','play',4,'keep',1,'session',1,2,0,6,6,0);
    INSERT INTO custom_tags(id,name,created_at,updated_at) VALUES('tag','Keep',1,1);
    INSERT INTO media_custom_tags VALUES('keep','tag');
    INSERT INTO playback_states(media_id,content_version_id,position_seconds,duration_seconds,completed,updated_at) VALUES('keep',1,4,10,0,1);").unwrap();
    fn facts(c: &Connection) -> Vec<String> {
        [
            "media",
            "like_events",
            "view_events",
            "play_events",
            "custom_tags",
            "media_custom_tags",
            "playback_states",
        ]
        .iter()
        .map(|table| {
            let mut statement = c.prepare(&format!("SELECT * FROM {table}")).unwrap();
            let mut rows = statement.query([]).unwrap();
            let row = rows.next().unwrap().unwrap();
            (0..row.as_ref().column_count())
                .map(|n| format!("{:?}", row.get_ref(n).unwrap()))
                .collect::<Vec<_>>()
                .join("|")
        })
        .collect()
    }
    let before = facts(&c);
    migrate(&mut c, dir.path()).unwrap();
    assert_eq!(facts(&c), before);
    for table in [
        "storage_roots",
        "import_sources",
        "import_source_tags",
        "media_import_sources",
        "media_import_tags",
        "import_jobs",
        "import_entries",
        "media_full_hashes",
        "managed_media",
    ] {
        assert_eq!(
            c.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    assert!(!c
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .exists([])
        .unwrap());
    let backup = fs::read_dir(dir.path().join("backups"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let old = Connection::open(backup).unwrap();
    assert_eq!(schema_version(&old).unwrap(), 11);
    assert_eq!(facts(&old), before);
    migrate(&mut c, dir.path()).unwrap();
    assert_eq!(facts(&c), before);
}

#[test]
fn first_release_upgrades_every_tracked_older_schema_and_keeps_backup_restorable() {
    for version in 1..CURRENT_SCHEMA_VERSION as usize {
        let dir = tempfile::tempdir().unwrap();
        let mut c = Connection::open(dir.path().join("upgrade.sqlite3")).unwrap();
        c.pragma_update(None, "foreign_keys", true).unwrap();
        migrate_with(&mut c, dir.path(), &MIGRATIONS[..version]).unwrap();
        c.execute(
            "INSERT INTO app_metadata(key,value) VALUES('release-sentinel','retained')",
            [],
        )
        .unwrap();
        if version >= 2 {
            c.execute_batch("INSERT INTO media(id,file_path,file_name,media_type,file_size,modified_time,imported_at,last_seen_at,created_seq) VALUES('keep','/synthetic/中文.mp4','中文.mp4','video',1,1,1,1,1);
              INSERT INTO like_events(id,request_id,ingest_seq,media_id,occurred_at,recorded_at) VALUES('keep-like','keep-like',2,'keep',1,1);
              INSERT INTO playback_states(media_id,content_version_id,position_seconds,duration_seconds,completed,updated_at) VALUES('keep',1,20,100,0,1);").unwrap();
        }
        migrate(&mut c, dir.path()).unwrap();
        assert_eq!(schema_version(&c).unwrap(), CURRENT_SCHEMA_VERSION);
        if version >= 2 {
            let kept: (i64, f64) = c
                .query_row(
                    "SELECT like_count,playback_position FROM media WHERE id='keep'",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            assert_eq!(kept, (1, 20.0));
        }
        assert!(!c
            .prepare("PRAGMA foreign_key_check")
            .unwrap()
            .exists([])
            .unwrap());
        let backup = fs::read_dir(dir.path().join("backups"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        drop(c);
        let mut restored = Connection::open(backup).unwrap();
        assert_eq!(schema_version(&restored).unwrap(), version as u32);
        assert_eq!(
            restored
                .query_row(
                    "SELECT value FROM app_metadata WHERE key='release-sentinel'",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            "retained"
        );
        // Reopening the recovery point can run the same official migration chain.
        migrate(&mut restored, dir.path()).unwrap();
        assert_eq!(schema_version(&restored).unwrap(), CURRENT_SCHEMA_VERSION);
    }
}

#[test]
fn phase11_index_upgrade_preserves_scaled_v10_data_and_is_idempotent() {
    for size in [10000, 100000] {
        let directory = tempfile::tempdir().unwrap();
        let mut c = Connection::open(directory.path().join("upgrade.sqlite3")).unwrap();
        c.pragma_update(None, "foreign_keys", true).unwrap();
        migrate_with(&mut c, directory.path(), &MIGRATIONS[..10]).unwrap();
        c.execute("WITH RECURSIVE n(v) AS(VALUES(1) UNION ALL SELECT v+1 FROM n WHERE v<?1)
          INSERT INTO media(id,file_path,file_name,media_type,file_size,modified_time,imported_at,last_seen_at,created_seq)
          SELECT printf('m%06d',v),printf('/synthetic/%06d.jpg',v),'test.jpg','image',1,1,v,1,v FROM n",[size]).unwrap();
        c.execute_batch("INSERT INTO like_events(id,request_id,ingest_seq,media_id,occurred_at,recorded_at) VALUES('retained','retained',200000,'m000001',1,1);
          INSERT INTO playback_states(media_id,content_version_id,position_seconds,duration_seconds,completed,updated_at) VALUES('m000001',1,20,100,0,1);").unwrap();
        let started = std::time::Instant::now();
        migrate(&mut c, directory.path()).unwrap();
        let upgrade = started.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(
            c.query_row("SELECT count(*) FROM media", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            size
        );
        assert_eq!(
            c.query_row("SELECT like_count FROM media WHERE id='m000001'", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            c.query_row("SELECT position_seconds FROM playback_states", [], |r| r
                .get::<_, f64>(0))
                .unwrap(),
            20.0
        );
        let backups: Vec<_> = fs::read_dir(directory.path().join("backups"))
            .unwrap()
            .map(|r| r.unwrap().path())
            .collect();
        assert_eq!(backups.len(), 1);
        let backup = Connection::open(&backups[0]).unwrap();
        assert_eq!(schema_version(&backup).unwrap(), 10);
        assert_eq!(
            backup
                .query_row("SELECT count(*) FROM media", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            size
        );
        assert!(!c
            .prepare("PRAGMA foreign_key_check")
            .unwrap()
            .exists([])
            .unwrap());
        migrate(&mut c, directory.path()).unwrap();
        assert_eq!(
            fs::read_dir(directory.path().join("backups"))
                .unwrap()
                .count(),
            1
        );
        println!("MIGRATION_SCALE media={size} v10_to_v11_including_backup_ms={upgrade:.3}");
    }
}

#[test]
fn reports_upgrade_preserves_v9_logs_progress_and_backup_then_reopens() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("reports.sqlite3");
    let mut connection = Connection::open(&file).unwrap();
    connection
        .pragma_update(None, "foreign_keys", true)
        .unwrap();
    migrate_with(&mut connection, directory.path(), &MIGRATIONS[..9]).unwrap();
    connection.execute_batch("INSERT INTO media(id,file_path,file_name,media_type,file_size,modified_time,imported_at,last_seen_at,created_seq,duration)
      VALUES('legacy','/synthetic/legacy.mp4','legacy.mp4','video',0,1,1,1,1,100);
      INSERT INTO playback_states(media_id,content_version_id,position_seconds,duration_seconds,completed,updated_at) VALUES('legacy',1,95,100,0,1);
      INSERT INTO like_events(id,ingest_seq,request_id,media_id,occurred_at,recorded_at) VALUES('like',2,'like','legacy',10,10);").unwrap();
    let facts = |c: &Connection| {
        c.query_row("SELECT (SELECT count(*) FROM like_events),(SELECT like_count FROM media WHERE id='legacy'),position_seconds,completed FROM playback_states WHERE media_id='legacy'",[],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,f64>(2)?,r.get::<_,i64>(3)?))).unwrap()
    };
    let before = facts(&connection);
    migrate(&mut connection, directory.path()).unwrap();
    assert_eq!(facts(&connection), before);
    assert_eq!(
        connection
            .query_row("SELECT completed FROM current_playback_progress", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM play_events", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let backups: Vec<_> = fs::read_dir(directory.path().join("backups"))
        .unwrap()
        .map(|r| r.unwrap().path())
        .collect();
    let backup = Connection::open(&backups[0]).unwrap();
    assert_eq!(schema_version(&backup).unwrap(), 9);
    assert_eq!(facts(&backup), before);
    drop(connection);
    let mut reopened = Connection::open(file).unwrap();
    migrate(&mut reopened, directory.path()).unwrap();
    assert_eq!(facts(&reopened), before);
    assert_eq!(schema_version(&reopened).unwrap(), CURRENT_SCHEMA_VERSION);
    assert!(!reopened
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .exists([])
        .unwrap());
}

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
        Migration {
            version: 7,
            name: MIGRATIONS[6].name,
            sql: MIGRATIONS[6].sql,
        },
        Migration {
            version: 8,
            name: MIGRATIONS[7].name,
            sql: MIGRATIONS[7].sql,
        },
        Migration {
            version: 9,
            name: MIGRATIONS[8].name,
            sql: MIGRATIONS[8].sql,
        },
        Migration {
            version: 10,
            name: MIGRATIONS[9].name,
            sql: MIGRATIONS[9].sql,
        },
        Migration {
            version: 11,
            name: MIGRATIONS[10].name,
            sql: MIGRATIONS[10].sql,
        },
        Migration {
            version: 12,
            name: MIGRATIONS[11].name,
            sql: MIGRATIONS[11].sql,
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
