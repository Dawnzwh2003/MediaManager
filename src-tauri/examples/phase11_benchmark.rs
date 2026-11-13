//! Repeatable synthetic catalog benchmarks. Never points at the user's library.
use media_manager::{
    db::Database,
    domain::{library_query::*, reports::*},
    platform::paths::AppPaths,
    services::{app_status::AppState, reports, statistics},
};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::{collections::BTreeSet, fs, io::Write, path::Path, time::Instant};

fn elapsed(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}
fn distribution(mut values: Vec<f64>) -> Value {
    values.sort_by(f64::total_cmp);
    json!({"samples":values.len(),"p50Ms":values[values.len()/2],"p95Ms":values[(values.len()*95/100).min(values.len()-1)],"maxMs":values.last()})
}
fn scalar(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}

fn seed(paths: &AppPaths, size: i64) -> Result<Value, Box<dyn std::error::Error>> {
    let started = Instant::now();
    let db = Database::open(paths)?;
    let c = Connection::open(&paths.database)?;
    c.execute_batch("PRAGMA foreign_keys=ON; BEGIN IMMEDIATE;
      INSERT INTO system_tags(id,parent_id,source_root,directory_path,name,last_seen_at) VALUES('root',NULL,'/phase11','/phase11','Collection',1);
      WITH RECURSIVE n(v) AS(VALUES(0) UNION ALL SELECT v+1 FROM n WHERE v<9)
      INSERT INTO system_tags(id,parent_id,source_root,directory_path,name,last_seen_at) SELECT 'dir-'||v,'root','/phase11','/phase11/folder-'||v,'Folder '||v,1 FROM n;
      INSERT INTO custom_tags(id,name,created_at,updated_at) VALUES('a','Travel',1,1),('b','Ideas',1,1),('c','Rare',1,1);")?;
    c.execute("WITH RECURSIVE n(v) AS(VALUES(1) UNION ALL SELECT v+1 FROM n WHERE v<?1)
      INSERT INTO media(id,file_path,file_name,media_type,file_size,modified_time,imported_at,last_seen_at,created_seq,duration)
      SELECT printf('m%06d',v),printf('/phase11/folder-%d/%06d.jpg',v%10,v),printf('%s%06d.jpg',CASE WHEN v%997=0 THEN 'rare-' ELSE 'media-' END,v),CASE WHEN v%5=0 THEN 'video' ELSE 'image' END,1,1,v,1,v,CASE WHEN v%5=0 THEN 100.0 ELSE NULL END FROM n",[size])?;
    c.execute_batch("INSERT INTO media_system_tags SELECT id,'dir-'||(created_seq%10) FROM media;
      INSERT INTO media_custom_tags SELECT id,'a' FROM media WHERE created_seq%2=0;
      INSERT INTO media_custom_tags SELECT id,'b' FROM media WHERE created_seq%5=0;
      INSERT INTO media_custom_tags SELECT id,'c' FROM media WHERE created_seq%997=0;
      INSERT INTO view_events(id,request_id,activation_id,ingest_seq,media_id,occurred_at,recorded_at) SELECT 'view-'||id,'view-'||id,'view-'||id,created_seq+1000000,id,1000000+created_seq,1000000+created_seq FROM media;
      INSERT INTO like_events(id,request_id,ingest_seq,media_id,occurred_at,recorded_at) SELECT 'like-'||id,'like-'||id,created_seq+2000000,id,1000000+created_seq,1000000+created_seq FROM media WHERE created_seq%7=0;
      INSERT INTO play_events(id,request_id,player_session_id,ingest_seq,media_id,content_version_id,started_at,qualified_at,start_position,end_position,watched_seconds,completed)
      SELECT 'play-'||id,'play-'||id,'play-'||id,created_seq+3000000,id,1,1000000+created_seq-5000,1000000+created_seq,0,5,5,0 FROM media WHERE media_type='video';
      INSERT INTO playback_states(media_id,content_version_id,position_seconds,duration_seconds,completed,updated_at) SELECT id,1,20,100,0,1000000+created_seq FROM media WHERE media_type='video';
      INSERT INTO scan_roots(path,generation,state) VALUES('/phase11',1,'complete');
      INSERT INTO scan_evidence SELECT id,'/phase11',id,'synthetic',1,'fixture' FROM media;
      INSERT INTO scan_observations SELECT '/phase11',file_path,id,'synthetic','synthetic',1,1,1,media_type FROM media;
      INSERT INTO scan_queue(root,path,kind) SELECT '/phase11',file_path,'file' FROM media;
      UPDATE media_derivatives SET status='failed',thumbnail_error='synthetic_fixture';
      UPDATE db_sequence SET value=4000000 WHERE id=1; COMMIT; PRAGMA wal_checkpoint(TRUNCATE);")?;
    assert_eq!(scalar(&c, "SELECT count(*) FROM media"), size);
    drop(c);
    drop(db);
    Ok(
        json!({"seedMs":elapsed(started),"media":size,"databaseBytes":fs::metadata(&paths.database)?.len()}),
    )
}

fn catalog(paths: &AppPaths, size: i64) -> Result<Value, Box<dyn std::error::Error>> {
    let start = Instant::now();
    let mut db = Database::open(paths)?;
    let open_ms = elapsed(start);
    let mut results = serde_json::Map::new();
    let base = LibraryFilter::default();
    for (name, filter) in [
        ("recent", base.clone()),
        (
            "random",
            LibraryFilter {
                sort: LibrarySort::Random,
                random: Some(RandomOrder {
                    seed: "0".repeat(32),
                    version: 1,
                }),
                ..base.clone()
            },
        ),
        (
            "filenameRare",
            LibraryFilter {
                search: "rare-".into(),
                search_scope: SearchScope::Filename,
                ..base.clone()
            },
        ),
        (
            "filenameNoMatch",
            LibraryFilter {
                search: "not-in-the-synthetic-library".into(),
                search_scope: SearchScope::Filename,
                ..base.clone()
            },
        ),
        (
            "customAnd",
            LibraryFilter {
                custom_tag_ids: vec!["a".into(), "b".into()],
                ..base.clone()
            },
        ),
        (
            "customOr",
            LibraryFilter {
                custom_tag_ids: vec!["a".into(), "b".into()],
                custom_tag_mode: TagMode::Or,
                ..base.clone()
            },
        ),
        (
            "systemDescendants",
            LibraryFilter {
                system_tag_id: Some("root".into()),
                ..base.clone()
            },
        ),
        (
            "systemLeaf",
            LibraryFilter {
                system_tag_id: Some("dir-3".into()),
                ..base.clone()
            },
        ),
        (
            "tagSearch",
            LibraryFilter {
                search: "Rare".into(),
                search_scope: SearchScope::CustomTag,
                ..base.clone()
            },
        ),
        (
            "combined",
            LibraryFilter {
                search: "media-".into(),
                custom_tag_ids: vec!["a".into(), "b".into()],
                media_type: Some("video".into()),
                ..base.clone()
            },
        ),
    ] {
        let mut cursor = None;
        let mut times = Vec::new();
        let mut ids = BTreeSet::new();
        let mut payload = 0;
        for _ in 0..30 {
            let start = Instant::now();
            let page = db
                .repositories()
                .library()
                .query(&filter, cursor.as_ref(), 100)?;
            times.push(elapsed(start));
            assert!(page.items.len() <= 100);
            payload = payload.max(serde_json::to_vec(&page)?.len());
            for item in page.items {
                assert!(ids.insert(item.id));
            }
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        let first = times[0];
        let count = ids.len();
        if name == "filenameNoMatch" {
            assert_eq!(count, 0)
        }
        if name == "filenameRare" || name == "tagSearch" {
            assert_eq!(count as i64, size / 997)
        }
        results.insert(name.into(),json!({"firstMs":first,"pages":distribution(times),"returnedMedia":count,"maxPayloadBytes":payload}));
    }
    // Use the real query signature/generation with a far-down key to exercise a deep seek.
    let first = db.repositories().library().query(&base, None, 100)?;
    let mut cursor = first.next_cursor.unwrap();
    cursor.id = "m000500".into();
    cursor.key = "500".into();
    let start = Instant::now();
    let deep = db
        .repositories()
        .library()
        .query(&base, Some(&cursor), 100)?;
    let deep_ms = elapsed(start);
    assert_eq!(deep.items[0].id, "m000499");
    Ok(json!({"openMs":open_ms,"deepRecentMs":deep_ms,"queries":results}))
}

fn history(paths: &AppPaths) -> Result<Value, Box<dyn std::error::Error>> {
    let mut db = Database::open(paths)?;
    let mut output = serde_json::Map::new();
    for kind in [
        ReportKind::RecentlyViewed,
        ReportKind::RecentlyPlayed,
        ReportKind::MostViewed,
        ReportKind::MostPlayed,
        ReportKind::MostLiked,
        ReportKind::ContinueWatching,
    ] {
        let mut times = vec![];
        let mut cursor = None;
        for _ in 0..5 {
            let start = Instant::now();
            let page = reports::query(
                &mut db,
                &ReportQuery {
                    kind,
                    window: TimeWindow {
                        start: None,
                        end: 50000000,
                    },
                },
                cursor.as_ref(),
                100,
            )?;
            times.push(elapsed(start));
            assert!(page.items.len() <= 100);
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        output.insert(
            serde_json::to_value(kind)?.as_str().unwrap().to_owned(),
            distribution(times),
        );
    }
    let start = Instant::now();
    let dashboard = reports::dashboard(
        &mut db,
        TimeWindow {
            start: Some(1000000),
            end: 50000000,
        },
    )?;
    output.insert("dashboardMs".into(), json!(elapsed(start)));
    let sql = Connection::open(&paths.database)?;
    assert_eq!(
        dashboard.views_today,
        scalar(&sql, "SELECT count(*) FROM view_events")
    );
    output.insert("events".into(),json!({"views":dashboard.views_today,"likes":dashboard.total_likes,"plays":scalar(&sql,"SELECT count(*) FROM play_events")}));
    Ok(Value::Object(output))
}

fn grow(paths: &AppPaths, size: i64) -> Result<Value, Box<dyn std::error::Error>> {
    let sql = Connection::open(&paths.database)?;
    sql.pragma_update(None, "foreign_keys", true)?;
    if scalar(&sql, "SELECT count(*) FROM view_events") != size {
        return Err("Growth fixture already exists".into());
    }
    let start = Instant::now();
    for from in (1..=size * 9).step_by(1000) {
        sql.execute_batch("BEGIN IMMEDIATE;")?;
        sql.execute("WITH RECURSIVE n(v) AS(VALUES(?1) UNION ALL SELECT v+1 FROM n WHERE v<?2)
          INSERT INTO view_events(id,request_id,activation_id,ingest_seq,media_id,occurred_at,recorded_at)
          SELECT 'growth-'||v,'growth-'||v,'growth-'||v,4000000+v,printf('m%06d',(v%?3)+1),1000000+v,1000000+v FROM n",params![from,(from+999).min(size*9),size])?;
        sql.execute_batch("COMMIT;")?;
    }
    sql.execute(
        "UPDATE db_sequence SET value=?1 WHERE id=1",
        [5000000 + size * 10],
    )?;
    let wal = paths.database.with_extension("sqlite3-wal");
    let before = fs::metadata(&wal).map(|m| m.len()).unwrap_or(0);
    sql.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
    assert_eq!(scalar(&sql, "SELECT count(*) FROM view_events"), size * 10);
    Ok(
        json!({"growthMs":elapsed(start),"viewEvents":size*10,"databaseBytes":fs::metadata(&paths.database)?.len(),"walBeforeCheckpointBytes":before,"walAfterCheckpointBytes":fs::metadata(wal).map(|m|m.len()).unwrap_or(0)}),
    )
}

fn rebuild(paths: &AppPaths) -> Result<Value, Box<dyn std::error::Error>> {
    let mut db = Database::open(paths)?;
    let sql = Connection::open(&paths.database)?;
    if scalar(
        &sql,
        "SELECT count(*) FROM statistics_jobs WHERE id='phase11-rebuild'",
    ) != 0
    {
        return Err("Rebuild fixture already used".into());
    }
    sql.execute_batch("UPDATE media SET like_count=77,view_count=77,play_count=77,last_viewed_at=0,last_played_at=0;")?;
    let mut job = statistics::rebuild_statistics(&mut db, "phase11-rebuild", "all", None)?;
    if job.state != "running" {
        return Err("Rebuild fixture already used".into());
    }
    let started = Instant::now();
    let mut times = vec![];
    let mut foreground = vec![];
    let mut steps = 0;
    let mut writer = db.background_connection()?;
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stopping = stop.clone();
    let concurrent = std::thread::spawn(move || {
        let mut timings = vec![];
        let mut n = 0;
        while !stopping.load(std::sync::atomic::Ordering::Relaxed) {
            let started = Instant::now();
            media_manager::services::activity::add_like(
                &mut writer,
                "m000001",
                &format!("concurrent-{n}"),
            )
            .unwrap();
            timings.push(elapsed(started));
            n += 1;
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        timings
    });
    while job.state == "running" || job.state == "cleaning" {
        let start = Instant::now();
        job = statistics::step(&mut db, &job.id)?;
        times.push(elapsed(start));
        steps += 1;
        if steps % 50 == 0 {
            let start = Instant::now();
            media_manager::services::activity::add_like(
                &mut db,
                "m000001",
                &format!("live-{steps}"),
            )?;
            foreground.push(elapsed(start));
        }
        std::thread::yield_now();
    }
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let concurrent = concurrent.join().unwrap();
    assert!(!concurrent.is_empty());
    assert_eq!(job.state, "complete");
    assert_eq!(scalar(&sql,"SELECT count(*) FROM media m WHERE m.view_count!=(SELECT count(*) FROM view_events e WHERE e.media_id=m.id) OR m.like_count!=(SELECT count(*) FROM like_events e WHERE e.media_id=m.id) OR m.play_count!=(SELECT count(*) FROM play_events e WHERE e.media_id=m.id) OR m.last_viewed_at IS NOT (SELECT max(occurred_at) FROM view_events WHERE media_id=m.id) OR m.last_played_at IS NOT (SELECT max(qualified_at) FROM play_events WHERE media_id=m.id)"),0);
    Ok(
        json!({"totalMs":elapsed(started),"steps":steps,"step":distribution(times),"foregroundBetweenBatches":distribution(foreground),"concurrentForeground":distribution(concurrent)}),
    )
}

fn startup(paths: &AppPaths) -> Result<Value, Box<dyn std::error::Error>> {
    let before = fs::metadata(&paths.database)?.len();
    let start = Instant::now();
    let state = AppState::new(paths.data.parent().unwrap().to_owned());
    let status = state.status()?;
    let ready = elapsed(start);
    assert_eq!(status.database.schema_version, 11);
    let start = Instant::now();
    for _ in 0..20 {
        state.status()?;
    }
    let warm = elapsed(start) / 20.0;
    drop(state);
    Ok(
        json!({"readyMs":ready,"warmStatusMeanMs":warm,"databaseBytesBefore":before,"databaseBytesAfter":fs::metadata(&paths.database)?.len()}),
    )
}

fn worker_rebuild(paths: &AppPaths) -> Result<Value, Box<dyn std::error::Error>> {
    let mut db = Database::open(paths)?;
    let sql = Connection::open(&paths.database)?;
    if scalar(
        &sql,
        "SELECT count(*) FROM statistics_jobs WHERE id='phase11-worker'",
    ) != 0
    {
        return Err("Worker fixture already used".into());
    }
    sql.execute_batch("UPDATE media SET like_count=77,view_count=77,play_count=77,last_viewed_at=0,last_played_at=0;")?;
    let mut job = statistics::rebuild_statistics(&mut db, "phase11-worker", "all", None)?;
    let worker = statistics::spawn(&db)?;
    let started = Instant::now();
    let mut timings = vec![];
    let mut n = 0;
    while job.state == "running" || job.state == "cleaning" {
        let start = Instant::now();
        media_manager::services::activity::add_like(
            &mut db,
            "m000001",
            &format!("worker-live-{n}"),
        )?;
        timings.push(elapsed(start));
        n += 1;
        std::thread::sleep(std::time::Duration::from_millis(5));
        job = statistics::status(&mut db, &job.id)?;
    }
    let total = elapsed(started);
    drop(worker);
    assert_eq!(job.state, "complete");
    assert_eq!(scalar(&sql,"SELECT count(*) FROM media m WHERE m.view_count!=(SELECT count(*) FROM view_events e WHERE e.media_id=m.id) OR m.like_count!=(SELECT count(*) FROM like_events e WHERE e.media_id=m.id) OR m.play_count!=(SELECT count(*) FROM play_events e WHERE e.media_id=m.id) OR m.last_viewed_at IS NOT (SELECT max(occurred_at) FROM view_events WHERE media_id=m.id) OR m.last_played_at IS NOT (SELECT max(qualified_at) FROM play_events WHERE media_id=m.id)"),0);
    Ok(json!({"workerMs":total,"foreground":distribution(timings),"counterDifferences":0}))
}

fn fingerprint(root: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    let mut result = vec![];
    for mib in [1, 4, 64] {
        let path = root.join(format!("fingerprint-{mib}.bin"));
        if !path.exists() {
            let mut f = fs::File::create(&path)?;
            let block = vec![71_u8; 1024 * 1024];
            for _ in 0..mib {
                f.write_all(&block)?;
            }
        }
        let mut times = vec![];
        let mut digest = None;
        for _ in 0..10 {
            let start = Instant::now();
            let value = media_manager::scanner::fingerprint::quick(
                &path,
                mib * 1024 * 1024,
                &std::sync::atomic::AtomicBool::new(false),
            )?;
            times.push(elapsed(start));
            if let Some(previous) = &digest {
                assert_eq!(previous, &value)
            }
            digest = Some(value);
        }
        result.push(json!({"sourceMiB":mib,"readMiB":mib.min(3),"time":distribution(times)}));
    }
    Ok(json!(result))
}

fn real_scan(root: &Path, batched: bool) -> Result<Value, Box<dyn std::error::Error>> {
    let source = root.join("real-small-files");
    fs::create_dir_all(&source)?;
    for n in 0..10000 {
        let path = source.join(format!("{n:05}.jpg"));
        if !path.exists() {
            fs::write(path, b"synthetic scanner fixture")?;
        }
    }
    let paths = AppPaths::prepare(&root.join(if batched {
        "scan-runtime-batched"
    } else {
        "scan-runtime"
    }))?;
    if paths.database.exists() {
        return Err("Refusing to reuse an existing scan benchmark database".into());
    }
    let mut db = Database::open(&paths)?;
    let start = Instant::now();
    let first = db.scan(&source, &std::sync::atomic::AtomicBool::new(false))?;
    let initial = elapsed(start);
    assert_eq!(first.new, 10000);
    let start = Instant::now();
    let second = db.scan(&source, &std::sync::atomic::AtomicBool::new(false))?;
    let incremental = elapsed(start);
    assert_eq!(second.unchanged, 10000);
    assert_eq!(second.fingerprinted, 0);
    Ok(
        json!({"files":10000,"initialMs":initial,"incrementalMs":incremental,"initialFingerprints":first.fingerprinted,"incrementalFingerprints":second.fingerprinted}),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let size: i64 = args.get(1).ok_or("size required")?.parse()?;
    if ![10000, 100000].contains(&size) {
        return Err("Only 10000 or 100000 fixtures are supported".into());
    }
    let mode = args.get(2).map(String::as_str).unwrap_or("catalog");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(".tools/validation/phase11/scale-{size}"));
    fs::create_dir_all(&root)?;
    let exists = root.join("runtime/data/library.sqlite3").exists();
    if mode == "seed" && exists {
        return Err("Refusing to overwrite an existing fixture".into());
    }
    if mode != "seed" && !exists {
        return Err("Run seed first".into());
    }
    let paths = AppPaths::prepare(&root.join("runtime"))?;
    let value = match mode {
        "seed" => seed(&paths, size)?,
        "catalog" => catalog(&paths, size)?,
        "history" => history(&paths)?,
        "grow" => grow(&paths, size)?,
        "rebuild" => rebuild(&paths)?,
        "worker-rebuild" => worker_rebuild(&paths)?,
        "startup" => startup(&paths)?,
        "fingerprint" => fingerprint(&root)?,
        "scan" => real_scan(&root, false)?,
        "scan-batched" => real_scan(&root, true)?,
        _ => return Err("Unknown benchmark mode".into()),
    };
    let value = json!({"size":size,"mode":mode,"pid":std::process::id(),"result":value});
    fs::write(
        root.join(format!("{mode}.json")),
        serde_json::to_vec_pretty(&value)?,
    )?;
    println!("{value}");
    Ok(())
}
