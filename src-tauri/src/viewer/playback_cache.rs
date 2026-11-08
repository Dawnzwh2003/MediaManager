use super::*;
use crate::{
    metadata::{run_process, Failure},
    platform::paths::{ensure_directory, validate_managed_file},
};
use sha2::{Digest, Sha256};
use std::{process::Command, sync::TryLockError, time::Duration};
static PLAYBACK_GATE: Mutex<()> = Mutex::new(());
const MAX_OUTPUT: u64 = 512 * 1024 * 1024;
const QUOTA: u64 = 2 * 1024 * 1024 * 1024;
fn failure(error: Failure) -> AppError {
    AppError::MediaUnavailable(error.code().into())
}
fn safe_directory(root: &Path) -> AppResult<()> {
    for ancestor in root.ancestors() {
        if fs::symlink_metadata(ancestor).is_ok_and(|m| is_link(&m)) {
            return Err(AppError::Storage("linked playback cache".into()));
        }
    }
    ensure_directory(root, false)
}
fn reserve(root: &Path, pins: &[PathBuf]) -> AppResult<()> {
    let mut total = 0;
    let mut oldest = vec![];
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !((name.len() == 68 && name.ends_with(".mp4"))
            || (name.len() == 73 && name.ends_with(".part.mp4")))
            || !name.as_bytes()[..64].iter().all(|c| c.is_ascii_hexdigit())
        {
            continue;
        }
        let meta = fs::symlink_metadata(&path)?;
        if is_link(&meta) || !meta.is_file() {
            continue;
        }
        total += meta.len();
        if !pins.contains(&path) {
            oldest.push((meta.modified()?, path, meta.len()));
            oldest.sort_by_key(|v| v.0);
            oldest.truncate(64);
        }
    }
    for (_, path, size) in oldest {
        if total + MAX_OUTPUT <= QUOTA {
            break;
        }
        validate_managed_file(&path)?;
        fs::remove_file(path)?;
        total = total.saturating_sub(size);
    }
    if total + MAX_OUTPUT > QUOTA {
        return Err(AppError::MediaUnavailable(
            "playback cache quota reached".into(),
        ));
    }
    Ok(())
}
pub(super) fn prepare(
    database: &mut Database,
    service: &ViewerService,
    session: &Session,
    mode: PlaybackMode,
    tools: &Toolchain,
) -> AppResult<ResourceFile> {
    let _permit = loop {
        if session.cancel.load(Ordering::Relaxed) {
            return Err(failure(Failure::Cancelled));
        }
        match PLAYBACK_GATE.try_lock() {
            Ok(lock) => break lock,
            Err(TryLockError::WouldBlock) => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => return Err(AppError::Internal),
        }
    };
    let root = database.paths.cache.join("playback");
    safe_directory(&root)?;
    let key = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(
            &session.media.id,
            session.media.content_version_id,
            &session.media.fingerprint,
            session.media.modified_time,
            session.media.file_size,
            mode,
            "h264-aac-yuv420p-720p-v1"
        ))?)
    );
    let target = root.join(format!("{key}.mp4"));
    validate_managed_file(&target)?;
    let valid = |path: &Path| -> bool {
        let Ok(meta) = fs::symlink_metadata(path) else {
            return false;
        };
        if is_link(&meta) || !meta.is_file() || meta.len() == 0 || meta.len() > MAX_OUTPUT {
            return false;
        }
        tools.probe(path, true, &session.cancel).is_ok_and(|m| {
            match (session.media.duration, m.duration) {
                (Some(expected), Some(actual)) => {
                    (actual - expected).abs() <= 1.0_f64.max(expected * 0.01)
                }
                (Some(_), None) => false,
                _ => true,
            }
        })
    };
    if valid(&target) {
        return ResourceFile::open(&target, "video/mp4".into());
    }
    reserve(&root, &service.pinned_paths()?)?;
    let temporary = root.join(format!("{key}.part.mp4"));
    validate_managed_file(&temporary)?;
    if temporary.exists() {
        fs::remove_file(&temporary)?;
    }
    let result = (|| {
        // Use the latest location for the same version; file observation checks surround FFmpeg.
        let current = database
            .repositories()
            .media()
            .get(&session.media.id)?
            .ok_or(AppError::NotFound)?;
        let evidence = Evidence::read(Path::new(&current.file_path))?;
        if evidence != session.source.evidence {
            return Err(AppError::MediaUnavailable("source changed".into()));
        }
        let mut command = Command::new(&tools.ffmpeg);
        command
            .args([
                "-hide_banner",
                "-v",
                "error",
                "-nostdin",
                "-n",
                "-max_alloc",
                "67108864",
                "-threads",
                "2",
                "-filter_threads",
                "1",
                "-protocol_whitelist",
                "file",
                "-format_whitelist",
                "mov,matroska,webm,avi",
                "-max_pixels",
                "100000000",
                "-i",
            ])
            .arg(&current.file_path)
            .args(["-map", "0:V:0", "-map", "0:a:0?", "-sn", "-dn"]);
        if mode == PlaybackMode::Remux {
            command.args(["-c", "copy"]);
        } else {
            command.args(["-c:v","libx264","-preset","veryfast","-crf","23","-vf","scale=w='min(1280,iw)':h='min(720,ih)':force_original_aspect_ratio=decrease:force_divisible_by=2","-pix_fmt","yuv420p","-c:a","aac","-b:a","128k","-threads","2"]);
        }
        command
            .args([
                "-movflags",
                "+faststart",
                "-fs",
                &MAX_OUTPUT.to_string(),
                "-f",
                "mp4",
            ])
            .arg(&temporary);
        run_process(command, Duration::from_secs(600), &session.cancel).map_err(failure)?;
        if session.cancel.load(Ordering::Relaxed) {
            return Err(failure(Failure::Cancelled));
        }
        if Evidence::read(Path::new(&current.file_path))? != evidence || !valid(&temporary) {
            return Err(AppError::MediaUnavailable(
                "incomplete playback output".into(),
            ));
        }
        ViewerService::validate(database, session)?;
        validate_managed_file(&target)?;
        if target.exists() {
            fs::remove_file(&target)?;
        }
        fs::rename(&temporary, &target)?;
        ResourceFile::open(&target, "video/mp4".into())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}
