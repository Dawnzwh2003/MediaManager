//! Disposable sources only; failure injection exercises the same copy/publish use case as the UI.
use media_manager::{
    db::Database,
    domain::{activity::*, importing::ImportJob, viewer::ProgressWrite},
    importing::{
        self as i,
        storage::{ImportStorage, LocalStorage},
    },
    platform::paths::AppPaths,
    services::activity as a,
    viewer::ViewerService,
};
use rusqlite::Connection;
use std::{
    cell::Cell,
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

fn fixture() -> (tempfile::TempDir, AppPaths, Database, PathBuf) {
    let t = tempfile::tempdir().unwrap();
    let p = AppPaths::prepare(&t.path().join("app")).unwrap();
    let db = Database::open(&p).unwrap();
    let s = t.path().join("source");
    fs::create_dir(&s).unwrap();
    (t, p, db, s)
}
fn write(root: &Path, rel: &str, bytes: &[u8]) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, bytes).unwrap();
}
fn preview(db: &mut Database, s: &Path) -> ImportJob {
    let j = i::create_preview(db, s).unwrap();
    let j = i::run_job(db, &j.id, &AtomicBool::new(false), &LocalStorage).unwrap();
    assert_eq!(j.state, "preview", "{j:?}");
    j
}
fn finish(db: &mut Database, j: &ImportJob) -> ImportJob {
    i::start_import(db, &j.id, false, &LocalStorage).unwrap();
    i::run_job(db, &j.id, &AtomicBool::new(false), &LocalStorage).unwrap()
}
fn import(db: &mut Database, s: &Path) -> ImportJob {
    let j = preview(db, s);
    finish(db, &j)
}
fn conn(p: &AppPaths) -> Connection {
    let c = Connection::open(&p.database).unwrap();
    c.pragma_update(None, "foreign_keys", true).unwrap();
    c
}
fn count(p: &AppPaths, sql: &str) -> i64 {
    conn(p).query_row(sql, [], |r| r.get(0)).unwrap()
}
fn no_events(p: &AppPaths) {
    for table in ["like_events", "view_events", "play_events"] {
        assert_eq!(count(p, &format!("SELECT count(*) FROM {table}")), 0);
    }
}

#[test]
fn recursive_image_video_unicode_and_long_names_persist_and_keep_physical_tags_separate() {
    let (_t, p, mut db, s) = fixture();
    write(
        &s,
        "动物/猫/照片 café 🐈.jpg",
        include_bytes!("../../tests/ui/assets/0.jpg"),
    );
    write(
        &s,
        "旅行/日本/movie.mp4",
        b"video fixture; decoder validation is a separate worker",
    );
    write(&s, &format!("{}.png", "a".repeat(240)), b"long name");
    write(&s, "ignore.txt", b"ignore");
    let deep = format!("{}x.jpg", "d/".repeat(24));
    write(&s, &deep, b"deep");
    let j = preview(&mut db, &s);
    assert_eq!((j.scanned_files, j.new_media, j.new_tags), (4, 4, 28));
    let j = finish(&mut db, &j);
    assert_eq!((j.newly_imported, j.failed_files), (4, 0));
    for m in db.repositories().media().list(None, 100).unwrap() {
        assert!(
            Path::new(&m.file_path).starts_with(fs::canonicalize(p.data.join("media")).unwrap())
        );
        assert!(Path::new(&m.file_path).is_file());
    }
    assert_eq!(
        count(&p, "SELECT count(*) FROM media WHERE media_type='video'"),
        1
    );
    assert_eq!(count(&p, "SELECT count(*) FROM media_import_sources"), 4);
    assert_eq!(count(&p,"SELECT count(*) FROM import_source_tags t JOIN import_source_tags parent ON parent.id=t.parent_id WHERE parent.source_id!=t.source_id"),0);
    assert_eq!(
        count(
            &p,
            "SELECT count(*) FROM system_tags WHERE name IN('动物','猫','旅行','日本')"
        ),
        0
    );
    no_events(&p);
    drop(db);
    let mut db = Database::open(&p).unwrap();
    assert_eq!(
        i::get_job(&mut db, None).unwrap().unwrap().newly_imported,
        4
    );
    assert_eq!(count(&p, "SELECT count(*) FROM media_import_tags"), 28);
}

