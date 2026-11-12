use media_manager::{
    db::Database,
    domain::{
        library::{MediaType, NewMedia},
        reports::*,
    },
    error::AppError,
    platform::paths::AppPaths,
    services::reports,
};
use rusqlite::{params, Connection};
use std::{collections::BTreeSet, time::Instant};

fn fixture() -> (tempfile::TempDir, AppPaths, Database, Connection) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::prepare(&temp.path().join("app")).unwrap();
    let mut db = Database::open(&paths).unwrap();
    for (index, id) in ["a", "b", "c", "d", "e", "f"].iter().enumerate() {
        db.repositories()
            .media()
            .insert(&NewMedia {
                id: (*id).into(),
                fingerprint: None,
                file_path: temp.path().join(id).to_string_lossy().into(),
                file_name: format!("{id}.mp4"),
                media_type: if *id == "f" {
                    MediaType::Image
                } else {
                    MediaType::Video
                },
                file_size: 0,
                created_time: None,
                modified_time: 1,
                imported_at: 100 + index as i64,
                last_seen_at: 1,
                width: None,
                height: None,
                duration: Some(100.0),
            })
            .unwrap();
    }
    let sql = Connection::open(&paths.database).unwrap();
    sql.pragma_update(None, "foreign_keys", true).unwrap();
    (temp, paths, db, sql)
}
fn event(sql: &Connection, kind: &str, id: &str, time: i64, seq: i64) {
    let event_id = format!("{kind}-{seq}");
    let statement = match kind {
        "view" => "INSERT INTO view_events(id,request_id,activation_id,ingest_seq,media_id,occurred_at,recorded_at) VALUES(?1,?1,?1,?2,?3,?4,?4)",
        "like" => "INSERT INTO like_events(id,request_id,ingest_seq,media_id,occurred_at,recorded_at) VALUES(?1,?1,?2,?3,?4,?4)",
        _ => "INSERT INTO play_events(id,request_id,player_session_id,ingest_seq,media_id,content_version_id,started_at,qualified_at,start_position,end_position,watched_seconds,completed) VALUES(?1,?1,?1,?2,?3,1,?4-5,?4,0,5,5,0)",
    };
    sql.execute(statement, params![event_id, seq, id, time])
        .unwrap();
}
fn query(kind: ReportKind, start: Option<i64>, end: i64) -> ReportQuery {
    ReportQuery {
        kind,
        window: TimeWindow { start, end },
    }
}
fn ids(page: &ReportPage) -> Vec<&str> {
    page.items.iter().map(|i| i.id.as_str()).collect()
}

