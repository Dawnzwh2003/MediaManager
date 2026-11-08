pub mod db;
pub mod domain;
pub mod error;
pub mod identity;
pub mod platform;
pub mod scanner;
pub mod services;

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
        .invoke_handler(tauri::generate_handler![commands::get_app_status])
        .run(tauri::generate_context!())
}
