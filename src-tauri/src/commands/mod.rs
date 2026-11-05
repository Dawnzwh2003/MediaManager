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
