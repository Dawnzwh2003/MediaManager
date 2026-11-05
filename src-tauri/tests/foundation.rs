use std::fs;

use media_manager::{
    db::Database, error::AppError, platform::paths::AppPaths, services::app_status::AppState,
};

#[test]
fn runtime_directories_are_idempotent_and_preserve_files() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("application");
    let first = AppPaths::prepare(&root).unwrap();
    fs::write(first.cache.join("keep.txt"), b"keep").unwrap();
    let second = AppPaths::prepare(&root).unwrap();
    assert!(second.data.is_dir() && second.cache.is_dir() && second.thumbnails.is_dir());
    assert_eq!(fs::read(second.cache.join("keep.txt")).unwrap(), b"keep");
}

#[test]
fn file_in_place_of_directory_is_rejected_without_overwrite() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("cache"), b"not-a-directory").unwrap();
    assert!(matches!(
        AppPaths::prepare(temp.path()),
        Err(AppError::Storage(_))
    ));
    assert_eq!(
        fs::read(temp.path().join("cache")).unwrap(),
        b"not-a-directory"
    );
}

#[test]
fn connection_has_required_pragmas_and_releases_its_instance_lock() {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::prepare(temp.path()).unwrap();
    let first = Database::open(&paths).unwrap();
    let status = first.status().unwrap();
    assert_eq!(
        status.schema_version,
        media_manager::db::migrations::CURRENT_SCHEMA_VERSION
    );
    assert!(status.foreign_keys);
    assert!(status.journal_mode.eq_ignore_ascii_case("wal"));
    assert_eq!(status.busy_timeout_ms, 2_000);
    assert!(matches!(Database::open(&paths), Err(AppError::Busy)));
    drop(first);
    assert!(Database::open(&paths).is_ok());
}

#[test]
fn corrupt_database_is_never_replaced_with_an_empty_one() {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::prepare(temp.path()).unwrap();
    let bytes = b"this is not a SQLite database";
    fs::write(&paths.database, bytes).unwrap();
    assert!(Database::open(&paths).is_err());
    assert_eq!(fs::read(paths.database).unwrap(), bytes);
}

#[test]
fn backend_status_is_serializable_and_retry_initialization_can_recover() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("application");
    fs::write(&root, b"blocked").unwrap();
    let state = AppState::new(root.clone());
    assert!(state.status().is_err());
    fs::remove_file(&root).unwrap();
    let status = state.status().unwrap();
    let json = serde_json::to_value(status).unwrap();
    assert_eq!(json["phase"], 1);
    assert_eq!(
        json["database"]["schemaVersion"],
        media_manager::db::migrations::CURRENT_SCHEMA_VERSION
    );
    assert_eq!(json["storage"]["thumbnailsReady"], true);
    assert_eq!(
        state.status().unwrap().database.schema_version,
        media_manager::db::migrations::CURRENT_SCHEMA_VERSION
    );
}

#[test]
fn ipc_errors_do_not_expose_paths_sql_or_internal_details() {
    let error = AppError::Storage("C:\\private\\media\\secret.jpg".into());
    let json = serde_json::to_string(&error).unwrap();
    assert!(!json.contains("secret.jpg"));
    assert!(json.contains("invalid_storage"));
    assert!(json.contains("retryable"));
}

#[cfg(unix)]
#[test]
fn symlinked_cache_is_rejected() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    symlink(outside.path(), temp.path().join("cache")).unwrap();
    assert!(matches!(
        AppPaths::prepare(temp.path()),
        Err(AppError::Storage(_))
    ));
    assert!(!outside.path().join("thumbnails").exists());
}

#[cfg(unix)]
#[test]
fn symlinked_sqlite_sidecar_is_rejected_without_touching_target() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::prepare(temp.path()).unwrap();
    let target = temp.path().join("keep.txt");
    fs::write(&target, b"untouched").unwrap();
    symlink(&target, paths.data.join("library.sqlite3-wal")).unwrap();
    assert!(matches!(Database::open(&paths), Err(AppError::Storage(_))));
    assert_eq!(fs::read(target).unwrap(), b"untouched");
}
