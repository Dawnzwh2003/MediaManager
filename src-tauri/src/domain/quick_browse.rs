use crate::domain::library_query::LibraryFilter;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuickBrowseRequest {
    pub request_id: String,
    pub filter: LibraryFilter,
    pub autoplay: bool,
    pub muted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseSession {
    pub id: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub sort_mode: String,
    pub filter: LibraryFilter,
    pub random_seed: Option<String>,
    pub algorithm_version: i64,
    pub state: String,
    pub phase: String,
    pub item_count: i64,
    pub built_count: i64,
    pub current_ordinal: Option<i64>,
    pub autoplay: bool,
    pub muted: bool,
    pub error: Option<String>,
    pub parent_session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseItem {
    pub ordinal: i64,
    pub media_id: String,
    pub content_version_id: i64,
    pub file_name: String,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TagBinding {
    pub digit: u8,
    pub tag_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedBinding {
    pub digit: u8,
    pub tag_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrowsePreferences {
    pub autoplay: bool,
    #[serde(default = "default_muted")]
    pub muted: bool,
    pub bindings: Vec<TagBinding>,
}

fn default_muted() -> bool {
    true
}
impl Default for BrowsePreferences {
    fn default() -> Self {
        Self {
            autoplay: false,
            muted: true,
            bindings: Vec::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseSettings {
    pub autoplay: bool,
    pub muted: bool,
    pub bindings: Vec<ResolvedBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToggleTagRequest {
    pub request_id: String,
    pub session_id: String,
    pub ordinal: i64,
    pub digit: u8,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleTagResult {
    pub media_id: String,
    pub tag_id: String,
    pub tag_name: String,
    pub attached: bool,
}
