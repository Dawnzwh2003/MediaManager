fn main() {
    #[cfg(feature = "desktop")]
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_app_status",
            "query_library",
            "get_media_details",
            "list_library_tags",
            "mutate_custom_tag",
            "get_thumbnails",
            "open_viewer",
            "close_viewer",
            "save_playback_state",
            "prepare_playback",
            "viewer_navigation",
            "get_playback_state",
            "add_like",
            "get_statistics",
            "activate_media",
            "open_player_session",
            "report_playback",
            "close_activity",
            "rebuild_statistics",
            "statistics_job",
            "cancel_statistics_rebuild",
        ]),
    ))
    .expect("failed to build the Tauri application manifest");
}
