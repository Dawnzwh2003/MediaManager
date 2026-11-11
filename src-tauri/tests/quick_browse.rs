use media_manager::{
    db::Database,
    domain::{
        activity::{Activation, ViewSource},
        library::{MediaType, NewMedia},
        library_query::*,
        quick_browse::*,
    },
    platform::paths::AppPaths,
    services::{activity, quick_browse as q, tag_toggle},
    viewer::ViewerService,
};
use std::{
    fs,
    time::{Duration, Instant},
};

fn fixture() -> (tempfile::TempDir, AppPaths, Database) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::prepare(&temp.path().join("app")).unwrap();
    let mut db = Database::open(&paths).unwrap();
    for (n, id) in ["a", "b", "c", "d"].iter().enumerate() {
        let path = temp.path().join(format!("{id}.png"));
        fs::write(&path, b"synthetic").unwrap();
        db.repositories()
            .media()
            .insert(&NewMedia {
                id: (*id).into(),
                fingerprint: None,
                file_path: path.to_str().unwrap().into(),
                file_name: format!("{id}.png"),
                media_type: MediaType::Image,
                file_size: 9,
                created_time: Some(n as i64),
                modified_time: 1,
                imported_at: n as i64,
                last_seen_at: 1,
                width: None,
                height: None,
                duration: None,
            })
            .unwrap();
    }
    (temp, paths, db)
}
fn request(id: &str, filter: LibraryFilter) -> QuickBrowseRequest {
    QuickBrowseRequest {
        request_id: id.into(),
        filter,
        autoplay: false,
        muted: true,
    }
}
fn build(db: &mut Database, id: &str, filter: LibraryFilter) -> BrowseSession {
    q::create(db, &request(id, filter)).unwrap();
    q::build_session(db, id, &|| false, q::BuildLimits::default()).unwrap()
}
fn ids(db: &mut Database, id: &str) -> Vec<String> {
    q::items(db, id, -1, 200)
        .unwrap()
        .into_iter()
        .map(|i| i.media_id)
        .collect()
}
fn sql(paths: &AppPaths) -> rusqlite::Connection {
    let c = rusqlite::Connection::open(&paths.database).unwrap();
    c.pragma_update(None, "foreign_keys", true).unwrap();
    c
}
fn scalar(c: &rusqlite::Connection, query: &str) -> i64 {
    c.query_row(query, [], |r| r.get(0)).unwrap()
}

#[test]
fn all_query_sorts_and_filters_are_frozen_against_activity_tags_and_scans() {
    let (_temp, paths, mut db) = fixture();
    let c = sql(&paths);
    c.execute_batch("UPDATE media SET like_count=imported_at,view_count=3-imported_at,play_count=imported_at%2,last_viewed_at=imported_at,last_played_at=3-imported_at;").unwrap();
    for (n, sort) in [
        LibrarySort::Recent,
        LibrarySort::ImportedAsc,
        LibrarySort::Name,
        LibrarySort::NameDesc,
        LibrarySort::Likes,
        LibrarySort::LikesAsc,
        LibrarySort::Views,
        LibrarySort::ViewsAsc,
        LibrarySort::Plays,
        LibrarySort::PlaysAsc,
        LibrarySort::Created,
        LibrarySort::CreatedAsc,
        LibrarySort::RecentlyViewed,
        LibrarySort::RecentlyPlayed,
        LibrarySort::Random,
    ]
    .into_iter()
    .enumerate()
    {
        let filter = LibraryFilter {
            sort,
            random: Some(RandomOrder {
                seed: "0123456789abcdef0123456789abcdef".into(),
                version: 1,
            }),
            ..Default::default()
        };
        let expected = db
            .repositories()
            .library()
            .query(&filter, None, 100)
            .unwrap()
            .items
            .into_iter()
            .map(|m| m.id)
            .collect::<Vec<_>>();
        let id = format!("sort-{n}");
        let session = build(&mut db, &id, filter);
        assert_eq!(ids(&mut db, &id), expected);
        assert_eq!(session.item_count, 4);
        assert_eq!(session.current_ordinal, Some(0));
        q::close(&mut db, &id).unwrap();
    }
    db.repositories()
        .custom_tags()
        .create("tag", "Keep", 1)
        .unwrap();
    for id in ["a", "b"] {
        db.repositories().custom_tags().attach(id, "tag").unwrap();
    }
    let filter = LibraryFilter {
        search: ".png".into(),
        search_scope: SearchScope::Filename,
        media_type: Some("image".into()),
        custom_tag_ids: vec!["tag".into()],
        viewed: ViewedStatus::Unviewed,
        sort: LibrarySort::Likes,
        ..Default::default()
    };
    c.execute_batch("UPDATE media SET view_count=0;").unwrap();
    let session = build(&mut db, "fixed", filter.clone());
    let original = ids(&mut db, "fixed");
    assert_eq!(original, vec!["b", "a"]);
    activity::add_like(&mut db, "a", "quick-like").unwrap();
    db.repositories().custom_tags().detach("b", "tag").unwrap();
    c.execute_batch("UPDATE media SET view_count=100 WHERE id='a'; INSERT INTO media_custom_tags VALUES('c','tag');").unwrap();
    assert_eq!(ids(&mut db, "fixed"), original);
    assert_eq!(
        serde_json::to_value(q::get(&mut db, "fixed").unwrap().filter).unwrap(),
        serde_json::to_value(filter).unwrap()
    );
    assert_eq!(
        q::get(&mut db, "fixed").unwrap().started_at,
        session.started_at
    );
    assert!(q::items(&mut db, "fixed", -1, 201).is_err());
    assert!(c
        .execute(
            "UPDATE quick_browse_sessions SET seed='different' WHERE id='fixed'",
            []
        )
        .is_err());
}

