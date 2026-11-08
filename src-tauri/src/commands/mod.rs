use crate::domain::library_query::*;
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
