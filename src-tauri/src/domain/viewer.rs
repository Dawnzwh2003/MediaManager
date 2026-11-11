use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum PlaybackMode {
    Direct,
    Remux,
    Proxy,
}
impl PlaybackMode {
    pub fn key(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Remux => "remux",
            Self::Proxy => "proxy",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackState {
    pub position_seconds: f64,
    pub duration_seconds: Option<f64>,
    pub state_version: i64,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewerOpen {
    pub token: String,
    pub media_id: String,
    pub content_version_id: i64,
    pub media_type: String,
    pub mime: String,
    pub state: Option<PlaybackState>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProgressWrite {
    pub token: String,
    pub sequence: i64,
    pub expected_version: i64,
    pub position_seconds: f64,
    pub duration_seconds: Option<f64>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewerNavigation {
    pub previous_id: Option<String>,
    pub next_id: Option<String>,
    pub generation: i64,
    pub key: String,
}