#[test]
fn reshuffle_is_explicit_idempotent_and_atomically_ends_previous_session() {
    let (_temp, paths, mut db) = fixture();
    let c = sql(&paths);
    let original = build(&mut db, "first", LibraryFilter::default());
    let order = ids(&mut db, "first");
    assert_eq!(
        q::create(&mut db, &request("first", LibraryFilter::default()))
            .unwrap()
            .id,
        original.id
    );
    let mut conflict = request("first", LibraryFilter::default());
    conflict.autoplay = true;
    assert!(q::create(&mut db, &conflict).is_err());
    let next = q::reshuffle(&mut db, "first", "second").unwrap();
    assert_eq!(
        q::reshuffle(&mut db, "first", "second")
            .unwrap()
            .random_seed,
        next.random_seed
    );
    assert_eq!(ids(&mut db, "first"), order);
    assert_eq!(q::get(&mut db, "first").unwrap().state, "ready");
    let ready = q::build_session(&mut db, "second", &|| false, q::BuildLimits::default()).unwrap();
    assert_eq!(ready.sort_mode, "random");
    assert_eq!(ready.random_seed.as_ref().unwrap().len(), 32);
    let ended = q::get(&mut db, "first").unwrap();
    assert_eq!(ended.state, "closed");
    assert!(ended.ended_at.unwrap() >= ended.started_at);
    let stable = ids(&mut db, "second");
    assert_eq!(ids(&mut db, "second"), stable);
    let third = q::reshuffle(&mut db, "second", "third").unwrap();
    assert_ne!(third.random_seed, ready.random_seed);
    q::close(&mut db, "third").unwrap();
    assert!(q::build_session(&mut db, "third", &|| false, q::BuildLimits::default()).is_err());
    while !q::cleanup_step(&mut db, "third").unwrap() {}
    assert_eq!(q::get(&mut db, "second").unwrap().state, "ready");
    q::close(&mut db, "second").unwrap();
    let end = q::get(&mut db, "second").unwrap().ended_at;
    q::close(&mut db, "second").unwrap();
    assert_eq!(q::get(&mut db, "second").unwrap().ended_at, end);
    assert_eq!(
        scalar(
            &c,
            "SELECT count(*) FROM quick_browse_session_items WHERE session_id='first'"
        ),
        4
    );
}

