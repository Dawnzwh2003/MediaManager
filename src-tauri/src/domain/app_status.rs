use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub app_name: &'static str,
    pub app_version: &'static str,
    pub phase: u8,
    pub database: DatabaseStatus,
    pub storage: StorageStatus,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseStatus {
    pub schema_version: u32,
    pub sqlite_version: String,
    pub foreign_keys: bool,
    pub journal_mode: String,
    pub busy_timeout_ms: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageStatus {
    pub data_ready: bool,
    pub cache_ready: bool,
    pub thumbnails_ready: bool,
}
