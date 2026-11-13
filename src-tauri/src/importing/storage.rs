//! Read-only sources, owned part files and atomic no-replace promotion.
use crate::{
    error::{AppError, AppResult},
    identity::{is_link, Evidence},
    scanner::fingerprint,
};
use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Component, Path, PathBuf},
    sync::atomic::AtomicBool,
    time::{Duration, UNIX_EPOCH},
};

pub fn checked_directory(path: &Path) -> AppResult<PathBuf> {
    if !path.is_absolute() {
        return Err(AppError::Validation("absolute directory required".into()));
    }
    for ancestor in path.ancestors() {
        let m = fs::symlink_metadata(ancestor)?;
        if is_link(&m) {
            return Err(AppError::Storage("linked directory".into()));
        }
    }
    let result = fs::canonicalize(path)?;
    if !result.is_dir() {
        return Err(AppError::Validation("not a directory".into()));
    }
    Ok(result)
}
pub fn prepare_directory(path: &Path) -> AppResult<PathBuf> {
    if !path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(AppError::Validation("unsafe storage path".into()));
    }
    for ancestor in path.ancestors() {
        if fs::symlink_metadata(ancestor).is_ok_and(|m| is_link(&m)) {
            return Err(AppError::Storage("linked storage".into()));
        }
    }
    fs::create_dir_all(path)?;
    checked_directory(path)
}
pub fn overlap(a: &Path, b: &Path) -> bool {
    #[cfg(windows)]
    {
        let a = PathBuf::from(a.to_string_lossy().to_lowercase());
        let b = PathBuf::from(b.to_string_lossy().to_lowercase());
        a.starts_with(&b) || b.starts_with(&a)
    }
    #[cfg(not(windows))]
    {
        a.starts_with(b) || b.starts_with(a)
    }
}
pub fn text(path: &Path) -> AppResult<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| AppError::Validation("non Unicode path".into()))
}
pub fn relative(path: &Path) -> AppResult<String> {
    let mut parts = vec![];
    for part in path.components() {
        match part {
            Component::Normal(v) => parts.push(v.to_str().ok_or(AppError::Internal)?),
            _ => return Err(AppError::Validation("unsafe relative path".into())),
        }
    }
    Ok(parts.join("/"))
}
pub fn join(root: &Path, relative: &str) -> AppResult<PathBuf> {
    let p = Path::new(relative);
    if p.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(AppError::Validation("unsafe relative path".into()));
    }
    let result = root.join(p);
    for ancestor in result.ancestors() {
        if fs::symlink_metadata(ancestor).is_ok_and(|m| is_link(&m)) {
            return Err(AppError::Storage("linked import path".into()));
        }
    }
    Ok(result)
}
pub fn evidence(path: &Path) -> AppResult<Evidence> {
    for p in path.ancestors() {
        if is_link(&fs::symlink_metadata(p)?) {
            return Err(AppError::Storage("linked source".into()));
        }
    }
    Ok(Evidence::read(path)?)
}

pub trait ImportStorage: Send + Sync {
    fn available(&self, path: &Path) -> io::Result<u64>;
    fn copy(&self, source: &Path, temporary: &Path, cancel: &AtomicBool) -> io::Result<String>;
}
pub struct LocalStorage;
impl ImportStorage for LocalStorage {
    fn available(&self, path: &Path) -> io::Result<u64> {
        fs2::available_space(path)
    }
    fn copy(&self, source: &Path, temporary: &Path, cancel: &AtomicBool) -> io::Result<String> {
        let mut input = File::open(source)?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(temporary)?;
        let hash = fingerprint::complete(&mut input, &mut output, cancel)?;
        output.sync_all()?;
        Ok(hash)
    }
}
pub fn set_modified(path: &Path, nanos: i64) -> AppResult<()> {
    let modified = UNIX_EPOCH
        .checked_add(Duration::from_nanos(
            nanos.try_into().map_err(|_| AppError::Internal)?,
        ))
        .ok_or(AppError::Internal)?;
    let file = OpenOptions::new().write(true).open(path)?;
    file.set_times(fs::FileTimes::new().set_modified(modified))?;
    file.sync_all()?;
    Ok(())
}
pub fn promote(part: &Path, target: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
        }
        let a: Vec<u16> = part.as_os_str().encode_wide().chain(Some(0)).collect();
        let b: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: NUL-terminated paths. WRITE_THROUGH only: never REPLACE_EXISTING.
        if unsafe { MoveFileExW(a.as_ptr(), b.as_ptr(), 8) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        fs::hard_link(part, target)?;
        fs::remove_file(part)?;
        File::open(
            target
                .parent()
                .ok_or_else(|| io::Error::other("no parent"))?,
        )?
        .sync_all()
    }
}
/// Only an exact per-job part name, never a source or a completed media file.
pub fn remove_part(part: &Path, job: &str) -> AppResult<()> {
    if part.file_name().and_then(|s| s.to_str()) != Some(&format!(".import-{job}.part")) {
        return Err(AppError::Storage("not an import part".into()));
    }
    if part.exists() {
        let m = fs::symlink_metadata(part)?;
        if is_link(&m) || !m.is_file() {
            return Err(AppError::Storage("unsafe part".into()));
        }
        fs::remove_file(part)?;
    }
    Ok(())
}
