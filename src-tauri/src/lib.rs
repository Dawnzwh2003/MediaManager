pub mod db;
pub mod domain;
pub mod error;
pub mod identity;
pub mod importing;
pub mod metadata;
pub mod platform;
pub mod scanner;
pub mod services;
pub mod thumbnails;
pub mod viewer;

#[cfg(feature = "desktop")]
mod commands;

#[cfg(feature = "desktop")]
pub fn run() -> tauri::Result<()> {
    use std::sync::Arc;
    use tauri::Manager;

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let root = app.path().app_local_data_dir()?;
            app.manage(Arc::new(services::app_status::AppState::new(root)));
            Ok(())
        })
        .register_asynchronous_uri_scheme_protocol("media", |context, request, responder| {
            if context.webview_label() != "main" {
                responder.respond(
                    tauri::http::Response::builder()
                        .status(403)
                        .body(Vec::<u8>::new())
                        .unwrap(),
                );
                return;
            }
            let state = Arc::clone(
                context
                    .app_handle()
                    .state::<Arc<services::app_status::AppState>>()
                    .inner(),
            );
            std::mem::drop(tauri::async_runtime::spawn_blocking(move || {
                let reply = state
                    .database()
                    .and_then(|mut db| {
                        state.viewers.read(
                            &mut db,
                            request.uri().path(),
                            request.method().as_str(),
                            request.headers().get("range").and_then(|h| h.to_str().ok()),
                        )
                    })
                    .unwrap_or_else(|error| {
                        viewer::empty(match error {
                            error::AppError::NotFound => 404,
                            error::AppError::MediaUnavailable(_) => 410,
                            _ => 500,
                        })
                    });
                let mut response = tauri::http::Response::builder()
                    .status(reply.status)
                    .header("Content-Type", reply.mime)
                    .header("Content-Length", reply.content_length)
                    .header("Accept-Ranges", "bytes")
                    .header("Cache-Control", "no-store")
                    .header("X-Content-Type-Options", "nosniff")
                    .header("Access-Control-Allow-Origin", "*")
                    .header(
                        "Access-Control-Expose-Headers",
                        "Content-Range,Content-Length,Accept-Ranges",
                    );
                if let Some(range) = reply.content_range {
                    response = response.header("Content-Range", range);
                }
                responder.respond(response.body(reply.body).unwrap());
            }));
        })
        .on_page_load(|webview, payload| {
            if webview.label() == "main"
                && matches!(payload.event(), tauri::webview::PageLoadEvent::Started)
            {
                if let Some(state) = webview.try_state::<Arc<services::app_status::AppState>>() {
                    state.viewers.cancel_all();
                }
            }
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                window
                    .state::<Arc<services::app_status::AppState>>()
                    .viewers
                    .cancel_all();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::pick_import_folder,
            commands::get_import_job,
            commands::start_folder_import,
            commands::cancel_folder_import,
            commands::get_import_storage,
            commands::pick_import_storage,
            commands::list_import_sources,
            commands::get_app_status,
            commands::query_reports,
            commands::get_dashboard,
            commands::create_quick_browse,
            commands::get_quick_browse,
            commands::quick_browse_items,
            commands::select_quick_browse,
            commands::shuffle_quick_browse,
            commands::close_quick_browse,
            commands::get_quick_browse_settings,
            commands::save_quick_browse_settings,
            commands::toggle_quick_browse_tag,
            commands::open_quick_browse_item,
            commands::query_library,
            commands::get_media_details,
            commands::list_library_tags,
            commands::mutate_custom_tag,
            commands::get_thumbnails,
            commands::open_viewer,
            commands::close_viewer,
            commands::save_playback_state,
            commands::prepare_playback,
            commands::viewer_navigation,
            commands::get_playback_state,
            commands::add_like,
            commands::get_statistics,
            commands::activate_media,
            commands::open_player_session,
            commands::report_playback,
            commands::close_activity,
            commands::rebuild_statistics,
            commands::statistics_job,
            commands::cancel_statistics_rebuild
        ])
        .run(tauri::generate_context!())
}
