//! Persisted records, not IPC commands or business services.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    Image,
    Video,
}

impl MediaType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Video => "video",
        }
    }
}

#[derive(Debug, Clone)]
pub struct NewMedia {
    pub id: String,
    pub fingerprint: Option<String>,
    pub file_path: String,
    pub file_name: String,
    pub media_type: MediaType,
    pub file_size: i64,
    pub created_time: Option<i64>,
    pub modified_time: i64,
    pub imported_at: i64,
    pub last_seen_at: i64,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct Media {
    pub id: String,
    pub fingerprint: Option<String>,
    pub file_path: String,
    pub file_name: String,
    pub media_type: MediaType,
    pub file_size: i64,
    pub created_time: Option<i64>,
    pub modified_time: i64,
    pub imported_at: i64,
    pub last_seen_at: i64,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration: Option<f64>,
    pub thumbnail_path: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub container: Option<String>,
    pub frame_rate: Option<f64>,
    pub like_count: i64,
    pub view_count: i64,
    pub play_count: i64,
    pub last_viewed_at: Option<i64>,
    pub last_played_at: Option<i64>,
    pub playback_position: f64,
    pub missing: bool,
    pub state: String,
    pub content_version_id: i64,
}

#[derive(Debug, Clone)]
pub struct MediaCursor {
    pub imported_at: i64,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomTag {
    pub id: String,
    pub name: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Only trusted filesystem observations belong here. No user-facing create command.
#[derive(Debug, Clone)]
pub struct ObservedDirectory {
    pub id: String,
    pub parent_id: Option<String>,
    pub source_root: String,
    pub directory_path: String,
    pub name: String,
    pub last_seen_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemTag {
    pub id: String,
    pub parent_id: Option<String>,
    pub source_root: String,
    pub directory_path: String,
    pub name: String,
    pub last_seen_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventCounts {
    pub likes: i64,
    pub views: i64,
    pub plays: i64,
}

#[derive(Debug, Clone)]
pub struct LikeEvent {
    pub id: String,
    pub ingest_seq: i64,
    pub request_id: String,
    pub media_id: String,
    pub occurred_at: i64,
    pub recorded_at: i64,
    pub policy_version: i64,
}

#[derive(Debug, Clone)]
pub struct ViewEvent {
    pub source: String,
    pub viewed_at: i64,
    pub id: String,
    pub ingest_seq: i64,
    pub request_id: String,
    pub media_id: String,
    pub activation_id: String,
    pub occurred_at: i64,
    pub recorded_at: i64,
    pub policy_version: i64,
}

#[derive(Debug, Clone)]
pub struct PlayEvent {
    pub id: String,
    pub ingest_seq: i64,
    pub request_id: String,
    pub media_id: String,
    pub content_version_id: i64,
    pub player_session_id: String,
    pub started_at: i64,
    pub qualified_at: i64,
    pub ended_at: Option<i64>,
    pub start_position: f64,
    pub end_position: f64,
    pub watched_seconds: f64,
    pub completed: bool,
    pub end_reason: Option<String>,
    pub policy_version: i64,
}

#[derive(Debug, Clone)]
pub struct QuickBrowseSession {
    pub id: String,
    pub seed: Option<String>,
    pub sort_mode: String,
    pub algorithm_version: i64,
    pub filter_json: String,
    pub state: String,
    pub item_count: i64,
    pub current_ordinal: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickBrowseItem {
    pub ordinal: i64,
    pub media_id: String,
    pub content_version_id: i64,
}
