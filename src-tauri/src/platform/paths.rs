use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::{
    domain::StorageStatus,
    error::{AppError, AppResult},
};

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data: PathBuf,
    pub cache: PathBuf,
    pub thumbnails: PathBuf,
    pub database: PathBuf,
    pub instance_lock: PathBuf,
}

impl AppPaths {
    pub fn prepare(root: &Path) -> AppResult<Self> {
        if !root.is_absolute() {
            return Err(AppError::Storage(
                "application root must be absolute".into(),
            ));
        }
        ensure_directory(root, true)?;
        let data = root.join("data");
        let cache = root.join("cache");
        let thumbnails = cache.join("thumbnails");
        for directory in [&data, &cache, &thumbnails] {
            ensure_directory(directory, false)?;
        }
        Ok(Self {
            database: data.join("library.sqlite3"),
            instance_lock: data.join("instance.lock"),
            data,
            cache,
            thumbnails,
        })
    }

    pub fn status(&self) -> StorageStatus {
        StorageStatus {
            data_ready: self.data.is_dir(),
            cache_ready: self.cache.is_dir(),
            thumbnails_ready: self.thumbnails.is_dir(),
        }
    }
}

fn is_link(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Reject Windows junctions and other reparse points as well as symlinks.
        if metadata.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}

pub(crate) fn ensure_directory(path: &Path, recursive: bool) -> AppResult<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if !is_link(&metadata) && metadata.is_dir() => return Ok(()),
        Ok(_) => return Err(AppError::Storage("directory is a file or link".into())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let result = if recursive {
        fs::create_dir_all(path)
    } else {
        fs::create_dir(path)
    };
    if let Err(error) = result {
        if error.kind() != io::ErrorKind::AlreadyExists {
            return Err(error.into());
        }
    }
    let metadata = fs::symlink_metadata(path)?;
    if is_link(&metadata) || !metadata.is_dir() {
        return Err(AppError::Storage("created directory is not safe".into()));
    }
    Ok(())
}

pub(crate) fn validate_managed_file(path: &Path) -> AppResult<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !is_link(&metadata) => Ok(()),
        Ok(_) => Err(AppError::Storage(
            "database or lock path is not a regular file".into(),
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}
