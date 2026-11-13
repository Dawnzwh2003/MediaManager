use media_manager::{
    db::Database,
    domain::library::{MediaType, NewMedia},
    platform::paths::AppPaths,
    services::app_status::AppState,
};
use rusqlite::Connection;
use std::time::{Duration, Instant};

#[test]
fn cancelled_scan_recovers_without_losing_published_media() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("media");
    std::fs::create_dir(&root).unwrap();
    for n in 0..1000 {
        std::fs::write(root.join(format!("{n}.jpg")), b"synthetic").unwrap();
    }
    let paths = AppPaths::prepare(&temp.path().join("app")).unwrap();
    let mut db = Database::open(&paths).unwrap();
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = cancel.clone();
    let stop = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(5));
        flag.store(true, std::sync::atomic::Ordering::Relaxed);
    });
    let interrupted = db.scan(&root, &cancel).unwrap();
    stop.join().unwrap();
    assert!(interrupted.cancelled);
    let sql = Connection::open(&paths.database).unwrap();
    assert_eq!(
        interrupted.new as i64,
        sql.query_row("SELECT count(*) FROM scan_evidence", [], |r| r
            .get::<_, i64>(0))
            .unwrap()
    );
    cancel.store(false, std::sync::atomic::Ordering::Relaxed);
    let recovered = db.scan(&root, &cancel).unwrap();
    assert!(!recovered.cancelled);
    assert_eq!(
        sql.query_row("SELECT count(*) FROM media WHERE state='active'", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1000
    );
}

fn fixture() -> (tempfile::TempDir, AppPaths, Database) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::prepare(&temp.path().join("app")).unwrap();
    let mut db = Database::open(&paths).unwrap();
    db.repositories()
        .media()
        .insert(&NewMedia {
            id: "video".into(),
            fingerprint: None,
            file_path: temp.path().join("synthetic.mp4").to_string_lossy().into(),
            file_name: "synthetic.mp4".into(),
            media_type: MediaType::Video,
            file_size: 0,
            created_time: None,
            modified_time: 1,
            imported_at: 1,
            last_seen_at: 1,
            width: None,
            height: None,
            duration: Some(100.0),
        })
        .unwrap();
    (temp, paths, db)
}

#[test]
fn existing_database_is_available_before_background_recovery_can_acquire_a_writer() {
    let (_temp, paths, db) = fixture();
    drop(db);
    let writer = Connection::open(&paths.database).unwrap();
    writer
        .execute_batch(
            "BEGIN IMMEDIATE; INSERT INTO app_metadata(key,value) VALUES('held-writer','test');",
        )
        .unwrap();
    let state = AppState::new(paths.data.parent().unwrap().to_owned());
    let started = Instant::now();
    let status = state.status();
    let elapsed = started.elapsed();
    writer.execute_batch("ROLLBACK;").unwrap();
    assert!(
        status.is_ok(),
        "background recovery must not gate the existing database/UI: {status:?}"
    );
    assert!(
        elapsed < Duration::from_secs(1),
        "startup waited for a recovery writer: {elapsed:?}"
    );
    assert!(state.status().is_ok());
}

#[test]
fn opening_a_player_waits_for_a_short_writer_without_losing_recovery_state() {
    let (_temp, paths, mut db) = fixture();
    let (send, receive) = std::sync::mpsc::channel();
    let path = paths.database.clone();
    let writer = std::thread::spawn(move || {
        let c = Connection::open(path).unwrap();
        c.execute_batch("BEGIN IMMEDIATE; UPDATE db_sequence SET value=value+1 WHERE id=1;")
            .unwrap();
        send.send(()).unwrap();
        std::thread::sleep(Duration::from_millis(150));
        c.execute_batch("COMMIT;").unwrap();
    });
    receive.recv().unwrap();
    let result = db.repositories().playback().claim("video", 1, 100);
    writer.join().unwrap();
    assert!(
        result.is_ok(),
        "claim upgraded a deferred snapshot instead of waiting: {result:?}"
    );
    assert_eq!(result.unwrap().1.position_seconds, 0.0);
    assert_eq!(
        db.repositories()
            .events()
            .counts("video")
            .unwrap()
            .unwrap()
            .plays,
        0
    );
}

