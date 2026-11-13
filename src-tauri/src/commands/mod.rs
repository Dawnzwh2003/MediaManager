use crate::domain::importing::*;
use crate::domain::library_query::*;
use crate::domain::quick_browse::*;
use crate::domain::reports::*;
use tauri_plugin_dialog::DialogExt;

async fn pick_directory(
    app: tauri::AppHandle,
    title: &'static str,
) -> AppResult<Option<std::path::PathBuf>> {
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title(title)
            .blocking_pick_folder()
            .map(|path| {
                path.into_path().map_err(|_| {
                    crate::error::AppError::Validation("local directory required".into())
                })
            })
            .transpose()
    })
    .await
    .map_err(|_| crate::error::AppError::Internal)?
}
#[tauri::command]
pub async fn pick_import_folder(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> AppResult<Option<ImportJob>> {
    let Some(path) = pick_directory(app, "Import Folder").await? else {
        return Ok(None);
    };
    library_call(Arc::clone(state.inner()), move |db| {
        crate::importing::create_preview(db, &path).map(Some)
    })
    .await
}
#[tauri::command]
pub async fn get_import_job(
    state: tauri::State<'_, Arc<AppState>>,
    id: Option<String>,
) -> AppResult<Option<ImportJob>> {
    library_call(Arc::clone(state.inner()), move |db| {
        crate::importing::get_job(db, id.as_deref())
    })
    .await
}
#[tauri::command]
pub async fn start_folder_import(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
    resume: bool,
) -> AppResult<ImportJob> {
    library_call(Arc::clone(state.inner()), move |db| {
        crate::importing::start_import(db, &id, resume, &crate::importing::storage::LocalStorage)
    })
    .await
}
#[tauri::command]
pub async fn cancel_folder_import(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> AppResult<()> {
    state.cancel_import(&id)?;
    library_call(Arc::clone(state.inner()), move |db| {
        crate::importing::cancel_import(db, &id)
    })
    .await
}
#[tauri::command]
pub async fn get_import_storage(
    state: tauri::State<'_, Arc<AppState>>,
) -> AppResult<ImportStorageSettings> {
    library_call(
        Arc::clone(state.inner()),
        crate::importing::storage_settings,
    )
    .await
}
#[tauri::command]
pub async fn pick_import_storage(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> AppResult<Option<ImportStorageSettings>> {
    let Some(path) = pick_directory(app, "Media data storage").await? else {
        return Ok(None);
    };
    library_call(Arc::clone(state.inner()), move |db| {
        crate::importing::set_storage(db, &path).map(Some)
    })
    .await
}
#[tauri::command]
pub async fn list_import_sources(
    state: tauri::State<'_, Arc<AppState>>,
    source_id: Option<String>,
    parent_id: Option<String>,
    after: String,
) -> AppResult<SourcePage> {
    library_call(Arc::clone(state.inner()), move |db| {
        crate::importing::source_nodes(db, source_id.as_deref(), parent_id.as_deref(), &after)
    })
    .await
}

#[tauri::command]
pub async fn query_reports(
    state: tauri::State<'_, Arc<AppState>>,
    query: ReportQuery,
    cursor: Option<ReportCursor>,
    limit: u32,
) -> AppResult<ReportPage> {
    library_call(Arc::clone(state.inner()), move |db| {
        crate::services::reports::query(db, &query, cursor.as_ref(), limit)
    })
    .await
}

#[tauri::command]
pub async fn get_dashboard(
    state: tauri::State<'_, Arc<AppState>>,
    today: TimeWindow,
) -> AppResult<Dashboard> {
    library_call(Arc::clone(state.inner()), move |db| {
        crate::services::reports::dashboard(db, today)
    })
    .await
}

#[tauri::command]
pub async fn create_quick_browse(
    state: tauri::State<'_, Arc<AppState>>,
    request: QuickBrowseRequest,
) -> AppResult<BrowseSession> {
    library_call(Arc::clone(state.inner()), move |db| {
        crate::services::quick_browse::create(db, &request)
    })
    .await
}
#[tauri::command]
pub async fn get_quick_browse(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
) -> AppResult<BrowseSession> {
    library_call(Arc::clone(state.inner()), move |db| {
        crate::services::quick_browse::get(db, &session_id)
    })
    .await
}
#[tauri::command]
pub async fn quick_browse_items(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
    after: i64,
    limit: u32,
) -> AppResult<Vec<BrowseItem>> {
    library_call(Arc::clone(state.inner()), move |db| {
        crate::services::quick_browse::items(db, &session_id, after, limit)
    })
    .await
}
#[tauri::command]
pub async fn select_quick_browse(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
    ordinal: i64,
) -> AppResult<()> {
    library_call(Arc::clone(state.inner()), move |db| {
        crate::services::quick_browse::select(db, &session_id, ordinal)
    })
    .await
}
#[tauri::command]
pub async fn shuffle_quick_browse(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
    request_id: String,
) -> AppResult<BrowseSession> {
    library_call(Arc::clone(state.inner()), move |db| {
        crate::services::quick_browse::reshuffle(db, &session_id, &request_id)
    })
    .await
}
#[tauri::command]
pub async fn close_quick_browse(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
) -> AppResult<()> {
    state.cancel_browse(&session_id)?;
    library_call(Arc::clone(state.inner()), move |db| {
        crate::services::quick_browse::close(db, &session_id)
    })
    .await
}
#[tauri::command]
pub async fn get_quick_browse_settings(
    state: tauri::State<'_, Arc<AppState>>,
) -> AppResult<BrowseSettings> {
    library_call(
        Arc::clone(state.inner()),
        crate::services::quick_browse::settings,
    )
    .await
}
#[tauri::command]
pub async fn save_quick_browse_settings(
    state: tauri::State<'_, Arc<AppState>>,
    preferences: BrowsePreferences,
    session_id: Option<String>,
) -> AppResult<BrowseSettings> {
    library_call(Arc::clone(state.inner()), move |db| {
        crate::services::quick_browse::save_settings(db, &preferences, session_id.as_deref())
    })
    .await
}
#[tauri::command]
pub async fn toggle_quick_browse_tag(
    state: tauri::State<'_, Arc<AppState>>,
    request: ToggleTagRequest,
) -> AppResult<ToggleTagResult> {
    library_call(Arc::clone(state.inner()), move |db| {
        crate::services::tag_toggle::toggle_for_quick_browse(db, &request)
    })
    .await
}
#[tauri::command]
pub async fn open_quick_browse_item(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: String,
    ordinal: i64,
) -> AppResult<ViewerOpen> {
    if ordinal < 0 {
        return Err(AppError::Validation("invalid browse ordinal".into()));
    }
    viewer_call(Arc::clone(state.inner()), move |service, db| {
        let session = crate::services::quick_browse::get(db, &session_id)?;
        if session.current_ordinal != Some(ordinal) {
            return Err(AppError::StaleState);
        }
        let item = crate::services::quick_browse::items(db, &session_id, ordinal - 1, 1)?
            .into_iter()
            .next()
            .filter(|item| item.ordinal == ordinal && item.available)
            .ok_or(AppError::NotFound)?;
        service.open_version(db, &item.media_id, Some(item.content_version_id))
    })
    .await
}
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
    catalog_call(
        Arc::clone(state.inner()),
        matches!(filter.sort, LibrarySort::Random),
        move |db| {
            db.repositories()
                .library()
                .query(&filter, cursor.as_ref(), limit)
        },
    )
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
    anchor: Option<String>,
) -> AppResult<ViewerNavigation> {
    catalog_call(
        Arc::clone(state.inner()),
        matches!(filter.sort, LibrarySort::Random),
        move |db| {
            db.repositories()
                .library()
                .navigation(&filter, &id, generation, anchor.as_deref())
        },
    )
    .await
}
async fn catalog_call<T: Send + 'static>(
    state: Arc<AppState>,
    random: bool,
    work: impl FnOnce(&mut crate::db::Database) -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(move || state.catalog(random, work))
        .await
        .map_err(|_| AppError::Internal)?
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
