//! Creates synthetic validation data only inside this checkout's .tools directory.
//! Run before the isolated desktop app; never points at the user's library.
use media_manager::{db::Database, metadata::Toolchain, platform::paths::AppPaths};
use std::{fs, path::Path, sync::atomic::AtomicBool};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let fixture = match std::env::args().nth(1).as_deref() {
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
            "2",
            "-c:v",
            "mpeg4",
        ])
        .arg(video)
        .output()?;
    if !output.status.success() {
        return Err("Synthetic video generation failed".into());
    }
    let paths = AppPaths::prepare(&runtime)?;
    let mut database = Database::open(&paths)?;
    let scan = database.scan(&source, &AtomicBool::new(false))?;
    if scan.failed != 0 || scan.new != 111 {
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
    if report.ready != 111 || report.failed != 0 {
        return Err(format!("Unexpected thumbnail result: {report:?}").into());
    }
    println!(
        "Seeded 111 synthetic media and thumbnails: {}",
        runtime.display()
    );
    Ok(())
}
