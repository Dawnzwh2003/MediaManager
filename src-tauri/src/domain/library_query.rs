use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LibraryFilter {
    #[serde(default)]
    pub search: String,
    #[serde(default)]
    pub search_scope: SearchScope,
    pub media_type: Option<String>,
    pub system_tag_id: Option<String>,
    #[serde(default)]
    pub source_tag_id: Option<String>,
    pub custom_tag_id: Option<String>,
    #[serde(default)]
    pub custom_tag_ids: Vec<String>,
    #[serde(default)]
    pub custom_tag_mode: TagMode,
    pub likes: Option<InclusiveRange>,
    pub views: Option<InclusiveRange>,
    pub plays: Option<InclusiveRange>,
    pub imported: Option<InclusiveRange>,
    #[serde(default)]
    pub viewed: ViewedStatus,
    pub random: Option<RandomOrder>,
    #[serde(default)]
    pub sort: LibrarySort,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SearchScope {
    #[default]
    All,
    Filename,
    SystemTag,
    CustomTag,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TagMode {
    #[default]
    And,
    Or,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ViewedStatus {
    #[default]
    All,
    Viewed,
    Unviewed,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InclusiveRange {
    pub min: Option<i64>,
    pub max: Option<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RandomOrder {
    pub seed: String,
    pub version: u8,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum LibrarySort {
    #[default]
    Recent,
    Name,
    Likes,
    NameDesc,
    LikesAsc,
    Views,
    ViewsAsc,
    Plays,
    PlaysAsc,
    ImportedAsc,
    Created,
    CreatedAsc,
    RecentlyViewed,
    RecentlyPlayed,
    Random,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LibraryCursor {
    pub generation: i64,
    pub signature: String,
    pub id: String,
    /// Text or a lossless decimal SQLite integer, selected by the signed query contract.
    pub key: String,
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