#[test]
fn number_bindings_toggle_with_retry_receipts_and_atomic_rollback() {
    let (_temp, paths, mut db) = fixture();
    let c = sql(&paths);
    db.repositories()
        .custom_tags()
        .create("tag", "Keep", 1)
        .unwrap();
    assert!(q::settings(&mut db).unwrap().muted);
    let prefs = BrowsePreferences {
        autoplay: true,
        muted: false,
        bindings: vec![TagBinding {
            digit: 1,
            tag_id: "tag".into(),
        }],
    };
    q::save_settings(&mut db, &prefs, None).unwrap();
    build(&mut db, "toggle", LibraryFilter::default());
    let req = ToggleTagRequest {
        request_id: "toggle-one".into(),
        session_id: "toggle".into(),
        ordinal: 0,
        digit: 1,
    };
    let first = tag_toggle::toggle_for_quick_browse(&mut db, &req).unwrap();
    assert!(first.attached);
    assert_eq!(first.media_id, "d");
    assert!(
        tag_toggle::toggle_for_quick_browse(&mut db, &req)
            .unwrap()
            .attached
    );
    let second = ToggleTagRequest {
        request_id: "toggle-two".into(),
        ..req.clone()
    };
    assert!(
        !tag_toggle::toggle_for_quick_browse(&mut db, &second)
            .unwrap()
            .attached
    );
    c.execute_batch("CREATE TRIGGER reject_receipt BEFORE INSERT ON quick_tag_operations BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    let third = ToggleTagRequest {
        request_id: "toggle-three".into(),
        ..req.clone()
    };
    assert!(tag_toggle::toggle_for_quick_browse(&mut db, &third).is_err());
    assert_eq!(scalar(&c, "SELECT count(*) FROM media_custom_tags"), 0);
    assert_eq!(scalar(&c, "SELECT count(*) FROM quick_tag_operations"), 2);
    c.execute_batch("DROP TRIGGER reject_receipt;").unwrap();
    assert!(
        tag_toggle::toggle_for_quick_browse(&mut db, &third)
            .unwrap()
            .attached
    );
    q::select(&mut db, "toggle", 1).unwrap();
    assert!(tag_toggle::toggle_for_quick_browse(
        &mut db,
        &ToggleTagRequest {
            request_id: "stale-item".into(),
            ..req.clone()
        }
    )
    .is_err());
    assert!(tag_toggle::toggle_for_quick_browse(
        &mut db,
        &ToggleTagRequest {
            digit: 2,
            ..req.clone()
        }
    )
    .is_err());
    assert!(
        tag_toggle::toggle_for_quick_browse(&mut db, &req)
            .unwrap()
            .attached
    ); // old ack may be retried after navigation
    assert!(q::save_settings(
        &mut db,
        &BrowsePreferences {
            bindings: vec![TagBinding {
                digit: 0,
                tag_id: "tag".into()
            }],
            ..prefs.clone()
        },
        None
    )
    .is_err());
    assert!(q::save_settings(
        &mut db,
        &BrowsePreferences {
            bindings: vec![TagBinding {
                digit: 1,
                tag_id: "missing".into()
            }],
            ..prefs
        },
        Some("toggle")
    )
    .is_err());
    assert!(q::settings(&mut db).unwrap().autoplay);
    db.repositories().custom_tags().delete("tag").unwrap();
    assert!(q::settings(&mut db).unwrap().bindings.is_empty());
    assert_eq!(scalar(&c, "SELECT count(*) FROM like_events"), 0);
    for media in db.repositories().media().list(None, 100).unwrap() {
        assert_eq!(fs::read(media.file_path).unwrap(), b"synthetic");
    }
}

#[test]
fn missing_and_changed_content_keep_slots_and_quick_views_use_shared_services() {
    let (temp, paths, mut db) = fixture();
    let c = sql(&paths);
    build(&mut db, "views", LibraryFilter::default());
    for (ordinal, media) in [(1, "d"), (2, "c"), (3, "d")] {
        let activation = Activation {
            id: format!("view-{ordinal}"),
            viewer_id: "views".into(),
            ordinal,
            media_id: media.into(),
            content_version_id: 1,
            source: ViewSource::QuickBrowser,
        };
        activity::record_view(&mut db, &activation).unwrap();
        activity::record_view(&mut db, &activation).unwrap();
    }
    activity::add_like(&mut db, "d", "same-service").unwrap();
    assert_eq!(
        scalar(
            &c,
            "SELECT count(*) FROM view_events WHERE source='quick_browser'"
        ),
        3
    );
    assert_eq!(scalar(&c, "SELECT view_count FROM media WHERE id='d'"), 2);
    assert_eq!(scalar(&c, "SELECT like_count FROM media WHERE id='d'"), 1);
    db.repositories().media().set_missing("c", true).unwrap();
    c.execute_batch("INSERT INTO media_revisions VALUES('d',2,'changed',9,2); UPDATE media SET current_content_version_id=2 WHERE id='d';").unwrap();
    let items = q::items(&mut db, "views", -1, 100).unwrap();
    assert_eq!(items.len(), 4);
    assert!(!items[0].available);
    assert!(!items[1].available);
    assert_eq!(items[2].ordinal, 2);
    assert!(ViewerService::default()
        .open_version(&mut db, "d", Some(1))
        .is_err());
    // A damaged original is isolated by the viewer; navigation is metadata-only.
    fs::remove_file(temp.path().join("b.png")).unwrap();
    assert!(ViewerService::default().open(&mut db, "b").is_err());
    assert_eq!(ids(&mut db, "views").len(), 4);
}

