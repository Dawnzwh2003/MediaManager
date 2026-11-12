use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ReportKind {
    RecentlyViewed,
    RecentlyPlayed,
    MostViewed,
    MostPlayed,
    MostLiked,
    ContinueWatching,
    RecentlyAdded,
}

/// UTC milliseconds; inclusive start, exclusive end. Frozen across pagination.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimeWindow {
    pub start: Option<i64>,
    pub end: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReportQuery {
    pub kind: ReportKind,
    pub window: TimeWindow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReportCursor {
    pub generation: i64,
    pub signature: String,
    pub key: i64,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportItem {
    pub id: String,
    pub file_name: String,
    pub media_type: String,
    pub state: String,
    pub count: i64,
    pub last_at: i64,
    pub position: Option<f64>,
    pub duration: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportPage {
    pub items: Vec<ReportItem>,
    pub next_cursor: Option<ReportCursor>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    pub total_media: i64,
    pub images: i64,
    pub videos: i64,
    pub total_likes: i64,
    pub views_today: i64,
    pub recently_added: Vec<ReportItem>,
    pub recently_viewed: Vec<ReportItem>,
    pub continue_watching: Vec<ReportItem>,
    pub most_liked: Vec<ReportItem>,
    pub most_viewed: Vec<ReportItem>,
}
