//! Read-only, revocable media handles and independent playback recovery.
mod playback_cache;
use crate::{
    db::Database,
    domain::{
        library::{Media, MediaType},
        viewer::*,
    },
    error::{AppError, AppResult},
    identity::{is_link, Evidence},
    metadata::Toolchain,
};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) struct ResourceFile {
    file: Mutex<File>,
    path: PathBuf,
    evidence: Evidence,
    mime: String,
}
impl ResourceFile {
    fn open(path: &Path, mime: String) -> AppResult<Self> {
        for ancestor in path.ancestors() {
            if is_link(&fs::symlink_metadata(ancestor)?) {
                return Err(AppError::Storage("linked media resource".into()));
            }
        }
        let file = File::open(path)?;
        let evidence = Evidence::from_file(&file)?;
        Ok(Self {
            file: Mutex::new(file),
            path: path.to_owned(),
            evidence,
            mime,
        })
    }
}
pub(crate) struct Session {
    media: Media,
    source: Arc<ResourceFile>,
    variants: Mutex<HashMap<PlaybackMode, Arc<ResourceFile>>>,
    cancel: AtomicBool,
}
#[derive(Default)]
pub struct ViewerService {
    sessions: Mutex<HashMap<String, Arc<Session>>>,
    read_gate: Mutex<()>,
}
impl Drop for ViewerService {
    fn drop(&mut self) {
        self.cancel_all();
    }
}
pub struct ResourceReply {
    pub status: u16,
    pub mime: String,
    pub content_range: Option<String>,
    pub content_length: u64,
    pub body: Vec<u8>,
}
pub(crate) fn now() -> AppResult<i64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AppError::Internal)?
        .as_millis() as i64)
}
impl ViewerService {
    fn session(&self, token: &str) -> AppResult<Arc<Session>> {
        if token.len() != 32 || !token.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(AppError::NotFound);
        }
        self.sessions
            .lock()
            .map_err(|_| AppError::Internal)?
            .get(token)
            .cloned()
            .ok_or(AppError::NotFound)
    }
    fn validate(database: &mut Database, session: &Session) -> AppResult<()> {
        if session.cancel.load(Ordering::Relaxed) {
            return Err(AppError::NotFound);
        }
        let current = database
            .repositories()
            .media()
            .get(&session.media.id)?
            .ok_or(AppError::NotFound)?;
        if current.missing
            || current.content_version_id != session.media.content_version_id
            || current.fingerprint != session.media.fingerprint
            || current.file_size != session.media.file_size
            || current.modified_time != session.media.modified_time
        {
            return Err(AppError::MediaUnavailable("content changed".into()));
        }
        let file = session.source.file.lock().map_err(|_| AppError::Internal)?;
        if Evidence::from_file(&file)? != session.source.evidence {
            return Err(AppError::MediaUnavailable("source changed".into()));
        }
        Ok(())
    }
    pub fn open(&self, database: &mut Database, id: &str) -> AppResult<ViewerOpen> {
        let media = database
            .repositories()
            .media()
            .get(id)?
            .filter(|m| !m.missing)
            .ok_or(AppError::NotFound)?;
        let mime = source_mime(&media)?;
        let source = Arc::new(ResourceFile::open(
            Path::new(&media.file_path),
            mime.clone(),
        )?);
        if source.evidence.size != media.file_size
            || source.evidence.modified != media.modified_time
        {
            return Err(AppError::MediaUnavailable("rescan changed source".into()));
        }
        let mut sessions = self.sessions.lock().map_err(|_| AppError::Internal)?;
        if sessions.len() >= 8 {
            return Err(AppError::Busy);
        }
        let (token, state) = if media.media_type == MediaType::Video {
            let (token, state) =
                database
                    .repositories()
                    .playback()
                    .claim(id, media.content_version_id, now()?)?;
            (token, Some(state))
        } else {
            (database.repositories().library().new_tag_id()?, None)
        };
        let reply = ViewerOpen {
            token: token.clone(),
            media_id: media.id.clone(),
            content_version_id: media.content_version_id,
            media_type: media.media_type.as_str().into(),
            mime,
            state,
        };
        sessions.insert(
            token,
            Arc::new(Session {
                media,
                source,
                variants: Mutex::new(HashMap::new()),
                cancel: AtomicBool::new(false),
            }),
        );
        Ok(reply)
    }
    pub fn save(
        &self,
        database: &mut Database,
        request: &ProgressWrite,
    ) -> AppResult<PlaybackState> {
        let session = self.session(&request.token)?;
        Self::validate(database, &session)?;
        if session.media.media_type != MediaType::Video {
            return Err(AppError::Validation("images have no playback state".into()));
        }
        database.repositories().playback().save(
            &session.media.id,
            session.media.content_version_id,
            request,
            now()?,
        )
    }
    pub fn close(&self, database: &mut Database, token: &str) -> AppResult<()> {
        if let Some(session) = self
            .sessions
            .lock()
            .map_err(|_| AppError::Internal)?
            .remove(token)
        {
            session.cancel.store(true, Ordering::Relaxed);
            database.repositories().playback().release(token)?;
        }
        Ok(())
    }
    pub fn cancel_all(&self) {
        if let Ok(mut sessions) = self.sessions.lock() {
            for session in sessions.values() {
                session.cancel.store(true, Ordering::Relaxed);
            }
            sessions.clear();
        }
    }
    pub fn prepare(
        &self,
        database: &mut Database,
        token: &str,
        mode: PlaybackMode,
        tools: &Toolchain,
    ) -> AppResult<()> {
        let session = self.session(token)?;
        Self::validate(database, &session)?;
        if mode == PlaybackMode::Direct || session.media.media_type != MediaType::Video {
            return Err(AppError::Validation("invalid playback variant".into()));
        }
        let file = playback_cache::prepare(database, self, &session, mode, tools)?;
        Self::validate(database, &session)?;
        session
            .variants
            .lock()
            .map_err(|_| AppError::Internal)?
            .insert(mode, Arc::new(file));
        Ok(())
    }
    pub fn read(
        &self,
        database: &mut Database,
        path: &str,
        method: &str,
        range: Option<&str>,
    ) -> AppResult<ResourceReply> {
        let _reader = self.read_gate.lock().map_err(|_| AppError::Internal)?;
        if method != "GET" && method != "HEAD" {
            return Ok(empty(405));
        }
        let mut parts = path.trim_start_matches('/').split('/');
        let token = parts.next().unwrap_or("");
        let mode = match parts.next() {
            Some("direct") => PlaybackMode::Direct,
            Some("remux") => PlaybackMode::Remux,
            Some("proxy") => PlaybackMode::Proxy,
            _ => return Ok(empty(404)),
        };
        if parts.next().is_some() {
            return Ok(empty(404));
        }
        let session = self.session(token)?;
        Self::validate(database, &session)?;
        let resource = if mode == PlaybackMode::Direct {
            Arc::clone(&session.source)
        } else {
            session
                .variants
                .lock()
                .map_err(|_| AppError::Internal)?
                .get(&mode)
                .cloned()
                .ok_or(AppError::NotFound)?
        };
        let mut file = resource.file.lock().map_err(|_| AppError::Internal)?;
        if Evidence::from_file(&file)? != resource.evidence {
            return Err(AppError::MediaUnavailable("resource changed".into()));
        }
        let size = resource.evidence.size as u64;
        let video = session.media.media_type == MediaType::Video;
        if !video && size > 64 * 1024 * 1024 {
            return Ok(empty(413));
        }
        let (start, end, status) = match byte_range(range, size, video) {
            Some(r) => r,
            None => {
                return Ok(ResourceReply {
                    status: 416,
                    mime: resource.mime.clone(),
                    content_range: Some(format!("bytes */{size}")),
                    content_length: 0,
                    body: vec![],
                })
            }
        };
        let length = end - start + 1;
        let body = if method == "HEAD" {
            vec![]
        } else {
            file.seek(SeekFrom::Start(start))?;
            let mut bytes = vec![0; length as usize];
            file.read_exact(&mut bytes)?;
            bytes
        };
        Ok(ResourceReply {
            status,
            mime: resource.mime.clone(),
            content_range: (status == 206).then(|| format!("bytes {start}-{end}/{size}")),
            content_length: length,
            body,
        })
    }
    pub(crate) fn pinned_paths(&self) -> AppResult<Vec<PathBuf>> {
        let mut paths = vec![];
        for session in self
            .sessions
            .lock()
            .map_err(|_| AppError::Internal)?
            .values()
        {
            for file in session
                .variants
                .lock()
                .map_err(|_| AppError::Internal)?
                .values()
            {
                paths.push(file.path.clone());
            }
        }
        Ok(paths)
    }
}
pub fn empty(status: u16) -> ResourceReply {
    ResourceReply {
        status,
        mime: "text/plain".into(),
        content_range: None,
        content_length: 0,
        body: vec![],
    }
}
fn source_mime(media: &Media) -> AppResult<String> {
    let extension = Path::new(&media.file_path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    Ok(match extension.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "avi" => "video/x-msvideo",
        _ => return Err(AppError::MediaUnavailable("unsupported media type".into())),
    }
    .into())
}
fn byte_range(range: Option<&str>, size: u64, video: bool) -> Option<(u64, u64, u16)> {
    if size == 0 {
        return None;
    }
    let cap = if video {
        4 * 1024 * 1024
    } else {
        64 * 1024 * 1024
    };
    let Some(range) = range else {
        return Some((
            0,
            (size - 1).min(cap - 1),
            if video && size > cap { 206 } else { 200 },
        ));
    };
    let (a, b) = range.strip_prefix("bytes=")?.split_once('-')?;
    if b.contains(',') {
        return None;
    }
    let (start, end) = if a.is_empty() {
        let suffix = b.parse::<u64>().ok()?;
        if suffix == 0 {
            return None;
        }
        (size.saturating_sub(suffix), size - 1)
    } else {
        let start = a.parse::<u64>().ok()?;
        let end = if b.is_empty() {
            size - 1
        } else {
            b.parse::<u64>().ok()?.min(size - 1)
        };
        (start, end)
    };
    if start >= size || end < start {
        return None;
    }
    Some((start, end.min(start.saturating_add(cap - 1)), 206))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ranges_are_bounded_and_reject_invalid_requests() {
        assert_eq!(byte_range(Some("bytes=2-5"), 10, true), Some((2, 5, 206)));
        assert_eq!(byte_range(Some("bytes=-3"), 10, true), Some((7, 9, 206)));
        assert_eq!(byte_range(Some("bytes=10-"), 10, true), None);
        assert_eq!(byte_range(Some("bytes=0-1,3-4"), 10, true), None);
        assert_eq!(byte_range(Some("bytes=-0"), 10, true), None);
        assert_eq!(
            byte_range(None, 10_000_000, true),
            Some((0, 4 * 1024 * 1024 - 1, 206))
        );
    }
}