#[test]
fn duplicate_matrix_repeated_import_and_multiple_source_paths_keep_one_instance_per_exact_name_content(
) {
    let (_t, p, mut db, s) = fixture();
    write(&s, "Animal/Cat/a.jpg", b"same");
    write(&s, "Travel/Japan/a.jpg", b"same");
    write(&s, "Other/a.jpg", b"DIFF");
    write(&s, "b.jpg", b"same");
    let j = preview(&mut db, &s);
    assert_eq!((j.new_media, j.duplicate_files, j.conflicts), (3, 1, 1));
    let j = finish(&mut db, &j);
    assert_eq!(
        (j.newly_imported, j.existing_media_reused, j.failed_files),
        (3, 1, 0)
    );
    assert_eq!(count(&p, "SELECT count(*) FROM media_import_sources"), 4);
    assert_eq!(
        count(
            &p,
            "SELECT max(n) FROM (SELECT count(*) n FROM media_import_tags GROUP BY media_id)"
        ),
        4
    );
    let before: Vec<(String, i64)> = conn(&p)
        .prepare("SELECT id,imported_at FROM media ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let j = preview(&mut db, &s);
    assert_eq!(
        (
            j.new_media,
            j.existing_media,
            j.estimated_storage_required,
            j.new_tags
        ),
        (0, 4, 0, 0)
    );
    let j = finish(&mut db, &j);
    assert_eq!(
        (j.newly_imported, j.existing_media_reused, j.tags_added),
        (0, 4, 0)
    );
    let after: Vec<(String, i64)> = conn(&p)
        .prepare("SELECT id,imported_at FROM media ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(before, after);
    no_events(&p);
    assert_eq!(count(&p, "SELECT count(*) FROM media_import_sources"), 4);
}

struct Clock(Cell<i64>);
impl a::ActivityClock for Clock {
    fn now_ms(&self) -> i64 {
        self.0.get()
    }
}
#[test]
fn legacy_duplicate_materializes_without_resetting_any_history_custom_tags_or_playback() {
    let (t, p, mut db, s) = fixture();
    let old = t.path().join("legacy");
    fs::create_dir(&old).unwrap();
    write(&old, "a.mp4", b"history video");
    db.scan(&old, &AtomicBool::new(false)).unwrap();
    let m = db.repositories().media().list(None, 10).unwrap().remove(0);
    a::add_like(&mut db, &m.id, "like").unwrap();
    a::record_view(
        &mut db,
        &Activation {
            id: "activation".into(),
            viewer_id: "viewer".into(),
            ordinal: 1,
            media_id: m.id.clone(),
            content_version_id: 1,
            source: ViewSource::Grid,
        },
    )
    .unwrap();
    let session = a::open_player_session(&mut db, "activation").unwrap();
    let clock = Clock(Cell::new(a::ActivityClock::now_ms(&a::SystemClock)));
    for seq in 1..=7 {
        if seq > 1 {
            clock.0.set(clock.0.get() + 1000);
        }
        a::record_play_with_clock(
            &mut db,
            &PlaybackEvidence {
                session_id: session.clone(),
                sequence: seq,
                clock_ms: clock.0.get() as f64,
                position: (seq - 1) as f64,
                duration: Some(10.),
                rate: 1.,
                phase: PlaybackPhase::Playing,
            },
            &clock,
        )
        .unwrap();
    }
    a::finalize_play(&mut db, &session).unwrap();
    db.repositories()
        .custom_tags()
        .create("custom", "Keep", 1)
        .unwrap();
    db.repositories()
        .custom_tags()
        .attach(&m.id, "custom")
        .unwrap();
    let viewer = ViewerService::default();
    let open = viewer.open(&mut db, &m.id).unwrap();
    viewer
        .save(
            &mut db,
            &ProgressWrite {
                token: open.token,
                sequence: 1,
                expected_version: open.state.unwrap().state_version,
                position_seconds: 4.,
                duration_seconds: Some(10.),
            },
        )
        .unwrap();
    let c = conn(&p);
    let old_state:String=c.query_row("SELECT json_object('p',position_seconds,'v',state_version,'u',updated_at) FROM playback_states",[],|r|r.get(0)).unwrap();
    write(&s, "Travel/Japan/a.mp4", b"history video");
    let j = import(&mut db, &s);
    assert_eq!(
        (j.newly_imported, j.existing_media_reused, j.failed_files),
        (0, 1, 0),
        "{j:?}"
    );
    let current = db.repositories().media().get(&m.id).unwrap().unwrap();
    assert_eq!(
        (current.like_count, current.view_count, current.play_count),
        (1, 1, 1)
    );
    assert_eq!(current.imported_at, m.imported_at);
    assert_eq!(current.content_version_id, m.content_version_id);
    assert_eq!(count(&p, "SELECT count(*) FROM media_custom_tags"), 1);
    let state:String=c.query_row("SELECT json_object('p',position_seconds,'v',state_version,'u',updated_at) FROM playback_states",[],|r|r.get(0)).unwrap();
    assert_eq!(state, old_state);
    assert_eq!(fs::read(old.join("a.mp4")).unwrap(), b"history video");
    assert!(current.file_path != m.file_path);
    let root = PathBuf::from(i::storage_settings(&mut db).unwrap().media_root);
    db.scan(&root, &AtomicBool::new(false)).unwrap();
    assert_eq!(db.repositories().media().list(None, 100).unwrap().len(), 1);
    assert_eq!(
        db.repositories().media().get(&m.id).unwrap().unwrap().state,
        "active"
    );
    assert_eq!(count(&p, "SELECT count(*) FROM like_events"), 1);
    assert_eq!(count(&p, "SELECT count(*) FROM view_events"), 1);
    assert_eq!(count(&p, "SELECT count(*) FROM play_events"), 1);
}

struct Fault {
    mode: &'static str,
    n: AtomicUsize,
}
impl ImportStorage for Fault {
    fn available(&self, p: &Path) -> io::Result<u64> {
        if self.mode == "space" {
            Ok(0)
        } else {
            LocalStorage.available(p)
        }
    }
    fn copy(&self, s: &Path, t: &Path, c: &AtomicBool) -> io::Result<String> {
        let n = self.n.fetch_add(1, Ordering::Relaxed);
        if n == 0 && self.mode == "fail" {
            fs::write(t, b"partial")?;
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "injected"));
        }
        let h = LocalStorage.copy(s, t, c)?;
        if n == 1 && self.mode == "cancel" {
            c.store(true, Ordering::Relaxed);
        }
        Ok(h)
    }
}
#[test]
fn cancellation_keeps_published_media_removes_only_part_and_resume_is_idempotent() {
    let (_t, p, mut db, s) = fixture();
    write(&s, "a.jpg", b"a");
    write(&s, "b.jpg", b"b");
    write(&s, "c.jpg", b"c");
    let j = preview(&mut db, &s);
    i::start_import(&mut db, &j.id, false, &LocalStorage).unwrap();
    let j = i::run_job(
        &mut db,
        &j.id,
        &AtomicBool::new(false),
        &Fault {
            mode: "cancel",
            n: AtomicUsize::new(0),
        },
    )
    .unwrap();
    assert_eq!((j.state.as_str(), j.newly_imported), ("cancelled", 1));
    assert_eq!(count(&p, "SELECT count(*) FROM media"), 1);
    i::start_import(&mut db, &j.id, true, &LocalStorage).unwrap();
    let j = i::run_job(&mut db, &j.id, &AtomicBool::new(false), &LocalStorage).unwrap();
    assert_eq!(
        (j.state.as_str(), j.newly_imported, j.failed_files),
        ("completed", 3, 0)
    );
    assert!(s.join("a.jpg").is_file());
    no_events(&p);
    for folder in fs::read_dir(p.data.join("media")).unwrap() {
        for f in fs::read_dir(folder.unwrap().path()).unwrap() {
            assert!(!f.unwrap().file_name().to_string_lossy().ends_with(".part"));
        }
    }
}
#[test]
fn copy_failure_is_isolated_and_retry_preserves_successful_rows() {
    let (_t, p, mut db, s) = fixture();
    write(&s, "a.jpg", b"a");
    write(&s, "b.jpg", b"b");
    let j = preview(&mut db, &s);
    i::start_import(&mut db, &j.id, false, &LocalStorage).unwrap();
    let j = i::run_job(
        &mut db,
        &j.id,
        &AtomicBool::new(false),
        &Fault {
            mode: "fail",
            n: AtomicUsize::new(0),
        },
    )
    .unwrap();
    assert_eq!((j.failed_files, j.newly_imported), (1, 1));
    i::start_import(&mut db, &j.id, true, &LocalStorage).unwrap();
    let j = i::run_job(&mut db, &j.id, &AtomicBool::new(false), &LocalStorage).unwrap();
    assert_eq!((j.failed_files, j.newly_imported), (0, 2));
    no_events(&p);
}
#[test]
fn insufficient_space_prevents_copy_and_does_not_create_media() {
    let (_t, p, mut db, s) = fixture();
    write(&s, "a.jpg", b"a");
    let j = preview(&mut db, &s);
    let result = i::start_import(
        &mut db,
        &j.id,
        false,
        &Fault {
            mode: "space",
            n: AtomicUsize::new(0),
        },
    );
    assert!(matches!(
        result,
        Err(media_manager::error::AppError::Import(
            "insufficient_storage"
        ))
    ));
    assert_eq!(count(&p, "SELECT count(*) FROM media"), 0);
    assert_eq!(
        i::get_job(&mut db, Some(&j.id)).unwrap().unwrap().state,
        "preview"
    );
    assert_eq!(fs::read(s.join("a.jpg")).unwrap(), b"a");
}
#[test]
fn unavailable_source_and_overlap_are_rejected_without_removing_files() {
    let (t, p, mut db, s) = fixture();
    let root = PathBuf::from(i::storage_settings(&mut db).unwrap().media_root);
    let sub = root.join("child");
    fs::create_dir(&sub).unwrap();
    for bad in [&root, &sub, t.path()] {
        assert!(i::create_preview(&mut db, bad).is_err());
    }
    write(&s, "a.jpg", b"a");
    let j = preview(&mut db, &s);
    let offline = s.with_file_name("offline");
    fs::rename(&s, &offline).unwrap();
    assert!(i::start_import(&mut db, &j.id, false, &LocalStorage).is_err());
    assert_eq!(count(&p, "SELECT count(*) FROM media"), 0);
    fs::rename(&offline, &s).unwrap();
    assert_eq!(finish(&mut db, &j).newly_imported, 1);
    assert!(i::set_storage(&mut db, &s).is_err());
}
#[test]
fn changed_source_after_preview_cannot_attach_wrong_provenance() {
    let (_t, p, mut db, s) = fixture();
    write(&s, "a.jpg", b"same");
    import(&mut db, &s);
    let j = preview(&mut db, &s);
    write(&s, "a.jpg", b"DIFF");
    let j = finish(&mut db, &j);
    assert_eq!((j.failed_files, j.existing_media_reused), (1, 0));
    assert_eq!(count(&p, "SELECT count(*) FROM media_import_sources"), 1);
    assert_eq!(count(&p, "SELECT count(*) FROM media"), 1);
    no_events(&p);
}
#[test]
fn matching_partial_fingerprints_never_prove_full_content_equality() {
    let (t, p, mut db, s) = fixture();
    let a = vec![0_u8; 8 * 1024 * 1024];
    write(&s, "a.jpg", &a);
    import(&mut db, &s);
    let other = t.path().join("other");
    let mut b = a;
    b[2 * 1024 * 1024] = 1;
    write(&other, "a.jpg", &b);
    let j = import(&mut db, &other);
    assert_eq!((j.newly_imported, j.existing_media_reused), (1, 0));
    assert_eq!(
        count(&p, "SELECT count(DISTINCT fingerprint) FROM media"),
        1
    );
    assert_eq!(
        count(
            &p,
            "SELECT count(DISTINCT full_hash) FROM media_full_hashes"
        ),
        2
    );
}
#[test]
fn database_failure_after_atomic_copy_rolls_back_all_links_and_recovers_reserved_target() {
    let (_t, p, mut db, s) = fixture();
    write(&s, "Folder/a.jpg", b"a");
    let j = preview(&mut db, &s);
    let c = conn(&p);
    c.execute_batch("CREATE TRIGGER reject_import BEFORE INSERT ON media_import_sources BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    let j = finish(&mut db, &j);
    assert_eq!((j.failed_files, j.newly_imported), (1, 0));
    for table in [
        "media",
        "managed_media",
        "media_full_hashes",
        "media_import_sources",
        "import_source_tags",
        "media_import_tags",
    ] {
        assert_eq!(count(&p, &format!("SELECT count(*) FROM {table}")), 0);
    }
    let reserved: (String, String) = c
        .query_row(
            "SELECT media_id,target_path FROM import_entries WHERE kind='file'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(Path::new(&reserved.1).is_file());
    let root = PathBuf::from(i::storage_settings(&mut db).unwrap().media_root);
    assert_eq!(
        db.scan(&root, &AtomicBool::new(false)).unwrap().new,
        0,
        "Scanner must not adopt an unpublished file reserved by the import journal"
    );
    c.execute_batch("DROP TRIGGER reject_import; UPDATE import_jobs SET state='interrupted';")
        .unwrap();
    drop(c);
    drop(db);
    let mut db = Database::open(&p).unwrap();
    i::start_import(&mut db, &j.id, true, &LocalStorage).unwrap();
    let j = i::run_job(&mut db, &j.id, &AtomicBool::new(false), &LocalStorage).unwrap();
    assert_eq!((j.newly_imported, j.failed_files), (1, 0));
    assert!(db
        .repositories()
        .media()
        .get(&reserved.0)
        .unwrap()
        .is_some());
    no_events(&p);
}
#[test]
fn storage_setting_only_changes_future_media_and_is_persisted_across_restart() {
    let (t, p, mut db, s) = fixture();
    write(&s, "a.jpg", b"a");
    import(&mut db, &s);
    let old = db.repositories().media().list(None, 10).unwrap().remove(0);
    let root = t.path().join("new-data");
    fs::create_dir(&root).unwrap();
    let selected = i::set_storage(&mut db, &root).unwrap().media_root;
    write(&s, "b.jpg", b"b");
    let j = import(&mut db, &s);
    assert_eq!((j.newly_imported, j.existing_media_reused), (1, 1));
    assert_eq!(
        db.repositories()
            .media()
            .get(&old.id)
            .unwrap()
            .unwrap()
            .file_path,
        old.file_path
    );
    drop(db);
    let mut db = Database::open(&p).unwrap();
    assert_eq!(i::storage_settings(&mut db).unwrap().media_root, selected);
    assert_eq!(count(&p, "SELECT count(*) FROM storage_roots"), 2);
}
#[test]
fn path_traversal_is_not_accepted_as_an_import_target() {
    let (_t, _p, _db, s) = fixture();
    for bad in ["../outside.jpg", "a/../../outside.jpg", "/absolute.jpg"] {
        assert!(i::storage::join(&s, bad).is_err());
    }
    assert!(i::storage::join(&s, "normal.jpg").is_ok());
}

#[test]
fn unfinished_preview_is_interrupted_on_worker_restart_and_can_resume() {
    let (_t, p, mut db, s) = fixture();
    write(&s, "a.jpg", b"a");
    let j = i::create_preview(&mut db, &s).unwrap();
    conn(&p)
        .execute("UPDATE import_jobs SET created_at=created_at+86400000", [])
        .unwrap();
    drop(db);
    let mut db = Database::open(&p).unwrap();
    let worker = i::spawn(&db).unwrap();
    for _ in 0..100 {
        if i::get_job(&mut db, Some(&j.id)).unwrap().unwrap().state == "interrupted" {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(
        i::get_job(&mut db, Some(&j.id)).unwrap().unwrap().state,
        "interrupted"
    );
    i::start_import(&mut db, &j.id, true, &LocalStorage).unwrap();
    for _ in 0..100 {
        if i::get_job(&mut db, Some(&j.id)).unwrap().unwrap().state == "preview" {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(
        i::get_job(&mut db, Some(&j.id)).unwrap().unwrap().state,
        "preview"
    );
    drop(worker);
    no_events(&p);
}

#[test]
fn hundred_thousand_media_source_filter_and_thousand_source_nodes_remain_paged_and_indexed() {
    use media_manager::domain::library_query::LibraryFilter;
    let (_t, p, mut db, s) = fixture();
    let j = import(&mut db, &s);
    assert_eq!(j.newly_imported, 0);
    let c = conn(&p);
    c.execute_batch("BEGIN;
 WITH RECURSIVE n(v) AS(VALUES(1) UNION ALL SELECT v+1 FROM n WHERE v<100000)
 INSERT INTO media(id,file_path,file_name,media_type,file_size,modified_time,imported_at,last_seen_at,created_seq) SELECT printf('scale-%06d',v),printf('/fixture/%d.jpg',v),printf('%d.jpg',v),'image',0,1,1,1,v FROM n;
 WITH RECURSIVE n(v) AS(VALUES(1) UNION ALL SELECT v+1 FROM n WHERE v<1000)
 INSERT INTO import_source_tags(id,source_id,parent_id,relative_dir,name) SELECT printf('%032x',v),s.id,NULL,printf('dir-%d',v),printf('dir-%d',v) FROM n CROSS JOIN import_sources s;
 INSERT INTO media_import_tags SELECT id,printf('%032x',1) FROM media WHERE created_seq%100=0;
 UPDATE db_sequence SET value=100000 WHERE id=1;COMMIT;").unwrap();
    let started = std::time::Instant::now();
    let filter = LibraryFilter {
        source_tag_id: Some(format!("{:032x}", 1)),
        ..Default::default()
    };
    let mut cursor = None;
    let mut seen = std::collections::HashSet::new();
    loop {
        let page = db
            .repositories()
            .library()
            .query(&filter, cursor.as_ref(), 200)
            .unwrap();
        assert!(page.items.len() <= 200);
        for item in page.items {
            assert!(seen.insert(item.id));
        }
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(seen.len(), 1000);
    let source = i::source_nodes(&mut db, None, None, "")
        .unwrap()
        .items
        .remove(0)
        .id;
    let mut after = String::new();
    let mut total = 0;
    loop {
        let page = i::source_nodes(&mut db, Some(&source), None, &after).unwrap();
        assert!(page.items.len() <= 100);
        total += page.items.len();
        if let Some(next) = page.next_after {
            after = next;
        } else {
            break;
        }
    }
    assert_eq!(total, 1000);
    let plan: String = c
        .query_row(
            "EXPLAIN QUERY PLAN SELECT id FROM import_source_tags WHERE parent_id='x'",
            [],
            |r| r.get(3),
        )
        .unwrap();
    assert!(plan.contains("import_tag_children"), "{plan}");
    println!(
        "IMPORT_SCALE 100000 media, 1000 matches + 1000 source nodes paged: {:?}",
        started.elapsed()
    );
    no_events(&p);
}

#[test]
#[ignore = "requires real FFmpeg/ffprobe; run with --include-ignored"]
fn real_video_import_generates_thumbnail_and_opens_with_independent_playback_state() {
    let (_t, p, mut db, s) = fixture();
    let tools = media_manager::metadata::Toolchain::default();
    let ffmpeg = std::env::var("MEDIA_MANAGER_FFMPEG_DIR")
        .map(|p| PathBuf::from(p).join("ffmpeg.exe"))
        .unwrap_or_else(|_| PathBuf::from("ffmpeg"));
    let target = s.join("正常视频.mp4");
    let output = std::process::Command::new(ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=160x90:r=10",
            "-t",
            "2",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(&target)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let before = fs::read(&target).unwrap();
    let j = import(&mut db, &s);
    assert_eq!((j.newly_imported, j.failed_files), (1, 0));
    assert_eq!(
        db.process_derivatives(&tools, &AtomicBool::new(false))
            .unwrap()
            .ready,
        1
    );
    let m = db.repositories().media().list(None, 10).unwrap().remove(0);
    assert!(m.duration.unwrap() > 1.);
    let viewer = ViewerService::default();
    let opened = viewer.open(&mut db, &m.id).unwrap();
    assert!(opened.state.is_some());
    viewer.close(&mut db, &opened.token).unwrap();
    assert_eq!(fs::read(target).unwrap(), before);
    no_events(&p);
}

#[test]
fn source_tag_filter_uses_descendants_and_old_quick_browse_filter_json_remains_readable() {
    use media_manager::domain::library_query::LibraryFilter;
    let (_t, p, mut db, s) = fixture();
    write(&s, "Animal/Cat/a.jpg", b"a");
    write(&s, "Travel/b.jpg", b"b");
    import(&mut db, &s);
    let id: String = conn(&p)
        .query_row(
            "SELECT id FROM import_source_tags WHERE relative_dir='Animal'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let filter = LibraryFilter {
        source_tag_id: Some(id),
        ..Default::default()
    };
    let page = db
        .repositories()
        .library()
        .query(&filter, None, 100)
        .unwrap();
    assert_eq!(page.items.len(), 1);
    let old: LibraryFilter = serde_json::from_str("{}").unwrap();
    assert!(old.source_tag_id.is_none());
    let roots = i::source_nodes(&mut db, None, None, "").unwrap();
    assert_eq!(roots.items.len(), 1);
    assert!(roots.items[0].root);
    assert_eq!(
        i::source_nodes(&mut db, Some(&roots.items[0].id), None, "")
            .unwrap()
            .items
            .len(),
        2
    );
}
