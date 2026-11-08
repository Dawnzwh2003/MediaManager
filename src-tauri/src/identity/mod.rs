//! Filesystem evidence. A digest never identifies a logical file instance.
use std::{
    fs::{self, File, Metadata},
    io,
    path::Path,
    time::UNIX_EPOCH,
};

pub fn is_link(meta: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}

pub fn timestamp(time: io::Result<std::time::SystemTime>) -> Option<i64> {
    time.ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos()
        .try_into()
        .ok()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    pub key: Option<String>,
    pub stat: String,
    pub size: i64,
    pub modified: i64,
    pub created: Option<i64>,
}
impl Evidence {
    pub fn read(path: &Path) -> io::Result<Self> {
        let meta = fs::symlink_metadata(path)?;
        if is_link(&meta) || !meta.is_file() {
            return Err(io::Error::other("not a regular unlinked file"));
        }
        let file = File::open(path)?;
        let meta = file.metadata()?;
        let created = timestamp(meta.created());
        let modified =
            timestamp(meta.modified()).ok_or_else(|| io::Error::other("mtime unavailable"))?;
        let size = i64::try_from(meta.len()).map_err(io::Error::other)?;
        let key = file_key(&file, &meta)?;
        Ok(Self {
            stat: format!("{size}:{modified}:{created:?}:{key:?}"),
            key,
            size,
            modified,
            created,
        })
    }
}

#[cfg(windows)]
fn file_key(file: &File, _: &Metadata) -> io::Result<Option<String>> {
    use std::{ffi::c_void, os::windows::io::AsRawHandle};
    #[repr(C)]
    #[derive(Default)]
    struct Info {
        attributes: u32,
        creation: [u32; 2],
        access: [u32; 2],
        write: [u32; 2],
        volume: u32,
        size_high: u32,
        size_low: u32,
        links: u32,
        index_high: u32,
        index_low: u32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFileInformationByHandle(handle: *mut c_void, info: *mut Info) -> i32;
    }
    let mut info = Info::default();
    // SAFETY: the file owns a valid handle; Info matches BY_HANDLE_FILE_INFORMATION.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((info.links == 1 && info.creation != [0, 0]).then(|| {
        format!(
            "win:{}:{}:{}:{}:{}",
            info.volume, info.index_high, info.index_low, info.creation[0], info.creation[1]
        )
    }))
}
#[cfg(unix)]
fn file_key(_: &File, meta: &Metadata) -> io::Result<Option<String>> {
    use std::os::unix::fs::MetadataExt;
    Ok(timestamp(meta.created())
        .filter(|_| meta.nlink() == 1)
        .map(|birth| format!("unix:{}:{}:{birth}", meta.dev(), meta.ino())))
}
#[cfg(not(any(windows, unix)))]
fn file_key(_: &File, _: &Metadata) -> io::Result<Option<String>> {
    Ok(None)
}
