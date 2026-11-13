//! First-release edge cases. All sources and databases are disposable fixtures.
use media_manager::{
    db::Database,
    domain::{
        library_query::{LibraryFilter, SearchScope},
        reports::TimeWindow,
        viewer::ProgressWrite,
    },
    metadata::Toolchain,
    platform::paths::AppPaths,
    services::{activity, app_status::AppState, reports},
    viewer::ViewerService,
};
use std::{fs, sync::atomic::AtomicBool};

#[test]
fn fresh_and_empty_data_folders_initialize_once_without_removing_unrelated_files() {
    for precreate in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("新应用 café");
        if precreate {
            fs::create_dir_all(root.join("data")).unwrap();
        }
        let paths = AppPaths::prepare(&root).unwrap();
        fs::write(paths.data.join("keep.txt"), b"unrelated").unwrap();
        let state = AppState::new(root.clone());
        assert_eq!(
            state.status().unwrap().database.schema_version,
            media_manager::db::migrations::CURRENT_SCHEMA_VERSION
        );
        drop(state);
        let mut db = Database::open(&paths).unwrap();
        assert!(db
            .repositories()
            .media()
            .list(None, 100)
            .unwrap()
            .is_empty());
        let dash = reports::dashboard(
            &mut db,
            TimeWindow {
                start: Some(0),
                end: 200000,
            },
        )
        .unwrap();
        assert_eq!(
            (dash.total_media, dash.total_likes, dash.views_today),
            (0, 0, 0)
        );
        drop(db);
        assert!(Database::open(&paths).is_ok());
        assert_eq!(fs::read(paths.data.join("keep.txt")).unwrap(), b"unrelated");
        assert!(!paths.data.join("backups").exists());
    }
}

#[test]
fn unicode_chinese_long_names_moves_and_restart_preserve_identity_and_progress() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("资料 café 日本語 🐱");
    fs::create_dir(&source).unwrap();
    let names = [
        "中文 空格 #%;'$(echo).mp4".to_owned(),
        format!("{}.jpg", "a".repeat(240)),
        "重名.JPG".into(),
    ];
    for name in &names {
        fs::write(source.join(name), b"synthetic read-only media").unwrap();
    }
    let paths = AppPaths::prepare(&temp.path().join("应用 data")).unwrap();
    let mut db = Database::open(&paths).unwrap();
    assert_eq!(db.scan(&source, &AtomicBool::new(false)).unwrap().new, 3);
    let video = db
        .repositories()
        .media()
        .list(None, 100)
        .unwrap()
        .into_iter()
        .find(|m| m.file_name == names[0])
        .unwrap();
    activity::add_like(&mut db, &video.id, "release-like").unwrap();
    db.repositories()
        .custom_tags()
        .create("tag", "中文 标签", 1)
        .unwrap();
    db.repositories()
        .custom_tags()
        .attach(&video.id, "tag")
        .unwrap();
    let viewer = ViewerService::default();
    let opened = viewer.open(&mut db, &video.id).unwrap();
    viewer
        .save(
            &mut db,
            &ProgressWrite {
                token: opened.token.clone(),
                sequence: 1,
                expected_version: opened.state.unwrap().state_version,
                position_seconds: 20.0,
                duration_seconds: Some(100.0),
            },
        )
        .unwrap();
    for path in [
        "/../../private/direct".to_owned(),
        format!("/{}/../../private", opened.token),
        format!("/{}/%2e%2e%2fsecret", opened.token),
    ] {
        let reply = viewer.read(&mut db, &path, "GET", None);
        assert!(reply.is_err() || reply.unwrap().status == 404);
    }
    viewer.close(&mut db, &opened.token).unwrap();
    drop(viewer);
    fs::create_dir(source.join("移动后")).unwrap();
    fs::rename(source.join(&names[0]), source.join("移动后/改名 中文.mp4")).unwrap();
    let moved = db.scan(&source, &AtomicBool::new(false)).unwrap();
    assert_eq!((moved.moved, moved.renamed), (1, 1));
    let filter = LibraryFilter {
        search: "中文".into(),
        search_scope: SearchScope::Filename,
        ..Default::default()
    };
    assert_eq!(
        db.repositories()
            .library()
            .query(&filter, None, 100)
            .unwrap()
            .items
            .len(),
        1
    );
    let injection = LibraryFilter {
        search: "' OR 1=1 --".into(),
        search_scope: SearchScope::Filename,
        ..Default::default()
    };
    assert!(db
        .repositories()
        .library()
        .query(&injection, None, 100)
        .unwrap()
        .items
        .is_empty());
    drop(db);
    let mut db = Database::open(&paths).unwrap();
    let retained = db.repositories().media().get(&video.id).unwrap().unwrap();
    assert_eq!(
        (
            retained.imported_at,
            retained.like_count,
            retained.playback_position
        ),
        (video.imported_at, 1, 20.0)
    );
    assert_eq!(
        db.repositories()
            .custom_tags()
            .for_media(&video.id, None, 100)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        fs::read(source.join("移动后/改名 中文.mp4")).unwrap(),
        b"synthetic read-only media"
    );
    let viewer = ViewerService::default();
    let restored = viewer.open(&mut db, &video.id).unwrap();
    assert_eq!(restored.state.unwrap().position_seconds, 20.0);
    viewer.close(&mut db, &restored.token).unwrap();
    println!(
        "RELEASE_PATHS longest_filename_utf16={} source_length_utf16={}",
        names[1].encode_utf16().count(),
        source
            .join(&names[1])
            .to_string_lossy()
            .encode_utf16()
            .count()
    );
}

#[test]
#[ignore = "requires real FFmpeg/ffprobe; run with --include-ignored"]
fn unicode_long_path_tools_and_corrupt_thumbnail_cache_recover_without_changing_source() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("图片 café 中文");
    fs::create_dir(&source).unwrap();
    let original = include_bytes!("../../tests/ui/assets/0.jpg");
    let name = format!("中文-{}.jpg", "x".repeat(230));
    let file = source.join(&name);
    fs::write(&file, original).unwrap();
    let paths = AppPaths::prepare(&temp.path().join("app")).unwrap();
    let mut db = Database::open(&paths).unwrap();
    assert_eq!(db.scan(&source, &AtomicBool::new(false)).unwrap().new, 1);
    let id = db.repositories().media().list(None, 1).unwrap()[0]
        .id
        .clone();
    let report = db
        .process_derivatives(&Toolchain::default(), &AtomicBool::new(false))
        .unwrap();
    assert_eq!(
        report.ready,
        1,
        "long Unicode path failed: {:?}",
        db.request_thumbnail(&id, false).unwrap()
    );
    let thumbnail = db
        .request_thumbnail(&id, false)
        .unwrap()
        .thumbnail_path
        .unwrap();
    for broken in [
        vec![],
        b"corrupted cache".to_vec(),
        vec![0xff, 0xd8, 0xff, 0xd9],
    ] {
        fs::write(&thumbnail, broken).unwrap();
        assert_eq!(db.request_thumbnail(&id, false).unwrap().status, "pending");
        assert_eq!(
            db.process_derivatives(&Toolchain::default(), &AtomicBool::new(false))
                .unwrap()
                .ready,
            1
        );
    }
    assert_eq!(fs::read(&file).unwrap(), original);
    println!(
        "RELEASE_FFMPEG_PATH utf16_length={}",
        file.to_string_lossy().encode_utf16().count()
    );
}
