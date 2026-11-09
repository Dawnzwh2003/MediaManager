use crate::domain::library_query::*;
#[tauri::command]
pub async fn get_statistics(
    state: tauri::State<'_, Arc<AppState>>,
    media_id: String,
) -> AppResult<ActivityResult> {
    library_call(Arc::clone(state.inner()), move |db| {
        db.repositories().events().cached_statistics(&media_id)
    })
    .await
}
use crate::domain::viewer::*;
use std::sync::Arc;

use crate::{
    domain::AppStatus,
    error::{AppError, AppResult},
    services::app_status::AppState,
};

#[tauri::command]
pub async fn get_app_status(state: tauri::State<'_, Arc<AppState>>) -> AppResult<AppStatus> {
    let state = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || state.status())
        .await
        .map_err(|_| AppError::Internal)?
}

async fn library_call<T: Send + 'static>(
    state: Arc<AppState>,
    work: impl FnOnce(&mut crate::db::Database) -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(move || work(&mut state.database()?))
        .await
        .map_err(|_| AppError::Internal)?
}
#[tauri::command]
pub async fn query_library(
    state: tauri::State<'_, Arc<AppState>>,
    filter: LibraryFilter,
    cursor: Option<LibraryCursor>,
    limit: u32,
) -> AppResult<LibraryPage> {
    library_call(Arc::clone(state.inner()), move |db| {
        db.repositories()
            .library()
            .query(&filter, cursor.as_ref(), limit)
    })
    .await
}
#[tauri::command]
pub async fn get_media_details(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> AppResult<LibraryDetails> {
    library_call(Arc::clone(state.inner()), move |db| {
        db.repositories().library().details(&id)
    })
    .await
}
#[tauri::command]
pub async fn list_library_tags(
    state: tauri::State<'_, Arc<AppState>>,
    query: TagQuery,
) -> AppResult<TagPage> {
    library_call(Arc::clone(state.inner()), move |db| {
        db.repositories().library().tags(&query)
    })
    .await
}
#[tauri::command]
pub async fn mutate_custom_tag(
    state: tauri::State<'_, Arc<AppState>>,
    mutation: TagMutation,
) -> AppResult<Option<TagLabel>> {
    library_call(Arc::clone(state.inner()), move |db| {
        crate::services::library::mutate_tag(db, mutation)
    })
    .await
}
#[tauri::command]
pub async fn get_thumbnails(
    state: tauri::State<'_, Arc<AppState>>,
    ids: Vec<String>,
    retry: bool,
) -> AppResult<Vec<ThumbnailResult>> {
    library_call(Arc::clone(state.inner()), move |db| {
        crate::services::library::thumbnails(db, ids, retry)
    })
    .await
}

async fn viewer_call<T: Send + 'static>(
    state: Arc<AppState>,
    work: impl FnOnce(&crate::viewer::ViewerService, &mut crate::db::Database) -> AppResult<T>
        + Send
        + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(move || work(&state.viewers, &mut state.database()?))
        .await
        .map_err(|_| AppError::Internal)?
}
#[tauri::command]
pub async fn get_playback_state(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
    content_version_id: i64,
) -> AppResult<Option<PlaybackState>> {
    library_call(Arc::clone(state.inner()), move |db| {
        db.repositories().playback().get(&id, content_version_id)
    })
    .await
}
#[tauri::command]
pub async fn open_viewer(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> AppResult<ViewerOpen> {
    viewer_call(Arc::clone(state.inner()), move |service, db| {
        service.open(db, &id)
    })
    .await
}
#[tauri::command]
pub async fn close_viewer(state: tauri::State<'_, Arc<AppState>>, token: String) -> AppResult<()> {
    viewer_call(Arc::clone(state.inner()), move |service, db| {
        service.close(db, &token)
    })
    .await
}
#[tauri::command]
pub async fn save_playback_state(
    state: tauri::State<'_, Arc<AppState>>,
    request: ProgressWrite,
) -> AppResult<PlaybackState> {
    viewer_call(Arc::clone(state.inner()), move |service, db| {
        service.save(db, &request)
    })
    .await
}
#[tauri::command]
pub async fn prepare_playback(
    state: tauri::State<'_, Arc<AppState>>,
    token: String,
    mode: PlaybackMode,
) -> AppResult<()> {
    viewer_call(Arc::clone(state.inner()), move |service, db| {
        service.prepare(db, &token, mode, &crate::metadata::Toolchain::default())
    })
    .await
}
#[tauri::command]
pub async fn viewer_navigation(
    state: tauri::State<'_, Arc<AppState>>,
    filter: LibraryFilter,
    id: String,
    generation: Option<i64>,
) -> AppResult<ViewerNavigation> {
    library_call(Arc::clone(state.inner()), move |db| {
        db.repositories()
            .library()
            .navigation(&filter, &id, generation)
    })
    .await
}
use crate::domain::activity::{
    Activation, ActivityResult, PlayResult, PlaybackEvidence, StatisticsJob,
};

#[tauri::command]
pub async fn add_like(
    state: tauri::State<'_, Arc<AppState>>,
    media_id: String,
    request_id: String,
) -> AppResult<ActivityResult> {
    library_call(Arc::clone(state.inner()), move |db| {
        crate::services::activity::add_like(db, &media_id, &request_id)
    })
    .await
}
#[tauri::command]
pub async fn activate_media(
    state: tauri::State<'_, Arc<AppState>>,
    token: String,
    activation: Activation,
) -> AppResult<ActivityResult> {
    viewer_call(Arc::clone(state.inner()), move |service, db| {
        service.authorize_activity(db, &token, &activation)?;
        crate::services::activity::record_view(db, &activation)
    })
    .await
}
#[tauri::command]
pub async fn open_player_session(
    state: tauri::State<'_, Arc<AppState>>,
    activation_id: String,
) -> AppResult<String> {
    library_call(Arc::clone(state.inner()), move |db| {
        crate::services::activity::open_player_session(db, &activation_id)
    })
    .await
}
#[tauri::command]
pub async fn report_playback(
    state: tauri::State<'_, Arc<AppState>>,
    evidence: PlaybackEvidence,
) -> AppResult<PlayResult> {
    library_call(Arc::clone(state.inner()), move |db| {
        crate::services::activity::record_play(db, &evidence)
    })
    .await
}
#[tauri::command]
pub async fn close_activity(
    state: tauri::State<'_, Arc<AppState>>,
    activation_id: String,
) -> AppResult<()> {
    library_call(Arc::clone(state.inner()), move |db| {
        crate::services::activity::finalize_play(db, &activation_id)
    })
    .await
}
#[tauri::command]
pub async fn rebuild_statistics(
    state: tauri::State<'_, Arc<AppState>>,
    request_id: String,
    kind: String,
    media_id: Option<String>,
) -> AppResult<StatisticsJob> {
    library_call(Arc::clone(state.inner()), move |db| {
        crate::services::statistics::rebuild_statistics(db, &request_id, &kind, media_id.as_deref())
    })
    .await
}
#[tauri::command]
pub async fn statistics_job(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> AppResult<StatisticsJob> {
    library_call(Arc::clone(state.inner()), move |db| {
        crate::services::statistics::status(db, &id)
    })
    .await
}
#[tauri::command]
pub async fn cancel_statistics_rebuild(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> AppResult<()> {
    library_call(Arc::clone(state.inner()), move |db| {
        crate::services::statistics::cancel(db, &id)
    })
    .await
}
