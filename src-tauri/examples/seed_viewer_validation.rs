//! Synthetic Phase 6 media, confined to this checkout's .tools validation directory.
use media_manager::{db::Database, metadata::Toolchain, platform::paths::AppPaths};
use std::{fs, path::Path, sync::atomic::AtomicBool};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let phase = if std::env::args().nth(1).as_deref() == Some("--phase7") {
        "phase7"
    } else {
        "phase6"
    };
    let root = workspace.join(".tools/validation").join(phase);
    let runtime = root.join("runtime");
    if runtime.join("data/library.sqlite3").exists() {
        return Err("Validation database exists; refusing to overwrite it".into());
    }
    let source = root.join("Synthetic Media");
    fs::create_dir_all(&source)?;
    let tools = Toolchain::default();
    for name in ["01_Landscape.jpg", "02_Still.jpg"] {
        let out = std::process::Command::new(&tools.ffmpeg)
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=3200x1800:rate=1",
                "-frames:v",
                "1",
            ])
            .arg(source.join(name))
            .output()?;
        if !out.status.success() {
            return Err("Synthetic image failed".into());
        }
    }
    for (name, codec) in [("03_Direct.mp4", "libx264"), ("04_Fallback.mkv", "mpeg4")] {
        let out = std::process::Command::new(&tools.ffmpeg)
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=320x180:rate=10",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=44100",
                "-t",
                "30",
                "-c:v",
                codec,
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                "-b:a",
                "64k",
            ])
            .arg(source.join(name))
            .output()?;
        if !out.status.success() {
            return Err(format!(
                "Synthetic video failed: {}",
                String::from_utf8_lossy(&out.stderr)
            )
            .into());
        }
    }
    fs::write(source.join("05_Corrupt.png"), b"intentional corrupt image")?;
    let paths = AppPaths::prepare(&runtime)?;
    let mut db = Database::open(&paths)?;
    let scan = db.scan(&source, &AtomicBool::new(false))?;
    if scan.new != 5 || scan.failed != 0 {
        return Err("Unexpected scan result".into());
    }
    db.repositories()
        .custom_tags()
        .create("review", "Review", 1)?;
    db.repositories()
        .custom_tags()
        .create("sample", "Sample", 1)?;
    let report = db.process_derivatives(&tools, &AtomicBool::new(false))?;
    if report.ready != 4 || report.failed != 1 {
        return Err(format!("Unexpected metadata result {report:?}").into());
    }
    println!(
        "Seeded 5 synthetic media for {phase}: {}",
        runtime.display()
    );
    Ok(())
}
