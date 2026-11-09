use media_manager::{
    db::Database,
    domain::activity::*,
    platform::paths::AppPaths,
    services::{activity as a, statistics as s},
};
use std::{
    cell::Cell,
    fs,
    sync::{atomic::AtomicBool, Arc, Barrier},
};

#[test]
fn hundred_thousand_statistics_publish_in_bounded_batches_with_live_likes() {
    let (_temp, paths, mut db, image, _) = fixture();
    let c = conn(&paths);
    c.execute_batch("BEGIN;
      WITH RECURSIVE n(v) AS(VALUES(1) UNION ALL SELECT v+1 FROM n WHERE v<100000)
      INSERT INTO media(id,file_path,file_name,media_type,file_size,modified_time,imported_at,last_seen_at,created_seq)
      SELECT printf('scale-%06d',v),printf('/synthetic/%d.png',v),printf('%d.png',v),'image',0,1,1,1,v+10 FROM n;
      WITH RECURSIVE n(v) AS(VALUES(1) UNION ALL SELECT v+1 FROM n WHERE v<2500)
      INSERT INTO like_events(id,ingest_seq,request_id,media_id,occurred_at,recorded_at)
      SELECT printf('scale-like-%d',v),200000+v,printf('scale-request-%d',v),'scale-100000',1,1 FROM n;
      UPDATE db_sequence SET value=300000 WHERE id=1;
      UPDATE media SET like_count=77;COMMIT;").unwrap();
    let started = std::time::Instant::now();
    s::rebuild_statistics(&mut db, "scale-rebuild", "all", None).unwrap();
    let mut injected = false;
    let mut steps = 0;
    loop {
        let before: String = c
            .query_row(
                "SELECT publish_cursor FROM statistics_jobs WHERE id='scale-rebuild'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let job = s::step(&mut db, "scale-rebuild").unwrap();
        steps += 1;
        let after: String = c
            .query_row(
                "SELECT publish_cursor FROM statistics_jobs WHERE id='scale-rebuild'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        if after != before {
            let batch: i64 = c
                .query_row(
                    "SELECT count(*) FROM media WHERE id>?1 AND id<=?2",
                    rusqlite::params![before, after],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(batch <= 100);
            if !injected {
                a::add_like(&mut db, &image, "published-live").unwrap();
                a::add_like(&mut db, "scale-100000", "unpublished-live").unwrap();
                injected = true;
            }
        }
        if job.state == "complete" {
            break;
        }
        assert!(steps < 2000);
    }
    assert!(steps >= 1000);
    assert!(injected);
    assert_eq!(scalar(&paths,"SELECT count(*) FROM media WHERE like_count != (SELECT count(*) FROM like_events WHERE media_id=media.id)"),0);
    assert_eq!(
        scalar(
            &paths,
            "SELECT like_count FROM media WHERE id='scale-100000'"
        ),
        2501
    );
    println!(
        "statistics scale: 100002 media, 2502 events, {steps} bounded steps, {:?}",
        started.elapsed()
    );
}

#[test]
fn failed_view_and_play_transactions_leave_work_state_and_caches_retryable() {
    let (_temp, paths, mut db, image, video) = fixture();
    let c = conn(&paths);
    c.execute_batch("CREATE TRIGGER reject_view BEFORE UPDATE OF view_count ON media BEGIN SELECT RAISE(ABORT,'injected view failure'); END;").unwrap();
    let view = activation("retry-view", "retry-viewer", 1, &image);
    assert!(a::record_view(&mut db, &view).is_err());
    assert_eq!(scalar(&paths, "SELECT count(*) FROM viewer_activations"), 0);
    assert_eq!(scalar(&paths, "SELECT count(*) FROM view_events"), 0);
    c.execute_batch("DROP TRIGGER reject_view").unwrap();
    a::record_view(&mut db, &view).unwrap();
    let (clock, id) = player(&mut db, &video, "retry-play");
    report(
        &mut db,
        &clock,
        &id,
        1,
        0.,
        Some(30.),
        PlaybackPhase::Playing,
        0,
        1.,
    );
    for seq in 2..=5 {
        report(
            &mut db,
            &clock,
            &id,
            seq,
            (seq - 1) as f64,
            Some(30.),
            PlaybackPhase::Playing,
            1000,
            1.,
        );
    }
    c.execute_batch("CREATE TRIGGER reject_play BEFORE UPDATE OF play_count ON media BEGIN SELECT RAISE(ABORT,'injected play failure'); END;").unwrap();
    clock.0.set(clock.0.get() + 1000);
    let evidence = PlaybackEvidence {
        session_id: id.clone(),
        sequence: 6,
        clock_ms: clock.0.get() as f64,
        position: 5.,
        duration: Some(30.),
        rate: 1.,
        phase: PlaybackPhase::Playing,
    };
    assert!(a::record_play_with_clock(&mut db, &evidence, &clock).is_err());
    assert_eq!(scalar(&paths, "SELECT count(*) FROM play_events"), 0);
    assert_eq!(scalar(&paths, "SELECT sequence FROM player_sessions"), 5);
    assert_eq!(scalar(&paths, "SELECT sum(play_count) FROM media"), 0);
    c.execute_batch("DROP TRIGGER reject_play").unwrap();
    assert!(
        a::record_play_with_clock(&mut db, &evidence, &clock)
            .unwrap()
            .qualified
    );
    assert_eq!(scalar(&paths, "SELECT count(*) FROM play_events"), 1);
}

#[test]
fn exactly_five_seconds_is_not_a_short_clip_and_unobserved_gaps_do_not_qualify() {
    let (_temp, paths, mut db, _, video) = fixture();
    let (clock, id) = player(&mut db, &video, "five-boundary");
    report(
        &mut db,
        &clock,
        &id,
        1,
        4.9,
        Some(5.),
        PlaybackPhase::Playing,
        0,
        1.,
    );
    assert!(
        !report(
            &mut db,
            &clock,
            &id,
            2,
            5.,
            Some(5.),
            PlaybackPhase::Ended,
            100,
            1.
        )
        .qualified
    );
    a::finalize_play(&mut db, &id).unwrap();
    let (clock, id) = player(&mut db, &video, "evidence-gap");
    report(
        &mut db,
        &clock,
        &id,
        1,
        0.,
        None,
        PlaybackPhase::Playing,
        0,
        1.,
    );
    assert!(
        !report(
            &mut db,
            &clock,
            &id,
            2,
            25.,
            None,
            PlaybackPhase::Playing,
            25000,
            1.
        )
        .qualified
    );
    a::finalize_play(&mut db, &id).unwrap();
    assert_eq!(scalar(&paths, "SELECT count(*) FROM play_events"), 0);
}
fn fixture() -> (tempfile::TempDir, AppPaths, Database, String, String) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("source");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("a.png"), b"synthetic").unwrap();
    fs::write(root.join("b.mp4"), b"synthetic video").unwrap();
    let paths = AppPaths::prepare(&temp.path().join("app")).unwrap();
    let mut db = Database::open(&paths).unwrap();
    db.scan(&root, &AtomicBool::new(false)).unwrap();
    let rows = db.repositories().media().list(None, 10).unwrap();
    let image = rows
        .iter()
        .find(|m| m.file_name == "a.png")
        .unwrap()
        .id
        .clone();
    let video = rows
        .iter()
        .find(|m| m.file_name == "b.mp4")
        .unwrap()
        .id
        .clone();
    (temp, paths, db, image, video)
}
fn activation(id: &str, viewer: &str, ordinal: i64, media: &str) -> Activation {
    Activation {
        id: id.into(),
        viewer_id: viewer.into(),
        ordinal,
        media_id: media.into(),
        content_version_id: 1,
        source: ViewSource::Grid,
    }
}
fn conn(paths: &AppPaths) -> rusqlite::Connection {
    rusqlite::Connection::open(&paths.database).unwrap()
}
fn scalar(paths: &AppPaths, sql: &str) -> i64 {
    conn(paths).query_row(sql, [], |r| r.get(0)).unwrap()
}
struct Clock(Cell<i64>);
impl a::ActivityClock for Clock {
    fn now_ms(&self) -> i64 {
        self.0.get()
    }
}
fn player(db: &mut Database, media: &str, id: &str) -> (Clock, String) {
    a::record_view(db, &activation(id, id, 1, media)).unwrap();
    let id = a::open_player_session(db, id).unwrap();
    let now = a::ActivityClock::now_ms(&a::SystemClock);
    (Clock(Cell::new(now)), id)
}
#[allow(clippy::too_many_arguments)] // Explicit test timeline samples keep boundary scenarios readable.
fn report(
    db: &mut Database,
    clock: &Clock,
    id: &str,
    seq: i64,
    position: f64,
    duration: Option<f64>,
    phase: PlaybackPhase,
    advance: i64,
    rate: f64,
) -> PlayResult {
    clock.0.set(clock.0.get() + advance);
    a::record_play_with_clock(
        db,
        &PlaybackEvidence {
            session_id: id.into(),
            sequence: seq,
            clock_ms: clock.0.get() as f64,
            position,
            duration,
            rate,
            phase,
        },
        clock,
    )
    .unwrap()
}
fn finish_job(db: &mut Database, id: &str) {
    for _ in 0..1000 {
        let job = s::step(db, id).unwrap();
        if matches!(job.state.as_str(), "complete" | "cancelled") {
            return;
        }
    }
    panic!("rebuild did not finish")
}
#[test]
fn hundred_rapid_concurrent_likes_are_atomic_and_retry_safe() {
    let (_temp, paths, mut db, image, _) = fixture();
    let barrier = Arc::new(Barrier::new(4));
    let mut threads = vec![];
    for worker in 0..4 {
        let mut connection = db.background_connection().unwrap();
        let id = image.clone();
        let barrier = barrier.clone();
        threads.push(std::thread::spawn(move || {
            barrier.wait();
            for n in 0..25 {
                a::add_like(&mut connection, &id, &format!("like-{worker}-{n}")).unwrap();
            }
        }));
    }
    for thread in threads {
        thread.join().unwrap();
    }
    let result = a::add_like(&mut db, &image, "like-0-0").unwrap();
    assert_eq!(result.like_count, 100);
    assert_eq!(scalar(&paths, "SELECT count(*) FROM like_events"), 100);
    assert!(a::add_like(&mut db, "wrong", "like-0-0").is_err());
}
#[test]
fn event_counter_sequence_and_rebuild_delta_rollback_together() {
    let (_temp, paths, mut db, image, _) = fixture();
    s::rebuild_statistics(&mut db, "rollback-job", "all", None).unwrap();
    let c = conn(&paths);
    c.execute_batch("CREATE TRIGGER fail_delta BEFORE INSERT ON statistics_delta BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    let before = scalar(&paths, "SELECT value FROM db_sequence");
    assert!(a::add_like(&mut db, &image, "rollback-like").is_err());
    assert_eq!(scalar(&paths, "SELECT count(*) FROM like_events"), 0);
    assert_eq!(scalar(&paths, "SELECT sum(like_count) FROM media"), 0);
    assert_eq!(scalar(&paths, "SELECT value FROM db_sequence"), before);
    assert_eq!(scalar(&paths, "SELECT count(*) FROM statistics_delta"), 0);
    c.execute_batch("DROP TRIGGER fail_delta;").unwrap();
    assert_eq!(
        a::add_like(&mut db, &image, "rollback-like")
            .unwrap()
            .like_count,
        1
    );
    finish_job(&mut db, "rollback-job");
    assert_eq!(scalar(&paths, "SELECT sum(like_count) FROM media"), 1);
    c.execute_batch("PRAGMA foreign_keys=ON;
      CREATE TABLE commit_probe(media_id TEXT REFERENCES media(id) DEFERRABLE INITIALLY DEFERRED);
      CREATE TRIGGER fail_commit AFTER INSERT ON like_events BEGIN INSERT INTO commit_probe VALUES('absent-media'); END;").unwrap();
    let before = scalar(&paths, "SELECT value FROM db_sequence");
    assert!(a::add_like(&mut db, &image, "commit-retry").is_err());
    assert_eq!(scalar(&paths, "SELECT count(*) FROM like_events"), 1);
    assert_eq!(scalar(&paths, "SELECT sum(like_count) FROM media"), 1);
    assert_eq!(scalar(&paths, "SELECT value FROM db_sequence"), before);
    c.execute_batch("DROP TRIGGER fail_commit;DROP TABLE commit_probe")
        .unwrap();
    assert_eq!(
        a::add_like(&mut db, &image, "commit-retry")
            .unwrap()
            .like_count,
        2
    );
}
#[test]
fn views_use_activation_identity_not_time_and_allow_a_b_a() {
    let (_temp, paths, mut db, image, video) = fixture();
    let first = activation("view-a1", "viewer", 1, &image);
    a::record_view(&mut db, &first).unwrap();
    for _ in 0..20 {
        a::record_view(&mut db, &first).unwrap();
    }
    a::record_view(&mut db, &activation("view-b", "viewer", 2, &video)).unwrap();
    let mut third = activation("view-a2", "viewer", 3, &image);
    third.source = ViewSource::History;
    a::record_view(&mut db, &third).unwrap();
    assert_eq!(a::record_view(&mut db, &third).unwrap().view_count, 2);
    assert_eq!(scalar(&paths, "SELECT count(*) FROM view_events"), 3);
    assert_eq!(
        scalar(
            &paths,
            "SELECT count(*) FROM view_events WHERE source='history' AND viewed_at=occurred_at"
        ),
        1
    );
    assert!(a::record_view(&mut db, &activation("stale", "viewer", 2, &image)).is_err());
    let mut conflict = first.clone();
    conflict.source = ViewSource::Search;
    assert!(a::record_view(&mut db, &conflict).is_err());
    assert_eq!(scalar(&paths, "SELECT count(*) FROM play_events"), 0);
}
#[test]
fn qualified_play_finalizes_once_without_recount_and_progress_is_independent() {
    let (_temp, paths, mut db, _, video) = fixture();
    let (clock, id) = player(&mut db, &video, "valid");
    assert_eq!(scalar(&paths, "SELECT count(*) FROM play_events"), 0);
    report(
        &mut db,
        &clock,
        &id,
        1,
        0.,
        Some(10.),
        PlaybackPhase::Playing,
        0,
        1.,
    );
    for seq in 2..=7 {
        report(
            &mut db,
            &clock,
            &id,
            seq,
            (seq - 1) as f64,
            Some(10.),
            PlaybackPhase::Playing,
            1000,
            1.,
        );
    }
    assert_eq!(scalar(&paths, "SELECT count(*) FROM play_events"), 1);
    report(
        &mut db,
        &clock,
        &id,
        8,
        9.6,
        Some(10.),
        PlaybackPhase::Seeking,
        0,
        1.,
    );
    a::finalize_play(&mut db, &id).unwrap();
    a::finalize_play(&mut db, &id).unwrap();
    assert_eq!(scalar(&paths, "SELECT sum(play_count) FROM media"), 1);
    assert_eq!(scalar(&paths, "SELECT completed FROM play_events"), 1);
    let watched: f64 = conn(&paths)
        .query_row("SELECT watched_seconds FROM play_events", [], |r| r.get(0))
        .unwrap();
    assert!((watched - 6.).abs() < 0.001);
    assert_eq!(scalar(&paths, "SELECT count(*) FROM playback_states"), 0);
}
#[test]
fn short_play_needs_real_ended_and_seek_or_restore_cannot_qualify() {
    let (_temp, paths, mut db, _, video) = fixture();
    for (name, seek) in [("short-invalid", false), ("seek-end", true)] {
        let (clock, id) = player(&mut db, &video, name);
        report(
            &mut db,
            &clock,
            &id,
            1,
            if seek { 2.9 } else { 0. },
            Some(3.),
            PlaybackPhase::Playing,
            0,
            1.,
        );
        if seek {
            report(
                &mut db,
                &clock,
                &id,
                2,
                3.,
                Some(3.),
                PlaybackPhase::Seeking,
                100,
                1.,
            );
            report(
                &mut db,
                &clock,
                &id,
                3,
                3.,
                Some(3.),
                PlaybackPhase::Ended,
                0,
                1.,
            );
        } else {
            report(
                &mut db,
                &clock,
                &id,
                2,
                1.,
                Some(3.),
                PlaybackPhase::Paused,
                1000,
                1.,
            );
        }
        a::finalize_play(&mut db, &id).unwrap();
    }
    assert_eq!(scalar(&paths, "SELECT count(*) FROM play_events"), 0);
    let (clock, id) = player(&mut db, &video, "short-valid");
    report(
        &mut db,
        &clock,
        &id,
        1,
        0.,
        Some(3.),
        PlaybackPhase::Playing,
        0,
        1.,
    );
    report(
        &mut db,
        &clock,
        &id,
        2,
        1.,
        Some(3.),
        PlaybackPhase::Playing,
        1000,
        1.,
    );
    report(
        &mut db,
        &clock,
        &id,
        3,
        2.,
        Some(3.),
        PlaybackPhase::Playing,
        1000,
        1.,
    );
    assert!(
        !report(
            &mut db,
            &clock,
            &id,
            4,
            3.,
            Some(3.),
            PlaybackPhase::Paused,
            1000,
            1.
        )
        .qualified
    );
    assert!(
        report(
            &mut db,
            &clock,
            &id,
            5,
            3.,
            Some(3.),
            PlaybackPhase::Ended,
            0,
            1.
        )
        .qualified
    );
    a::finalize_play(&mut db, &id).unwrap();
    assert_eq!(scalar(&paths, "SELECT completed FROM play_events"), 1);
    assert!(!a::completed(0.5, Some(3.)));
    assert!(!a::completed(100., None));
}
#[test]
fn speed_pause_buffer_seek_and_duplicate_evidence_do_not_invent_watch_time() {
    let (_temp, paths, mut db, _, video) = fixture();
    let (clock, id) = player(&mut db, &video, "speed");
    report(
        &mut db,
        &clock,
        &id,
        1,
        0.,
        Some(30.),
        PlaybackPhase::Playing,
        0,
        2.,
    );
    for seq in 2..=4 {
        report(
            &mut db,
            &clock,
            &id,
            seq,
            ((seq - 1) * 2) as f64,
            Some(30.),
            PlaybackPhase::Playing,
            1000,
            2.,
        );
    }
    assert_eq!(scalar(&paths, "SELECT count(*) FROM play_events"), 0);
    report(
        &mut db,
        &clock,
        &id,
        5,
        6.,
        Some(30.),
        PlaybackPhase::Waiting,
        1000,
        2.,
    );
    report(
        &mut db,
        &clock,
        &id,
        6,
        25.,
        Some(30.),
        PlaybackPhase::Seeking,
        10000,
        2.,
    );
    report(
        &mut db,
        &clock,
        &id,
        7,
        25.,
        Some(30.),
        PlaybackPhase::Playing,
        1000,
        2.,
    );
    report(
        &mut db,
        &clock,
        &id,
        8,
        27.,
        Some(30.),
        PlaybackPhase::Playing,
        1000,
        2.,
    );
    let result = report(
        &mut db,
        &clock,
        &id,
        9,
        29.,
        Some(30.),
        PlaybackPhase::Playing,
        1000,
        2.,
    );
    assert!(result.qualified);
    assert!((result.watched_seconds - 5.).abs() < 0.001);
    let duplicate = PlaybackEvidence {
        session_id: id.clone(),
        sequence: 9,
        clock_ms: clock.0.get() as f64,
        position: 29.,
        duration: Some(30.),
        rate: 2.,
        phase: PlaybackPhase::Playing,
    };
    assert!(
        a::record_play_with_clock(&mut db, &duplicate, &clock)
            .unwrap()
            .qualified
    );
    assert_eq!(scalar(&paths, "SELECT count(*) FROM play_events"), 1);
}
#[test]
fn statistics_rebuild_repairs_counts_timestamps_zeros_and_concurrent_events() {
    let (_temp, paths, mut db, image, video) = fixture();
    for n in 0..15 {
        a::add_like(&mut db, &image, &format!("base-{n}")).unwrap();
    }
    a::record_view(&mut db, &activation("rebuild-view", "rebuild", 1, &image)).unwrap();
    let (clock, id) = player(&mut db, &video, "rebuild-play");
    report(
        &mut db,
        &clock,
        &id,
        1,
        0.,
        None,
        PlaybackPhase::Playing,
        0,
        1.,
    );
    for seq in 2..=6 {
        report(
            &mut db,
            &clock,
            &id,
            seq,
            (seq - 1) as f64,
            None,
            PlaybackPhase::Playing,
            1000,
            1.,
        );
    }
    a::finalize_play(&mut db, &id).unwrap();
    conn(&paths).execute_batch("UPDATE media SET like_count=999,view_count=999,play_count=999,last_viewed_at=1,last_played_at=1").unwrap();
    s::rebuild_statistics(&mut db, "all-stats", "all", None).unwrap();
    s::step(&mut db, "all-stats").unwrap();
    a::add_like(&mut db, &image, "delta-like").unwrap();
    a::record_view(&mut db, &activation("delta-view", "delta", 1, &image)).unwrap();
    finish_job(&mut db, "all-stats");
    let mismatch=scalar(&paths,"SELECT count(*) FROM media m WHERE like_count!=(SELECT count(*) FROM like_events WHERE media_id=m.id) OR view_count!=(SELECT count(*) FROM view_events WHERE media_id=m.id) OR play_count!=(SELECT count(*) FROM play_events WHERE media_id=m.id) OR last_viewed_at IS NOT (SELECT max(occurred_at) FROM view_events WHERE media_id=m.id) OR last_played_at IS NOT (SELECT max(qualified_at) FROM play_events WHERE media_id=m.id)");
    assert_eq!(mismatch, 0);
    assert_eq!(scalar(&paths, "SELECT count(*) FROM statistics_base"), 0);
    assert_eq!(scalar(&paths, "SELECT count(*) FROM statistics_delta"), 0);
    for (kind, request) in [
        ("like", "likes-only"),
        ("view", "views-only"),
        ("play", "plays-only"),
    ] {
        s::rebuild_statistics(&mut db, request, kind, Some(&image)).unwrap();
        finish_job(&mut db, request);
    }
    assert_eq!(scalar(&paths, "SELECT count(*) FROM like_events"), 16);
}
#[test]
fn rebuild_resumes_after_restart_and_cancel_stops_delta_capture() {
    let (_temp, paths, mut db, image, _) = fixture();
    a::add_like(&mut db, &image, "resume-like").unwrap();
    s::rebuild_all_like_counts(&mut db, "resume-job").unwrap();
    s::step(&mut db, "resume-job").unwrap();
    drop(db);
    let mut db = Database::open(&paths).unwrap();
    finish_job(&mut db, "resume-job");
    s::rebuild_statistics(&mut db, "cancel-job", "all", None).unwrap();
    s::cancel(&mut db, "cancel-job").unwrap();
    a::add_like(&mut db, &image, "after-cancel").unwrap();
    finish_job(&mut db, "cancel-job");
    assert_eq!(s::status(&mut db, "cancel-job").unwrap().state, "cancelled");
    assert_eq!(scalar(&paths, "SELECT sum(like_count) FROM media"), 2);
}
#[test]
fn interrupted_sessions_finalize_only_last_durable_qualified_evidence() {
    let (_temp, paths, mut db, _, video) = fixture();
    let (clock, id) = player(&mut db, &video, "interrupted");
    report(
        &mut db,
        &clock,
        &id,
        1,
        0.,
        Some(30.),
        PlaybackPhase::Playing,
        0,
        1.,
    );
    for seq in 2..=6 {
        report(
            &mut db,
            &clock,
            &id,
            seq,
            (seq - 1) as f64,
            Some(30.),
            PlaybackPhase::Playing,
            1000,
            1.,
        );
    }
    a::recover_interrupted(&mut db).unwrap();
    assert_eq!(scalar(&paths,"SELECT count(*) FROM play_events WHERE ended_at IS NOT NULL AND end_reason='interrupted'"),1);
    let watched: f64 = conn(&paths)
        .query_row("SELECT watched_seconds FROM play_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(watched, 5.);
    assert_eq!(scalar(&paths, "SELECT sum(play_count) FROM media"), 1);
}
