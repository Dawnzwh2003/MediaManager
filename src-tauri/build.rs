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
        ]),
    ))
    .expect("failed to build the Tauri application manifest");
}
