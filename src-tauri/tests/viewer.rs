use media_manager::{
    db::Database,
    domain::{
        library_query::{LibraryFilter, LibrarySort},
        viewer::*,
    },
    error::AppError,
    metadata::Toolchain,
    platform::paths::AppPaths,
    viewer::ViewerService,
};
use std::{fs, path::PathBuf, sync::atomic::AtomicBool};
fn fixture() -> (
    tempfile::TempDir,
    AppPaths,
    Database,
    ViewerService,
    String,
    String,
) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("media");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("a.png"), b"corrupt image test input").unwrap();
    fs::write(root.join("b.mp4"), b"0123456789abcdef").unwrap();
    let paths = AppPaths::prepare(&temp.path().join("app")).unwrap();
    let mut db = Database::open(&paths).unwrap();
    db.scan(&root, &AtomicBool::new(false)).unwrap();
    let items = db.repositories().media().list(None, 100).unwrap();
    let image = items
        .iter()
        .find(|m| m.file_name == "a.png")
        .unwrap()
        .id
        .clone();
    let video = items
        .iter()
        .find(|m| m.file_name == "b.mp4")
        .unwrap()
        .id
        .clone();
    (temp, paths, db, ViewerService::default(), image, video)
}
fn request(token: &str, sequence: i64, version: i64, position: f64) -> ProgressWrite {
    ProgressWrite {
        token: token.into(),
        sequence,
        expected_version: version,
        position_seconds: position,
        duration_seconds: Some(100.0),
    }
}
fn assert_no_events(paths: &AppPaths, db: &mut Database) {
    let c = rusqlite::Connection::open(&paths.database).unwrap();
    let count:i64=c.query_row("SELECT (SELECT count(*) FROM like_events)+(SELECT count(*) FROM view_events)+(SELECT count(*) FROM play_events)",[],|r|r.get(0)).unwrap();
    assert_eq!(count, 0);
    for m in db.repositories().media().list(None, 100).unwrap() {
        assert_eq!((m.like_count, m.view_count, m.play_count), (0, 0, 0));
    }
}
#[test]
fn resource_tokens_are_read_only_revocable_and_range_bounded() {
    let (_temp, paths, mut db, service, image, video) = fixture();
    let source = service.open(&mut db, &video).unwrap();
    let url = format!("/{}/direct", source.token);
    let range = service
        .read(&mut db, &url, "GET", Some("bytes=2-5"))
        .unwrap();
    assert_eq!((range.status, range.body), (206, b"2345".to_vec()));
    assert_eq!(range.content_range.as_deref(), Some("bytes 2-5/16"));
    let head = service.read(&mut db, &url, "HEAD", None).unwrap();
    assert!(head.body.is_empty());
    assert_eq!(head.content_length, 16);
    assert_eq!(
        service.read(&mut db, &url, "POST", None).unwrap().status,
        405
    );
    assert_eq!(
        service
            .read(&mut db, &url, "GET", Some("bytes=99-"))
            .unwrap()
            .status,
        416
    );
    assert_eq!(
        service
            .read(&mut db, "/../../private/direct", "GET", None)
            .unwrap()
            .status,
        404
    );
    let original = db.repositories().media().get(&video).unwrap().unwrap();
    fs::rename(
        &original.file_path,
        PathBuf::from(&original.file_path).with_file_name("moved.mp4"),
    )
    .unwrap();
    db.scan(
        PathBuf::from(&original.file_path).parent().unwrap(),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(
        service.read(&mut db, &url, "GET", None).unwrap().body,
        b"0123456789abcdef"
    );
    service.close(&mut db, &source.token).unwrap();
    assert!(service.read(&mut db, &url, "GET", None).is_err());
    let image = service.open(&mut db, &image).unwrap();
    assert!(image.state.is_none());
    assert!(service
        .save(&mut db, &request(&image.token, 1, 1, 3.0))
        .is_err());
    assert_no_events(&paths, &mut db);
}
#[test]
fn page_unload_revokes_resources_and_releases_session_capacity() {
    let (_temp, _paths, mut db, service, image, _video) = fixture();
    let opened = service.open(&mut db, &image).unwrap();
    service.cancel_all();
    assert!(service
        .read(&mut db, &format!("/{}/direct", opened.token), "GET", None)
        .is_err());
    for _ in 0..16 {
        service.open(&mut db, &image).unwrap();
        service.cancel_all();
    }
}
#[test]
fn progress_is_idempotent_resumable_and_rejects_old_owner_and_sequence() {
    let (_temp, paths, mut db, service, _image, video) = fixture();
    let opened = service.open(&mut db, &video).unwrap();
    let first = request(&opened.token, 1, opened.state.unwrap().state_version, 12.0);
    let state = service.save(&mut db, &first).unwrap();
    assert_eq!(state.position_seconds, 12.0);
    assert_eq!(
        service.save(&mut db, &first).unwrap().state_version,
        state.state_version
    );
    assert!(service
        .save(
            &mut db,
            &request(&opened.token, 1, state.state_version, 13.0)
        )
        .is_err());
    assert!(service
        .save(&mut db, &request(&opened.token, 2, 1, 13.0))
        .is_err());
    let next = service.open(&mut db, &video).unwrap();
    assert_eq!(next.state.as_ref().unwrap().position_seconds, 12.0);
    assert!(matches!(
        service.save(
            &mut db,
            &request(&opened.token, 2, state.state_version, 40.0)
        ),
        Err(AppError::StaleState)
    ));
    let s = service
        .save(
            &mut db,
            &request(&next.token, 1, next.state.unwrap().state_version, 7.0),
        )
        .unwrap();
    assert_eq!(
        db.repositories()
            .media()
            .get(&video)
            .unwrap()
            .unwrap()
            .playback_position,
        7.0
    );
    service.close(&mut db, &next.token).unwrap();
    drop(service);
    drop(db);
    let mut db = Database::open(&paths).unwrap();
    let service = ViewerService::default();
    let reopened = service.open(&mut db, &video).unwrap();
    assert_eq!(reopened.state.unwrap().position_seconds, 7.0);
    assert!(s.state_version > 1);
    assert_no_events(&paths, &mut db);
}
#[test]
fn source_change_version_switch_and_replacement_cannot_inherit_or_overwrite_state() {
    let (_temp, paths, mut db, service, _image, video) = fixture();
    let opened = service.open(&mut db, &video).unwrap();
    service
        .save(&mut db, &request(&opened.token, 1, 1, 8.0))
        .unwrap();
    let c = rusqlite::Connection::open(&paths.database).unwrap();
    c.pragma_update(None, "foreign_keys", true).unwrap();
    c.execute(
        "INSERT INTO media_revisions VALUES(?1,2,'changed',16,2)",
        [&video],
    )
    .unwrap();
    c.execute(
        "UPDATE media SET current_content_version_id=2 WHERE id=?1",
        [&video],
    )
    .unwrap();
    assert!(service
        .save(&mut db, &request(&opened.token, 2, 2, 70.0))
        .is_err());
    let next = service.open(&mut db, &video).unwrap();
    assert_eq!(next.state.unwrap().position_seconds, 0.0);
    let m = db.repositories().media().get(&video).unwrap().unwrap();
    fs::write(&m.file_path, b"changed length now").unwrap();
    assert!(service
        .read(&mut db, &format!("/{}/direct", next.token), "GET", None)
        .is_err());
    db.scan(
        PathBuf::from(&m.file_path).parent().unwrap(),
        &AtomicBool::new(false),
    )
    .unwrap();
    let new = db
        .repositories()
        .media()
        .list(None, 100)
        .unwrap()
        .into_iter()
        .find(|m| m.file_name == "b.mp4" && !m.missing)
        .unwrap();
    assert_ne!(new.id, video);
    assert_eq!(new.playback_position, 0.0);
    assert_eq!(c.query_row("SELECT position_seconds FROM playback_states WHERE media_id=?1 AND content_version_id=1",[video],|r|r.get::<_,f64>(0)).unwrap(),8.0);
    assert_no_events(&paths, &mut db);
}
#[test]
fn progress_transaction_rolls_back_and_invalid_input_never_counts_plays() {
    let (_temp, paths, mut db, service, _image, video) = fixture();
    let opened = service.open(&mut db, &video).unwrap();
    let c = rusqlite::Connection::open(&paths.database).unwrap();
    c.execute_batch("CREATE TRIGGER fail_progress BEFORE UPDATE OF playback_position ON media BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert!(service
        .save(&mut db, &request(&opened.token, 1, 1, 10.0))
        .is_err());
    c.execute_batch("DROP TRIGGER fail_progress;").unwrap();
    assert_eq!(
        service
            .save(&mut db, &request(&opened.token, 1, 1, 10.0))
            .unwrap()
            .position_seconds,
        10.0
    );
    for position in [-1.0, f64::NAN, f64::INFINITY, 101.0] {
        assert!(service
            .save(&mut db, &request(&opened.token, 2, 2, position))
            .is_err());
    }
    service.cancel_all();
    assert!(service
        .save(&mut db, &request(&opened.token, 2, 2, 20.0))
        .is_err());
    assert_no_events(&paths, &mut db);
}
#[test]
fn navigation_uses_filter_order_across_pages_and_reports_stale_generations() {
    let (_temp, _paths, mut db, _service, image, video) = fixture();
    let f = LibraryFilter {
        sort: LibrarySort::Name,
        ..Default::default()
    };
    let nav = db
        .repositories()
        .library()
        .navigation(&f, &image, None)
        .unwrap();
    assert_eq!(nav.next_id, Some(video.clone()));
    assert!(nav.previous_id.is_none());
    let back = db
        .repositories()
        .library()
        .navigation(&f, &video, Some(nav.generation))
        .unwrap();
    assert_eq!(back.previous_id, Some(image));
    db.repositories()
        .custom_tags()
        .create("new", "New", 1)
        .unwrap();
    assert!(matches!(
        db.repositories()
            .library()
            .navigation(&f, &video, Some(nav.generation)),
        Err(AppError::StaleCursor)
    ));
}
#[test]
fn missing_ffmpeg_and_missing_source_fail_without_changing_originals() {
    let (_temp, paths, mut db, service, _image, video) = fixture();
    let opened = service.open(&mut db, &video).unwrap();
    let tools = Toolchain {
        ffmpeg: paths.cache.join("missing-ffmpeg"),
        ffprobe: paths.cache.join("missing-ffprobe"),
        ..Default::default()
    };
    assert!(service
        .prepare(&mut db, &opened.token, PlaybackMode::Proxy, &tools)
        .is_err());
    let m = db.repositories().media().get(&video).unwrap().unwrap();
    assert_eq!(fs::read(&m.file_path).unwrap(), b"0123456789abcdef");
    service.close(&mut db, &opened.token).unwrap();
    fs::remove_file(&m.file_path).unwrap();
    assert!(service.open(&mut db, &video).is_err());
    assert_no_events(&paths, &mut db);
}
#[test]
#[ignore = "requires real FFmpeg/ffprobe; run with --include-ignored"]
fn real_remux_proxy_cache_and_progress_keep_one_media_and_zero_play_events() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    fs::create_dir(&source).unwrap();
    let video = source.join("synthetic.mkv");
    let tools = Toolchain::default();
    let output = std::process::Command::new(&tools.ffmpeg)
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=128x72:rate=10",
            "-t",
            "2",
            "-c:v",
            "mpeg4",
        ])
        .arg(&video)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = fs::read(&video).unwrap();
    let paths = AppPaths::prepare(&temp.path().join("app")).unwrap();
    let mut db = Database::open(&paths).unwrap();
    db.scan(&source, &AtomicBool::new(false)).unwrap();
    db.process_derivatives(&tools, &AtomicBool::new(false))
        .unwrap();
    let id = db.repositories().media().list(None, 1).unwrap()[0]
        .id
        .clone();
    let service = ViewerService::default();
    let opened = service.open(&mut db, &id).unwrap();
    let mut write = request(&opened.token, 1, 1, 1.0);
    write.duration_seconds = Some(2.0);
    service.save(&mut db, &write).unwrap();
    for mode in [PlaybackMode::Remux, PlaybackMode::Proxy] {
        service
            .prepare(&mut db, &opened.token, mode, &tools)
            .unwrap();
        let reply = service
            .read(
                &mut db,
                &format!("/{}/{}", opened.token, mode.key()),
                "GET",
                Some("bytes=0-31"),
            )
            .unwrap();
        assert_eq!(reply.status, 206);
        assert_eq!(&reply.body[4..8], b"ftyp");
    }
    let cached = fs::read_dir(paths.cache.join("playback"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(cached.len(), 2);
    let times = cached
        .iter()
        .map(|p| fs::metadata(p).unwrap().modified().unwrap())
        .collect::<Vec<_>>();
    service
        .prepare(&mut db, &opened.token, PlaybackMode::Proxy, &tools)
        .unwrap();
    assert_eq!(
        times,
        cached
            .iter()
            .map(|p| fs::metadata(p).unwrap().modified().unwrap())
            .collect::<Vec<_>>()
    );
    let proxy = cached
        .iter()
        .find(|p| {
            tools
                .probe(p, true, &AtomicBool::new(false))
                .unwrap()
                .video_codec
                .as_deref()
                == Some("h264")
        })
        .unwrap();
    assert!(proxy.starts_with(&paths.cache));
    assert_eq!(fs::read(&video).unwrap(), bytes);
    assert_eq!(
        db.repositories()
            .media()
            .get(&id)
            .unwrap()
            .unwrap()
            .playback_position,
        1.0
    );
    assert_no_events(&paths, &mut db);
}
