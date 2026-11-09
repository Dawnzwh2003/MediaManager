use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ViewSource {
    Grid,
    NormalViewer,
    QuickBrowser,
    Search,
    History,
}
impl ViewSource {
    pub fn key(&self) -> &'static str {
        match self {
            Self::Grid => "grid",
            Self::NormalViewer => "normal_viewer",
            Self::QuickBrowser => "quick_browser",
            Self::Search => "search",
            Self::History => "history",
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Activation {
    pub id: String,
    pub viewer_id: String,
    pub ordinal: i64,
    pub media_id: String,
    pub content_version_id: i64,
    pub source: ViewSource,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackPhase {
    Playing,
    Paused,
    Waiting,
    Seeking,
    Ended,
    Closed,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlaybackEvidence {
    pub session_id: String,
    pub sequence: i64,
    pub clock_ms: f64,
    pub position: f64,
    pub duration: Option<f64>,
    pub rate: f64,
    pub phase: PlaybackPhase,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityResult {
    pub id: String,
    pub like_count: i64,
    pub view_count: i64,
    pub play_count: i64,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayResult {
    pub session_id: String,
    pub qualified: bool,
    pub watched_seconds: f64,
    pub finalized: bool,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatisticsJob {
    pub id: String,
    pub state: String,
    pub phase: i64,
    pub error: Option<String>,
}