#[test]
fn scanning_tags_media_in_the_root_itself_as_well_as_descendant_directories() {
    let (temp, _paths, mut db) = fixture();
    let root = temp.path().join("source");
    std::fs::create_dir_all(root.join("child")).unwrap();
    std::fs::write(root.join("own.jpg"), b"synthetic").unwrap();
    std::fs::write(root.join("child/nested.jpg"), b"synthetic").unwrap();
    let report = db
        .scan(&root, &std::sync::atomic::AtomicBool::new(false))
        .unwrap();
    assert_eq!(report.new, 2);
    let root = std::fs::canonicalize(root)
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let filter = media_manager::domain::library_query::LibraryFilter {
        system_tag_id: Some(root),
        ..Default::default()
    };
    assert_eq!(
        db.repositories()
            .library()
            .query(&filter, None, 100)
            .unwrap()
            .items
            .len(),
        2
    );
    let rescan = db
        .scan(
            temp.path().join("source").as_path(),
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap();
    assert_eq!(rescan.unchanged, 2);
    assert_eq!(rescan.fingerprinted, 0);
    assert_eq!(
        db.repositories()
            .library()
            .query(&filter, None, 100)
            .unwrap()
            .items
            .len(),
        2
    );
}

#[test]
fn unexpected_exit_child() {
    let Some(root) = std::env::var_os("MEDIA_MANAGER_CRASH_FIXTURE") else {
        return;
    };
    let paths = AppPaths::prepare(std::path::Path::new(&root)).unwrap();
    let mut db = Database::open(&paths).unwrap();
    media_manager::services::activity::add_like(&mut db, "video", "committed-before-crash")
        .unwrap();
    let sql = Connection::open(&paths.database).unwrap();
    sql.execute_batch("UPDATE media_derivatives SET status='running';
      INSERT INTO playback_states(media_id,content_version_id,position_seconds,duration_seconds,completed,updated_at) VALUES('video',1,20,100,0,1);
      BEGIN IMMEDIATE;
      INSERT INTO like_events(id,request_id,ingest_seq,media_id,occurred_at,recorded_at) VALUES('uncommitted','uncommitted',999,'video',1,1);
      UPDATE media SET like_count=999 WHERE id='video';").unwrap();
    std::process::exit(73);
}

#[test]
fn actual_unexpected_exit_retains_committed_facts_rolls_back_partial_writes_and_recovers_queue() {
    let (_temp, paths, db) = fixture();
    drop(db);
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "unexpected_exit_child", "--nocapture"])
        .env("MEDIA_MANAGER_CRASH_FIXTURE", paths.data.parent().unwrap());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    assert_eq!(command.output().unwrap().status.code(), Some(73));
    let mut db = Database::open(&paths).unwrap();
    let counts = db.repositories().events().counts("video").unwrap().unwrap();
    assert_eq!((counts.likes, counts.views, counts.plays), (1, 0, 0));
    assert_eq!(
        db.repositories()
            .media()
            .get("video")
            .unwrap()
            .unwrap()
            .like_count,
        1
    );
    assert_eq!(
        db.repositories()
            .playback()
            .get("video", 1)
            .unwrap()
            .unwrap()
            .position_seconds,
        20.0
    );
    let worker = db
        .start_thumbnail_worker(media_manager::metadata::Toolchain::default())
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let status = db.request_thumbnail("video", false).unwrap();
        if status.status == "failed" {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(worker);
    let sql = Connection::open(&paths.database).unwrap();
    assert_eq!(
        sql.query_row("PRAGMA quick_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
    assert!(!sql
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .exists([])
        .unwrap());
}

#[test]
fn overwrite_reconciliation_preserves_both_histories_when_identity_evidence_changes() {
    let (temp, paths, mut db) = fixture();
    let root = temp.path().join("overwrite");
    std::fs::create_dir(&root).unwrap();
    let source = root.join("a.mp4");
    let target = root.join("b.mp4");
    std::fs::write(&source, b"first synthetic file").unwrap();
    std::thread::sleep(Duration::from_millis(75));
    std::fs::write(&target, b"different synthetic file").unwrap();
    db.scan(&root, &std::sync::atomic::AtomicBool::new(false))
        .unwrap();
    let items = db.repositories().media().list(None, 100).unwrap();
    let a = items.iter().find(|m| m.file_name == "a.mp4").unwrap();
    let b = items.iter().find(|m| m.file_name == "b.mp4").unwrap();
    let a_id = a.id.clone();
    let b_id = b.id.clone();
    let imported = a.imported_at;
    media_manager::services::activity::add_like(&mut db, &a_id, "like-source").unwrap();
    media_manager::services::activity::add_like(&mut db, &b_id, "like-destination").unwrap();
    db.repositories()
        .custom_tags()
        .create("keep", "Keep", 1)
        .unwrap();
    db.repositories()
        .custom_tags()
        .attach(&a_id, "keep")
        .unwrap();
    let (owner, state) = db.repositories().playback().claim(&a_id, 1, 1).unwrap();
    db.repositories()
        .playback()
        .save(
            &a_id,
            1,
            &media_manager::domain::viewer::ProgressWrite {
                token: owner,
                sequence: 1,
                expected_version: state.state_version,
                position_seconds: 20.0,
                duration_seconds: Some(100.0),
            },
            2,
        )
        .unwrap();
    let source_before = media_manager::identity::Evidence::read(&source).unwrap();
    let target_before = media_manager::identity::Evidence::read(&target).unwrap();
    std::fs::remove_file(&target).unwrap();
    std::fs::rename(&source, &target).unwrap();
    let target_after = media_manager::identity::Evidence::read(&target).unwrap();
    println!("OVERWRITE_EVIDENCE source_before={source_before:?} target_before={target_before:?} target_after={target_after:?}");
    let report = db.scan(&root, &std::sync::atomic::AtomicBool::new(false));
    assert!(report.is_ok(),"confirmed overwrite must retire the old destination before relocating the source: {report:?}");
    let moved = db.repositories().media().get(&a_id).unwrap().unwrap();
    assert_eq!(
        (moved.imported_at, moved.like_count, moved.playback_position),
        (imported, 1, 20.0)
    );
    // Windows can tunnel the deleted target's creation time onto the source.
    // Birth time is part of the approved identity evidence, so that case must
    // take the conservative missing + new path rather than inherit history.
    if source_before.key.is_some() && source_before.key == target_after.key {
        assert_eq!(moved.file_name, "b.mp4");
        assert!(!moved.missing);
    } else {
        assert_eq!(moved.file_name, "a.mp4");
        assert!(moved.missing);
        let active = db
            .repositories()
            .media()
            .list(None, 100)
            .unwrap()
            .into_iter()
            .find(|m| m.file_name == "b.mp4" && !m.missing)
            .unwrap();
        assert_ne!(active.id, a_id);
        assert_ne!(active.id, b_id);
        assert_eq!((active.like_count, active.playback_position), (0, 0.0));
    }
    assert_eq!(
        db.repositories()
            .custom_tags()
            .for_media(&a_id, None, 10)
            .unwrap()
            .len(),
        1
    );
    let retired = db.repositories().media().get(&b_id).unwrap().unwrap();
    assert!(retired.missing);
    assert_eq!(retired.state, "superseded");
    let sql = Connection::open(&paths.database).unwrap();
    assert_eq!(
        sql.query_row("SELECT count(*) FROM like_events", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        2
    );
}
