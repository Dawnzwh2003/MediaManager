//! Creates synthetic validation data only inside this checkout's .tools directory.
//! Run before the isolated desktop app; never points at the user's library.
use media_manager::{db::Database, metadata::Toolchain, platform::paths::AppPaths};
use std::{fs, path::Path, sync::atomic::AtomicBool};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let fixture = match std::env::args().nth(1).as_deref() {
        Some("--phase10") => "phase10",
        Some("--phase9") => "phase9",
        Some("--phase8") => "phase8",
        _ => "phase5-native",
    };
    let root = workspace.join(".tools/validation").join(fixture);
    let runtime = root.join("runtime");
    if runtime.join("data/library.sqlite3").exists() {
        return Err("Validation database already exists; refusing to overwrite it".into());
    }
    let source = root.join("Synthetic Collection");
    let photos = source.join("Photos/Landscapes");
    let clips = source.join("Videos");
    fs::create_dir_all(&photos)?;
    fs::create_dir_all(&clips)?;
    let tools = Toolchain::default();
    for i in 0..110 {
        let name = format!(
            "{i:03}_{}.jpg",
            [
                "Alpine light",
                "Quiet coast",
                "Field notes",
                "Studio study",
                "Evening walk",
                "Still moments"
            ][i % 6]
        );
        fs::copy(
            workspace.join(format!("tests/ui/assets/{}.jpg", i % 6)),
            photos.join(name),
        )?;
    }
    let video = clips.join("Synthetic clip.mp4");
    let output = std::process::Command::new(&tools.ffmpeg)
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=320x180:rate=10",
            "-t",
            if fixture == "phase10" { "30" } else { "2" },
            "-c:v",
            if fixture == "phase10" {
                "libx264"
            } else {
                "mpeg4"
            },
        ])
        .arg(&video)
        .output()?;
    if !output.status.success() {
        return Err("Synthetic video generation failed".into());
    }
    if fixture == "phase10" {
        fs::copy(&video, clips.join("Completed clip.mp4"))?;
    }
    let media_count = if fixture == "phase10" { 112 } else { 111 };
    let paths = AppPaths::prepare(&runtime)?;
    let mut database = Database::open(&paths)?;
    let scan = database.scan(&source, &AtomicBool::new(false))?;
    if scan.failed != 0 || scan.new != media_count {
        return Err(format!("Unexpected scan result: {scan:?}").into());
    }
    for (id, name) in [
        ("validation-travel", "旅行"),
        ("validation-ideas", "灵感"),
        ("validation-later", "待整理"),
    ] {
        database.repositories().custom_tags().create(id, name, 1)?;
    }
    for (index, media) in database
        .repositories()
        .media()
        .list(None, 200)?
        .iter()
        .enumerate()
    {
        if index % 3 == 0 {
            database
                .repositories()
                .custom_tags()
                .attach(&media.id, "validation-travel")?;
        }
        if index % 5 == 0 {
            database
                .repositories()
                .custom_tags()
                .attach(&media.id, "validation-ideas")?;
        }
    }
    let report = database.process_derivatives(&tools, &AtomicBool::new(false))?;
    if report.ready != media_count || report.failed != 0 {
        return Err(format!("Unexpected thumbnail result: {report:?}").into());
    }
    if fixture == "phase10" {
        // Synthetic historical facts only; never used by the product entry point.
        let sql = rusqlite::Connection::open(&paths.database)?;
        sql.pragma_update(None, "foreign_keys", true)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis() as i64;
        sql.execute("WITH numbered AS (SELECT id,row_number() OVER(ORDER BY file_name) AS n FROM media)
          INSERT INTO view_events(id,request_id,activation_id,ingest_seq,media_id,occurred_at,recorded_at)
          SELECT 'view-'||n,'view-'||n,'view-'||n,n,id,?1-CASE n%4 WHEN 0 THEN 0 WHEN 1 THEN 3 WHEN 2 THEN 14 ELSE 40 END*86400000,?1 FROM numbered", [now])?;
        sql.execute("WITH numbered AS (SELECT id,row_number() OVER(ORDER BY file_name) AS n FROM media WHERE media_type='video'),
          events AS (SELECT id,n,n*2 AS seq,?1 AS t FROM numbered UNION ALL SELECT id,n,n*2+1,?1-3*86400000 FROM numbered)
          INSERT INTO play_events(id,request_id,player_session_id,ingest_seq,media_id,content_version_id,started_at,qualified_at,start_position,end_position,watched_seconds,completed)
          SELECT 'play-'||seq,'play-'||seq,'play-'||seq,1000+seq,id,1,t-5000,t,0,5,5,0 FROM events", [now])?;
        sql.execute("INSERT INTO like_events(id,request_id,ingest_seq,media_id,occurred_at,recorded_at)
          SELECT 'like-'||v,'like-'||v,2000+v,(SELECT id FROM media ORDER BY file_name LIMIT 1),?1,?1 FROM (SELECT 1 AS v UNION ALL SELECT 2 UNION ALL SELECT 3 UNION ALL SELECT 4)", [now])?;
        sql.execute("INSERT INTO like_events(id,request_id,ingest_seq,media_id,occurred_at,recorded_at)
          SELECT 'like-video','like-video',3000,id,?1,?1 FROM media WHERE file_name='Synthetic clip.mp4'",[now])?;
        sql.execute("INSERT INTO playback_states(media_id,content_version_id,position_seconds,duration_seconds,completed,updated_at)
          SELECT id,1,CASE WHEN file_name='Completed clip.mp4' THEN 28.5 ELSE 7.5 END,30,0,?1 FROM media WHERE media_type='video'", [now])?;
        sql.execute("UPDATE db_sequence SET value=4000 WHERE id=1", [])?;
    }
    println!(
        "Seeded {media_count} synthetic media and thumbnails: {}",
        runtime.display()
    );
    Ok(())
}
