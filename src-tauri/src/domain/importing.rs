use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportJob {
    pub id: String,
    pub source_root: String,
    pub storage_root: String,
    pub state: String,
    pub stage: String,
    pub scanned_files: i64,
    pub new_media: i64,
    pub existing_media: i64,
    pub duplicate_files: i64,
    pub new_tags: i64,
    pub conflicts: i64,
    pub estimated_storage_required: i64,
    pub processed: i64,
    pub newly_imported: i64,
    pub existing_media_reused: i64,
    pub tags_added: i64,
    pub failed_files: i64,
    pub current_file: Option<String>,
    pub error: Option<String>,
    pub failures: Vec<ImportFailure>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFailure {
    pub relative_path: String,
    pub error: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportStorageSettings {
    pub media_root: String,
    pub available_bytes: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceNode {
    pub id: String,
    pub name: String,
    pub source_id: String,
    pub parent_id: Option<String>,
    pub relative_path: String,
    pub has_children: bool,
    pub root: bool,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcePage {
    pub items: Vec<SourceNode>,
    pub next_after: Option<String>,
}
