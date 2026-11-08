pub mod db;
pub mod domain;
pub mod error;
pub mod identity;
pub mod metadata;
pub mod platform;
pub mod scanner;
pub mod services;
pub mod thumbnails;

#[cfg(feature = "desktop")]
mod commands;

#[cfg(feature = "desktop")]
pub fn run() -> tauri::Result<()> {
    use std::sync::Arc;
    use tauri::Manager;

    tauri::Builder::default()
        .setup(|app| {
            let root = app.path().app_local_data_dir()?;
            app.manage(Arc::new(services::app_status::AppState::new(root)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_status,
            commands::query_library,
            commands::get_media_details,
            commands::list_library_tags,
            commands::mutate_custom_tag,
            commands::get_thumbnails
        ])
        .run(tauri::generate_context!())
}
