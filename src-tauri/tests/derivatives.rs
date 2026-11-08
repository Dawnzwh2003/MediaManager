use media_manager::{
    db::Database, identity::Evidence, metadata::Toolchain, platform::paths::AppPaths,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

fn setup() -> (tempfile::TempDir, AppPaths, Database, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::prepare(&temp.path().join("app")).unwrap();
    let db = Database::open(&paths).unwrap();
    let source = temp.path().join("synthetic media");
    fs::create_dir(&source).unwrap();
    (temp, paths, db, source)
}
fn tools() -> Toolchain {
    Toolchain::default()
}
fn generate(path: &Path, video: bool) {
    let result = Command::new(tools().ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-f",
            "lavfi",
            "-i",
            if video {
                "color=c=red:s=160x90:r=10,drawbox=color=blue:t=fill:enable='gte(t,0.1)'"
            } else {
                "color=c=red:s=160x90:r=10"
            },
        ])
        .args(if video {
            vec!["-t", "2", "-c:v", "mpeg4"]
        } else {
            vec!["-frames:v", "1"]
        })
        .arg(path)
        .output()
        .expect("real media tests require FFmpeg in PATH or MEDIA_MANAGER_FFMPEG_DIR");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
fn id(db: &mut Database) -> String {
    db.repositories().media().list(None, 1).unwrap()[0]
        .id
        .clone()
}
fn scan(db: &mut Database, root: &Path) {
    assert_eq!(db.scan(root, &AtomicBool::new(false)).unwrap().failed, 0);
}
fn process(db: &mut Database) -> media_manager::thumbnails::Report {
    db.process_derivatives(&tools(), &AtomicBool::new(false))
        .unwrap()
}
fn sql(paths: &AppPaths) -> rusqlite::Connection {
    let c = rusqlite::Connection::open(&paths.database).unwrap();
    c.pragma_update(None, "foreign_keys", true).unwrap();
    c
}

#[test]
fn missing_tools_are_isolated_persisted_and_explicitly_retryable() {
    let (_temp, paths, mut db, root) = setup();
    fs::write(root.join("bad.png"), b"corrupted image").unwrap();
    fs::write(root.join("bad.mp4"), b"corrupted video").unwrap();
    scan(&mut db, &root);
    let absent = Toolchain {
        ffprobe: root.join("absent-ffprobe"),
        ffmpeg: root.join("absent-ffmpeg"),
        ..tools()
    };
    assert_eq!(
        db.process_derivatives(&absent, &AtomicBool::new(false))
            .unwrap()
            .failed,
        2
    );
    let media = db.repositories().media().list(None, 10).unwrap();
    for m in &media {
        let status = db.request_thumbnail(&m.id, false).unwrap();
        assert_eq!(status.status, "failed");
        assert_eq!(status.metadata_error.as_deref(), Some("ffmpeg_missing"));
        assert_eq!(db.request_thumbnail(&m.id, false).unwrap().attempts, 1);
    }
    assert_eq!(
        db.process_derivatives(&absent, &AtomicBool::new(false))
            .unwrap()
            .failed,
        0
    );
    assert_eq!(
        db.request_thumbnail(&media[0].id, true).unwrap().status,
        "pending"
    );
    drop(db);
    let mut db = Database::open(&paths).unwrap();
    assert_eq!(
        db.request_thumbnail(&media[0].id, false).unwrap().status,
        "pending"
    );
    assert_eq!(
        db.repositories()
            .media()
            .get(&media[0].id)
            .unwrap()
            .unwrap()
            .play_count,
        0
    );
}

#[test]
#[ignore = "requires real ffprobe and FFmpeg; run with --include-ignored"]
fn image_cache_survives_reopen_move_and_recovers_from_eviction_and_changed_content() {
    let (_temp, paths, mut db, root) = setup();
    let original = root.join("image space %.png");
    generate(&original, false);
    let original_bytes = fs::read(&original).unwrap();
    scan(&mut db, &root);
    let first_id = id(&mut db);
    let report = process(&mut db);
    assert_eq!(
        report.ready,
        1,
        "{:?}",
        db.request_thumbnail(&first_id, false).unwrap()
    );
    let m = db.repositories().media().get(&first_id).unwrap().unwrap();
    assert_eq!((m.width, m.height, m.duration), (Some(160), Some(90), None));
    let cached = PathBuf::from(m.thumbnail_path.unwrap());
    assert!(cached.starts_with(&paths.thumbnails));
    let bytes = fs::read(&cached).unwrap();
    assert!(bytes.starts_with(&[0xff, 0xd8]));
    let timestamp = fs::metadata(&cached).unwrap().modified().unwrap();
    drop(db);
    let mut db = Database::open(&paths).unwrap();
    for _ in 0..3 {
        assert_eq!(
            db.request_thumbnail(&first_id, false).unwrap().status,
            "ready"
        );
    }
    assert_eq!(process(&mut db).ready, 0);
    assert_eq!(
        fs::metadata(&cached).unwrap().modified().unwrap(),
        timestamp
    );
    sql(&paths).execute_batch(&format!("INSERT INTO custom_tags VALUES('t','Test',1,1); INSERT INTO media_custom_tags VALUES('{first_id}','t'); INSERT INTO playback_states VALUES('{first_id}',1,4,10,0,1,1); INSERT INTO like_events(id,ingest_seq,request_id,media_id,occurred_at,recorded_at) VALUES('l',999,'r','{first_id}',1,1);")).unwrap();
    let renamed = root.join("renamed.png");
    fs::rename(&original, &renamed).unwrap();
    scan(&mut db, &root);
    assert_eq!(id(&mut db), first_id);
    assert_eq!(
        db.request_thumbnail(&first_id, false)
            .unwrap()
            .thumbnail_path
            .as_deref(),
        cached.to_str()
    );
    assert_eq!(process(&mut db).ready, 0);
    let old = db.repositories().media().get(&first_id).unwrap().unwrap();
    assert_eq!(
        (old.like_count, old.play_count, old.playback_position),
        (1, 0, 4.0)
    );
    assert_eq!(
        db.repositories()
            .custom_tags()
            .for_media(&first_id, None, 10)
            .unwrap()
            .len(),
        1
    );
    fs::remove_file(&cached).unwrap();
    assert_eq!(
        db.request_thumbnail(&first_id, false).unwrap().status,
        "pending"
    );
    assert_eq!(process(&mut db).ready, 1);
    assert_eq!(fs::read(&cached).unwrap(), bytes);
    assert_eq!(fs::read(&renamed).unwrap(), original_bytes);
    // Same-path replacement gets a new logical identity and cannot inherit the old cache/history.
    fs::write(&renamed, b"changed content corrupted image").unwrap();
    scan(&mut db, &root);
    let current = db
        .repositories()
        .media()
        .list(None, 10)
        .unwrap()
        .into_iter()
        .find(|m| !m.missing)
        .unwrap();
    assert_ne!(current.id, first_id);
    assert!(current.thumbnail_path.is_none());
    assert_eq!((current.like_count, current.playback_position), (0, 0.0));
    assert_eq!(process(&mut db).failed, 1);
    assert!(cached.exists());
}

#[test]
#[ignore = "requires real ffprobe and FFmpeg; run with --include-ignored"]
fn video_metadata_thumbnail_and_corruption_are_isolated_without_modifying_sources() {
    let (_temp, paths, mut db, root) = setup();
    let video = root.join("video.mp4");
    generate(&video, true);
    let source = fs::read(&video).unwrap();
    let unsupported_path = root.join("unsupported.avi");
    generate(&unsupported_path, true);
    let mut unsupported = fs::read(&unsupported_path).unwrap();
    let tags = unsupported
        .windows(4)
        .enumerate()
        .filter_map(|(i, w)| (w == b"FMP4").then_some(i))
        .collect::<Vec<_>>();
    assert!(!tags.is_empty());
    for tag in tags {
        unsupported[tag..tag + 4].copy_from_slice(b"ZZZZ");
    }
    fs::write(unsupported_path, unsupported).unwrap();
    fs::write(root.join("broken.mp4"), b"corrupt").unwrap();
    fs::write(root.join("broken.jpg"), b"corrupt").unwrap();
    scan(&mut db, &root);
    let r = process(&mut db);
    assert_eq!((r.ready, r.failed), (1, 3));
    for m in db.repositories().media().list(None, 10).unwrap() {
        let s = db.request_thumbnail(&m.id, false).unwrap();
        if m.file_name == "video.mp4" {
            assert_eq!((m.width, m.height), (Some(160), Some(90)));
            assert_eq!(m.video_codec.as_deref(), Some("mpeg4"));
            assert!((m.duration.unwrap() - 2.0).abs() < 0.1);
            assert_eq!(m.frame_rate, Some(10.0));
            assert_eq!(s.status, "ready");
            let pixel = Command::new(tools().ffmpeg)
                .args(["-v", "error", "-i"])
                .arg(s.thumbnail_path.as_ref().unwrap())
                .args([
                    "-vf",
                    "scale=1:1",
                    "-frames:v",
                    "1",
                    "-f",
                    "rawvideo",
                    "-pix_fmt",
                    "rgb24",
                    "pipe:1",
                ])
                .output()
                .unwrap();
            assert!(pixel.status.success());
            // First frame is red, frames after 0.1s blue: the 10% (0.2s) thumbnail must be blue.
            assert_eq!(pixel.stdout.len(), 3);
            assert!(
                pixel.stdout[2] > 200 && pixel.stdout[0] < 50,
                "{:?}",
                pixel.stdout
            );
            // Decode the published JPEG with the real decoder, independently of the generator's checks.
            assert!(Command::new(tools().ffmpeg)
                .args(["-v", "error", "-i"])
                .arg(s.thumbnail_path.unwrap())
                .args(["-f", "null", "-"])
                .status()
                .unwrap()
                .success());
        } else {
            assert_eq!(s.status, "failed");
            assert!(s.thumbnail_path.is_none());
            if m.file_name == "unsupported.avi" {
                assert_eq!(s.metadata_error.as_deref(), Some("unsupported_codec"));
            }
        }
    }
    assert_eq!(source, fs::read(video).unwrap());
    assert_eq!(
        sql(&paths)
            .query_row("SELECT count(*) FROM play_events", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
#[ignore = "requires real ffprobe and FFmpeg; run with --include-ignored"]
fn every_scanned_image_format_produces_dimensions_and_a_thumbnail() {
    let (_temp, _paths, mut db, root) = setup();
    for extension in ["jpg", "jpeg", "png", "webp", "gif", "bmp"] {
        generate(&root.join(format!("image.{extension}")), false);
    }
    scan(&mut db, &root);
    let r = process(&mut db);
    assert_eq!((r.ready, r.failed), (6, 0));
    for m in db.repositories().media().list(None, 10).unwrap() {
        assert_eq!((m.width, m.height), (Some(160), Some(90)));
        assert!(m.thumbnail_path.is_some());
    }
}

#[test]
#[ignore = "requires real ffprobe and FFmpeg; run with --include-ignored"]
fn thumbnail_failure_preserves_metadata_and_cache_errors_recover() {
    let (_temp, paths, mut db, root) = setup();
    let image = root.join("good.png");
    generate(&image, false);
    scan(&mut db, &root);
    let id = id(&mut db);
    let broken = Toolchain {
        ffmpeg: root.join("missing-ffmpeg"),
        ..tools()
    };
    assert_eq!(
        db.process_derivatives(&broken, &AtomicBool::new(false))
            .unwrap()
            .failed,
        1
    );
    let status = db.request_thumbnail(&id, false).unwrap();
    assert_eq!(status.metadata_status, "ready", "{status:?}");
    assert_eq!(status.thumbnail_error.as_deref(), Some("ffmpeg_missing"));
    assert_eq!(
        db.repositories().media().get(&id).unwrap().unwrap().width,
        Some(160)
    );
    db.request_thumbnail(&id, true).unwrap();
    fs::remove_dir(&paths.thumbnails).unwrap();
    fs::write(&paths.thumbnails, b"blocked").unwrap();
    assert_eq!(process(&mut db).failed, 1);
    assert_eq!(
        db.request_thumbnail(&id, false)
            .unwrap()
            .thumbnail_error
            .as_deref(),
        Some("cache_io")
    );
    fs::remove_file(&paths.thumbnails).unwrap();
    fs::create_dir(&paths.thumbnails).unwrap();
    db.request_thumbnail(&id, true).unwrap();
    assert_eq!(process(&mut db).ready, 1);
    assert!(fs::read_dir(&paths.thumbnails).unwrap().all(|p| p
        .unwrap()
        .path()
        .extension()
        .unwrap()
        == "jpg"));
}

#[test]
fn background_worker_recovers_interruption_and_keeps_status_queries_available() {
    let (_temp, paths, mut db, root) = setup();
    fs::write(root.join("x.png"), b"bad").unwrap();
    scan(&mut db, &root);
    let id = id(&mut db);
    sql(&paths)
        .execute("UPDATE media_derivatives SET status='running'", [])
        .unwrap();
    let missing = Toolchain {
        ffprobe: root.join("missing"),
        ..tools()
    };
    let worker = db.start_thumbnail_worker(missing).unwrap();
    assert!(db.start_thumbnail_worker(tools()).is_err());
    let deadline = Instant::now() + Duration::from_secs(5);
    while db.request_thumbnail(&id, false).unwrap().status != "failed" {
        assert!(Instant::now() < deadline);
        assert_eq!(
            db.status().unwrap().schema_version,
            media_manager::db::migrations::CURRENT_SCHEMA_VERSION
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(worker);
    assert!(matches!(
        Database::open(&paths),
        Err(media_manager::error::AppError::Busy)
    ));
    drop(db);
    assert!(Database::open(&paths).is_ok());
}

#[test]
fn stale_source_is_rejected_before_tools_and_cancel_keeps_durable_pending_work() {
    let (_temp, _paths, mut db, root) = setup();
    let image = root.join("x.png");
    fs::write(&image, b"before").unwrap();
    scan(&mut db, &root);
    let id = id(&mut db);
    assert!(Evidence::read(&image).is_ok());
    let cancelled = db
        .process_derivatives(&tools(), &AtomicBool::new(true))
        .unwrap();
    assert!(cancelled.cancelled);
    assert_eq!(db.request_thumbnail(&id, false).unwrap().attempts, 0);
    fs::write(image, b"after and longer").unwrap();
    assert_eq!(process(&mut db).failed, 1);
    assert_eq!(
        db.request_thumbnail(&id, false)
            .unwrap()
            .metadata_error
            .as_deref(),
        Some("source_changed")
    );
}