#[test]
fn cancelled_timeout_quota_and_cache_failure_are_cleaned_and_retryable() {
    let (_temp, paths, mut db) = fixture();
    for (id, limits, cancel) in [
        ("cancel", q::BuildLimits::default(), true),
        (
            "timeout",
            q::BuildLimits {
                read_timeout: Duration::ZERO,
                ..q::BuildLimits::default()
            },
            false,
        ),
        (
            "quota",
            q::BuildLimits {
                spool_bytes: 4096,
                ..q::BuildLimits::default()
            },
            false,
        ),
    ] {
        q::create(&mut db, &request(id, LibraryFilter::default())).unwrap();
        assert!(q::build_session(&mut db, id, &|| cancel, limits).is_err());
        assert_eq!(q::get(&mut db, id).unwrap().phase, "cleaning");
        while !q::cleanup_step(&mut db, id).unwrap() {}
        assert_eq!(q::get(&mut db, id).unwrap().state, "closed");
    }
    let browse = paths.cache.join("browse");
    fs::remove_dir(&browse).unwrap();
    fs::write(&browse, b"blocked").unwrap();
    q::create(&mut db, &request("bad-cache", LibraryFilter::default())).unwrap();
    assert!(q::build_session(&mut db, "bad-cache", &|| false, q::BuildLimits::default()).is_err());
    assert!(q::cleanup_step(&mut db, "bad-cache").is_err());
    assert_eq!(q::get(&mut db, "bad-cache").unwrap().state, "closed");
    fs::remove_file(&browse).unwrap();
    assert_eq!(
        build(&mut db, "recovered", LibraryFilter::default()).item_count,
        4
    );
    assert_eq!(fs::read_dir(browse).unwrap().count(), 0);
}

