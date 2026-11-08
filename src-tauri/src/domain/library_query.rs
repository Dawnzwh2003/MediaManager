use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LibraryFilter {
    #[serde(default)]
    pub search: String,
    pub media_type: Option<String>,
    pub system_tag_id: Option<String>,
    pub custom_tag_id: Option<String>,
    #[serde(default)]
    pub sort: LibrarySort,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum LibrarySort {
    #[default]
    Recent,
    Name,
    Likes,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LibraryCursor {
    pub generation: i64,
    pub signature: String,
    pub id: String,
    pub imported_at: i64,
    pub file_name: String,
    pub like_count: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TagLabel {
    pub id: String,
    pub name: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryMedia {
    pub id: String,
    pub file_name: String,
    pub media_type: String,
    pub imported_at: i64,
    pub like_count: i64,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration: Option<f64>,
    pub custom_tags: Vec<TagLabel>,
    pub custom_tag_count: i64,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPage {
    pub items: Vec<LibraryMedia>,
    pub next_cursor: Option<LibraryCursor>,
    pub generation: i64,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryDetails {
    pub media: LibraryMedia,
    pub file_path: String,
    pub file_size: i64,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub container: Option<String>,
    pub frame_rate: Option<f64>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryTag {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub has_children: bool,
    pub attached: bool,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagPage {
    pub items: Vec<LibraryTag>,
    pub next_after: Option<String>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TagQuery {
    pub kind: String,
    pub parent_id: Option<String>,
    pub after_id: Option<String>,
    pub media_id: Option<String>,
    #[serde(default)]
    pub search: String,
    pub limit: u32,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase", deny_unknown_fields)]
pub enum TagMutation {
    Create {
        name: String,
    },
    Rename {
        id: String,
        name: String,
    },
    Delete {
        id: String,
    },
    #[serde(rename_all = "camelCase")]
    Attach {
        media_ids: Vec<String>,
        tag_ids: Vec<String>,
    },
    #[serde(rename_all = "camelCase")]
    Detach {
        media_ids: Vec<String>,
        tag_ids: Vec<String>,
    },
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThumbnailResult {
    pub media_id: String,
    pub status: String,
    pub path: Option<String>,
    pub error: Option<String>,
}