#[test]
fn history_aggregates_each_range_uses_event_time_and_keeps_missing_identity() {
    let (_temp, _paths, mut db, sql) = fixture();
    for kind in ["view", "play", "like"] {
        for (n, (id, t)) in [("a", 99), ("a", 100), ("a", 100), ("b", 199), ("c", 200)]
            .iter()
            .enumerate()
        {
            event(&sql, kind, id, *t, n as i64 + 1);
        }
    }
    sql.execute_batch("UPDATE media SET state='missing',missing=1 WHERE id='b';")
        .unwrap();
    for kind in [
        ReportKind::MostViewed,
        ReportKind::MostPlayed,
        ReportKind::MostLiked,
    ] {
        let page = reports::query(&mut db, &query(kind, Some(100), 200), None, 100).unwrap();
        assert_eq!(ids(&page), ["a", "b"]);
        assert_eq!(page.items[0].count, 2);
        assert_eq!(page.items[1].state, "missing");
        let all = reports::query(&mut db, &query(kind, None, 201), None, 100).unwrap();
        assert_eq!(all.items[0].count, 3);
        assert!(
            reports::query(&mut db, &query(kind, Some(201), 300), None, 100)
                .unwrap()
                .items
                .is_empty()
        );
    }
    for kind in [ReportKind::RecentlyViewed, ReportKind::RecentlyPlayed] {
        let page = reports::query(&mut db, &query(kind, Some(100), 200), None, 100).unwrap();
        assert_eq!(ids(&page), ["b", "a"]);
        assert_eq!(page.items[0].last_at, 199);
    }
    let result = reports::dashboard(
        &mut db,
        TimeWindow {
            start: Some(100),
            end: 200,
        },
    )
    .unwrap();
    assert_eq!(
        (
            result.total_media,
            result.images,
            result.videos,
            result.total_likes,
            result.views_today
        ),
        (6, 1, 5, 4, 3)
    );
    assert_eq!(result.recently_added.len(), 5); // active only; missing retained in totals
    let before: i64 = sql
        .query_row("SELECT count(*) FROM view_events", [], |r| r.get(0))
        .unwrap();
    reports::dashboard(
        &mut db,
        TimeWindow {
            start: Some(100),
            end: 200,
        },
    )
    .unwrap();
    assert_eq!(
        sql.query_row("SELECT count(*) FROM view_events", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        before
    );
}

#[test]
fn continue_watching_derives_completion_and_isolates_versions_without_play_events() {
    let (_temp, _paths, mut db, sql) = fixture();
    sql.execute_batch("INSERT INTO playback_states(media_id,content_version_id,position_seconds,duration_seconds,completed,updated_at) VALUES
      ('a',1,50,100,0,100),('b',1,95,100,0,101),('c',1,1,2,0,102),
      ('d',1,1.9,2,0,103),('e',1,10,NULL,0,104),('f',1,20,100,0,105);
      UPDATE media SET duration=NULL WHERE id='e';").unwrap();
    let q = query(ReportKind::ContinueWatching, None, 1000);
    let page = reports::query(&mut db, &q, None, 100).unwrap();
    assert_eq!(ids(&page), ["e", "c", "a"]);
    assert_eq!(page.items[0].duration, None);
    // A persisted completed bit from an older client cannot override current facts.
    sql.execute_batch("UPDATE playback_states SET completed=1 WHERE media_id='a';")
        .unwrap();
    assert_eq!(
        ids(&reports::query(&mut db, &q, None, 100).unwrap()),
        ["e", "c", "a"]
    );
    // Restore/seek alone can complete recovery state, without a qualified event.
    sql.execute_batch("UPDATE playback_states SET position_seconds=95 WHERE media_id='a';")
        .unwrap();
    assert_eq!(
        ids(&reports::query(&mut db, &q, None, 100).unwrap()),
        ["e", "c"]
    );
    sql.execute_batch("BEGIN; INSERT INTO media_revisions(media_id,content_version_id,fingerprint,file_size,created_at) VALUES('c',2,NULL,0,1); UPDATE media SET current_content_version_id=2 WHERE id='c'; COMMIT;").unwrap();
    assert_eq!(ids(&reports::query(&mut db, &q, None, 100).unwrap()), ["e"]);
    sql.execute_batch("UPDATE media SET current_content_version_id=1 WHERE id='c'; UPDATE media SET state='missing',missing=1 WHERE id='e';").unwrap();
    assert_eq!(ids(&reports::query(&mut db, &q, None, 100).unwrap()), ["c"]);
    sql.execute_batch(
        "BEGIN; UPDATE playback_states SET position_seconds=2 WHERE media_id='c'; ROLLBACK;",
    )
    .unwrap();
    assert_eq!(ids(&reports::query(&mut db, &q, None, 100).unwrap()), ["c"]);
    assert_eq!(
        sql.query_row("SELECT count(*) FROM play_events", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn reports_reject_changed_queries_stale_pages_and_invalid_inputs_then_recover() {
    let (_temp, _paths, mut db, sql) = fixture();
    for (n, id) in ["a", "b", "c"].iter().enumerate() {
        event(&sql, "like", id, 100, n as i64 + 1);
    }
    let q = query(ReportKind::MostLiked, None, 200);
    let first = reports::query(&mut db, &q, None, 1).unwrap();
    let cursor = first.next_cursor.unwrap();
    assert_eq!(
        ids(&reports::query(&mut db, &q, Some(&cursor), 2).unwrap()),
        ["b", "a"]
    );
    assert!(matches!(
        reports::query(
            &mut db,
            &query(ReportKind::MostViewed, None, 200),
            Some(&cursor),
            2
        ),
        Err(AppError::StaleCursor)
    ));
    event(&sql, "like", "a", 101, 4);
    assert!(matches!(
        reports::query(&mut db, &q, Some(&cursor), 2),
        Err(AppError::StaleCursor)
    ));
    assert_eq!(ids(&reports::query(&mut db, &q, None, 1).unwrap()), ["a"]);
    for limit in [0, 201] {
        assert!(reports::query(&mut db, &q, None, limit).is_err());
    }
    for (start, end) in [(Some(-1), 10), (Some(10), 10), (None, 0)] {
        assert!(
            reports::query(&mut db, &query(ReportKind::MostLiked, start, end), None, 10).is_err()
        );
    }
    sql.execute_batch("INSERT INTO playback_states(media_id,content_version_id,position_seconds,duration_seconds,completed,updated_at) SELECT id,1,10,100,0,100 FROM media WHERE media_type='video';").unwrap();
    let q = query(ReportKind::ContinueWatching, None, 200);
    let cursor = reports::query(&mut db, &q, None, 1)
        .unwrap()
        .next_cursor
        .unwrap();
    sql.execute_batch("UPDATE playback_states SET position_seconds=20 WHERE media_id='a';")
        .unwrap();
    assert!(matches!(
        reports::query(&mut db, &q, Some(&cursor), 2),
        Err(AppError::StaleCursor)
    ));
    assert!(reports::dashboard(
        &mut db,
        TimeWindow {
            start: None,
            end: 200
        }
    )
    .is_err());
}

#[test]
fn empty_dashboard_and_failure_retry_do_not_fabricate_activity() {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::prepare(&temp.path().join("app")).unwrap();
    let mut db = Database::open(&paths).unwrap();
    let result = reports::dashboard(
        &mut db,
        TimeWindow {
            start: Some(1),
            end: 200,
        },
    )
    .unwrap();
    assert_eq!(
        (
            result.total_media,
            result.images,
            result.videos,
            result.total_likes,
            result.views_today
        ),
        (0, 0, 0, 0, 0)
    );
    assert!(result.continue_watching.is_empty());
    let sql = Connection::open(&paths.database).unwrap();
    sql.execute_batch("ALTER TABLE report_generation RENAME TO hidden_generation;")
        .unwrap();
    let q = query(ReportKind::MostViewed, None, 200);
    assert!(reports::query(&mut db, &q, None, 100).is_err());
    sql.execute_batch("ALTER TABLE hidden_generation RENAME TO report_generation;")
        .unwrap();
    assert!(reports::query(&mut db, &q, None, 100)
        .unwrap()
        .items
        .is_empty());
}

#[test]
fn continue_watching_save_waits_for_a_short_activity_writer_and_remains_retry_safe() {
    let (_temp, paths, mut db, sql) = fixture();
    let (owner, state) = db.repositories().playback().claim("a", 1, 100).unwrap();
    let (send, receive) = std::sync::mpsc::channel();
    let path = paths.database.clone();
    let writer = std::thread::spawn(move || {
        let connection = Connection::open(path).unwrap();
        connection
            .execute_batch("BEGIN IMMEDIATE; UPDATE db_sequence SET value=value+1 WHERE id=1;")
            .unwrap();
        send.send(()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(150));
        connection.execute_batch("COMMIT;").unwrap();
    });
    receive.recv().unwrap();
    let request = media_manager::domain::viewer::ProgressWrite {
        token: owner,
        sequence: 1,
        expected_version: state.state_version,
        position_seconds: 96.0,
        duration_seconds: Some(100.0),
    };
    let saved = db.repositories().playback().save("a", 1, &request, 101);
    writer.join().unwrap();
    assert!(saved.is_ok(),"progress must wait for a short writer, rather than upgrading a stale deferred snapshot: {saved:?}");
    let saved = saved.unwrap();
    let retry = db
        .repositories()
        .playback()
        .save("a", 1, &request, 102)
        .unwrap();
    assert_eq!(saved.state_version, retry.state_version);
    assert!(reports::query(
        &mut db,
        &query(ReportKind::ContinueWatching, None, 200),
        None,
        100
    )
    .unwrap()
    .items
    .is_empty());
    assert_eq!(
        sql.query_row("SELECT count(*) FROM play_events", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn hundred_thousand_media_event_aggregation_keyset_and_dashboard_are_measured() {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::prepare(&temp.path().join("app")).unwrap();
    let mut db = Database::open(&paths).unwrap();
    let sql = Connection::open(&paths.database).unwrap();
    sql.execute_batch("PRAGMA foreign_keys=ON; BEGIN;
      WITH RECURSIVE n(v) AS(VALUES(1) UNION ALL SELECT v+1 FROM n WHERE v<100000)
      INSERT INTO media(id,file_path,file_name,media_type,file_size,modified_time,imported_at,last_seen_at,created_seq)
      SELECT printf('m%06d',v),printf('/synthetic/%d.mp4',v),printf('%d.mp4',v),'video',0,1,v,1,v FROM n;
      INSERT INTO view_events(id,request_id,activation_id,ingest_seq,media_id,occurred_at,recorded_at)
      SELECT id,id,id,created_seq,id,imported_at,imported_at FROM media;
      INSERT INTO playback_states(media_id,content_version_id,position_seconds,duration_seconds,completed,updated_at)
      SELECT id,1,10,100,0,imported_at FROM media; COMMIT;").unwrap();
    let mut seen = BTreeSet::new();
    let mut cursor = None;
    let mut times = Vec::new();
    let q = query(ReportKind::MostViewed, None, 200000);
    loop {
        let start = Instant::now();
        let page = reports::query(&mut db, &q, cursor.as_ref(), 200).unwrap();
        times.push(start.elapsed().as_secs_f64() * 1000.0);
        for item in &page.items {
            assert_eq!(item.count, 1);
            assert!(seen.insert(item.id.clone()));
        }
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(seen.len(), 100000);
    assert_eq!(times.len(), 500);
    let last = *times.last().unwrap();
    times.sort_by(f64::total_cmp);
    let start = Instant::now();
    let dashboard = reports::dashboard(
        &mut db,
        TimeWindow {
            start: Some(1),
            end: 200000,
        },
    )
    .unwrap();
    let dashboard_ms = start.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(
        (dashboard.total_media, dashboard.views_today),
        (100000, 100000)
    );
    assert_eq!(dashboard.continue_watching.len(), 6);
    let start = Instant::now();
    let watching = reports::query(
        &mut db,
        &query(ReportKind::ContinueWatching, None, 200000),
        None,
        200,
    )
    .unwrap();
    assert_eq!(watching.items.len(), 200);
    eprintln!("REPORT SCALE 100000 media/events: 500 pages p50={:.3}ms p95={:.3}ms last={last:.3}ms dashboard={dashboard_ms:.3}ms continue={:.3}ms",times[249],times[474],start.elapsed().as_secs_f64()*1000.0);
    for (table, time, index) in [
        ("view_events", "occurred_at", "history_views_time"),
        ("play_events", "qualified_at", "history_plays_time"),
        ("like_events", "occurred_at", "history_likes_time"),
    ] {
        let plan:Vec<String>=sql.prepare(&format!("EXPLAIN QUERY PLAN SELECT media_id,count(*),max({time}) FROM {table} WHERE {time}>=10 AND {time}<200 GROUP BY media_id")).unwrap().query_map([],|r|r.get(3)).unwrap().collect::<Result<_,_>>().unwrap();
        assert!(
            plan.iter()
                .any(|p| p.contains("SEARCH") && p.contains(index)),
            "{plan:?}"
        );
    }
}