#[test]
fn background_worker_recovers_interrupted_builds_and_processes_fresh_requests() {
    let (_temp, paths, mut db) = fixture();
    q::create(&mut db, &request("interrupted", LibraryFilter::default())).unwrap();
    let worker = q::spawn(&db).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if q::get(&mut db, "interrupted").unwrap().state == "closed" {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        q::get(&mut db, "interrupted").unwrap().error.as_deref(),
        Some("snapshot_interrupted")
    );
    q::create(&mut db, &request("background", LibraryFilter::default())).unwrap();
    loop {
        if q::get(&mut db, "background").unwrap().state == "ready" {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(ids(&mut db, "background").len(), 4);
    drop(worker);
    assert_eq!(
        sql(&paths)
            .query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
}

#[test]
fn empty_sessions_and_partial_publication_cancel_preserve_visibility_boundaries() {
    let (_temp, paths, mut db) = fixture();
    let empty = build(
        &mut db,
        "empty",
        LibraryFilter {
            search: "no-such-file".into(),
            ..Default::default()
        },
    );
    assert_eq!(empty.item_count, 0);
    assert_eq!(empty.current_ordinal, None);
    assert!(q::items(&mut db, "empty", -1, 1).unwrap().is_empty());
    q::close(&mut db, "empty").unwrap();
    let c = sql(&paths);
    c.execute_batch("WITH RECURSIVE n(v) AS(VALUES(1) UNION ALL SELECT v+1 FROM n WHERE v<2500) INSERT INTO media(id,file_path,file_name,media_type,file_size,modified_time,imported_at,last_seen_at,created_seq) SELECT printf('partial-%d',v),printf('/synthetic/partial-%d.png',v),printf('%d.png',v),'image',0,1,v,1,v+100 FROM n;").unwrap();
    q::create(&mut db, &request("partial", LibraryFilter::default())).unwrap();
    assert!(q::items(&mut db, "partial", -1, 100).is_err());
    let observer = std::sync::Mutex::new(sql(&paths));
    let cancelled = || {
        observer
            .lock()
            .unwrap()
            .query_row(
                "SELECT published_count>=2000 FROM quick_browse_sessions WHERE id='partial'",
                [],
                |r| r.get::<_, bool>(0),
            )
            .unwrap()
    };
    assert!(q::build_session(&mut db, "partial", &cancelled, q::BuildLimits::default()).is_err());
    assert_eq!(
        scalar(
            &c,
            "SELECT count(*) FROM quick_browse_session_items WHERE session_id='partial'"
        ),
        2000
    );
    assert!(q::items(&mut db, "partial", -1, 100).is_err());
    assert!(!q::cleanup_step(&mut db, "partial").unwrap());
    assert_eq!(
        scalar(
            &c,
            "SELECT count(*) FROM quick_browse_session_items WHERE session_id='partial'"
        ),
        1000
    );
    assert!(q::cleanup_step(&mut db, "partial").unwrap());
    assert_eq!(q::get(&mut db, "partial").unwrap().state, "closed");
}

#[test]
fn hundred_thousand_snapshot_streams_with_live_writes_and_bounded_publication_and_pages() {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::prepare(&temp.path().join("app")).unwrap();
    let mut db = Database::open(&paths).unwrap();
    let c = sql(&paths);
    c.execute_batch("WITH RECURSIVE n(v) AS(VALUES(1) UNION ALL SELECT v+1 FROM n WHERE v<100000) INSERT INTO media(id,file_path,file_name,media_type,file_size,modified_time,imported_at,last_seen_at,created_seq) SELECT printf('m%06d',v),printf('/synthetic/%d.png',v),printf('%d.png',v),'image',0,1,v,1,v FROM n; UPDATE db_sequence SET value=100001 WHERE id=1;
      CREATE TABLE validation_batches(delta INTEGER); CREATE TRIGGER validation_batch AFTER UPDATE OF published_count ON quick_browse_sessions WHEN NEW.published_count>OLD.published_count BEGIN INSERT INTO validation_batches VALUES(NEW.published_count-OLD.published_count); END;").unwrap();
    let mut writer = db.background_connection().unwrap();
    let mut count = 0;
    let start = Instant::now();
    db.repositories().library().visit_browse_snapshot(&LibraryFilter::default(),&||false,|item| {
        count+=1;assert_eq!(item.ordinal,count-1);
        if count==1 {activity::add_like(&mut writer,"m100000","live").unwrap();c.execute_batch("INSERT INTO media(id,file_path,file_name,media_type,file_size,modified_time,imported_at,last_seen_at,created_seq) VALUES('later','/synthetic/later.png','later.png','image',0,1,200000,1,100002);").unwrap();}
        assert_ne!(item.media_id,"later");Ok(())
    }).unwrap();
    assert_eq!(count, 100000);
    let snapshot_ms = start.elapsed().as_secs_f64() * 1000.;
    let filter = LibraryFilter {
        sort: LibrarySort::Random,
        random: Some(RandomOrder {
            seed: "123456789abcdef0123456789abcdef0".into(),
            version: 1,
        }),
        ..Default::default()
    };
    let start = Instant::now();
    let session = build(&mut db, "scale", filter);
    let build_ms = start.elapsed().as_secs_f64() * 1000.;
    assert_eq!(session.item_count, 100001);
    assert_eq!(
        scalar(&c, "SELECT max(delta) FROM validation_batches"),
        1000
    );
    assert_eq!(scalar(&c, "SELECT count(*) FROM validation_batches"), 101);
    let mut after = -1;
    let mut count = 0;
    let mut timings = Vec::new();
    let mut seen = std::collections::HashSet::new();
    loop {
        let start = Instant::now();
        let page = q::items(&mut db, "scale", after, 200).unwrap();
        timings.push(start.elapsed().as_secs_f64() * 1000.);
        if page.is_empty() {
            break;
        }
        assert!(page.len() <= 200);
        for item in page {
            assert_eq!(item.ordinal, count);
            assert!(seen.insert(item.media_id));
            count += 1;
            after = item.ordinal;
        }
    }
    assert_eq!(count, 100001);
    timings.sort_by(f64::total_cmp);
    let plan=c.prepare("EXPLAIN QUERY PLAN SELECT media_id FROM quick_browse_session_items WHERE session_id='scale' AND ordinal>90000 ORDER BY ordinal LIMIT 200").unwrap().query_map([],|r|r.get::<_,String>(3)).unwrap().collect::<Result<Vec<_>,_>>().unwrap();
    assert!(
        plan.iter()
            .any(|s| s.contains("SEARCH") && s.contains("ordinal>")),
        "{plan:?}"
    );
    assert_eq!(fs::read_dir(paths.cache.join("browse")).unwrap().count(), 0);
    println!("QUICK SCALE: 100001 members, source read + concurrent Like/insert={snapshot_ms:.3}ms, random build={build_ms:.3}ms, 101 batches <=1000, page p50={:.3}ms p95={:.3}ms, session bytes={}",timings[timings.len()/2],timings[timings.len()*95/100],fs::metadata(paths.database).unwrap().len());
}
