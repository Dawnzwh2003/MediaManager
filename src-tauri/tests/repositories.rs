use media_manager::{
    db::Database,
    domain::library::{MediaCursor, MediaType, NewMedia, ObservedDirectory, QuickBrowseItem},
    platform::paths::AppPaths,
};
use serde_json::json;

fn database() -> (tempfile::TempDir, Database) {
    let temporary = tempfile::tempdir().unwrap();
    let paths = AppPaths::prepare(temporary.path()).unwrap();
    let database = Database::open(&paths).unwrap();
    (temporary, database)
}

fn input(id: &str) -> NewMedia {
    NewMedia {
        id: id.into(),
        fingerprint: Some("same-fingerprint".into()),
        file_path: format!("/{id}.jpg"),
        file_name: format!("{id}.jpg"),
        media_type: MediaType::Image,
        file_size: 42,
        created_time: None,
        modified_time: 10,
        imported_at: 20,
        last_seen_at: 30,
        width: None,
        height: None,
        duration: None,
    }
}

#[test]
fn custom_tags_many_to_many_rename_delete_and_foreign_keys() {
    let (_temporary, mut database) = database();
    let mut repositories = database.repositories();
    repositories.media().insert(&input("m1")).unwrap();
    repositories.media().insert(&input("m2")).unwrap();
    let mut tags = repositories.custom_tags();
    tags.create("a", "Travel", 1).unwrap();
    tags.create("b", "Family", 1).unwrap();
    assert!(tags.create("c", "travel", 1).is_err());
    tags.attach("m1", "a").unwrap();
    tags.attach("m1", "b").unwrap();
    tags.attach("m2", "a").unwrap();
    tags.attach("m1", "a").unwrap();
    assert_eq!(tags.for_media("m1", None, 100).unwrap().len(), 2);
    assert_eq!(tags.media_ids("a", None, 100).unwrap(), vec!["m1", "m2"]);
    assert!(tags.attach("absent", "a").is_err());
    assert!(tags.attach("m1", "absent").is_err());
    tags.rename("a", "Trips", 2).unwrap();
    assert_eq!(tags.get("a").unwrap().unwrap().name, "Trips");
    tags.delete("a").unwrap();
    assert_eq!(tags.for_media("m1", None, 100).unwrap().len(), 1);
    assert!(tags.for_media("m2", None, 100).unwrap().is_empty());
    assert_eq!(
        repositories.media().get("m1").unwrap().unwrap().file_path,
        "/m1.jpg"
    );
}

#[test]
fn invalid_tag_set_rolls_back_the_previous_associations() {
    let (_temporary, mut database) = database();
    let mut repositories = database.repositories();
    repositories.media().insert(&input("m1")).unwrap();
    let mut tags = repositories.custom_tags();
    tags.create("a", "Travel", 1).unwrap();
    tags.set_for_media("m1", &["a".into()]).unwrap();
    assert!(tags.set_for_media("m1", &["not-there".into()]).is_err());
    assert_eq!(tags.for_media("m1", None, 100).unwrap()[0].id, "a");
    assert!(tags.set_for_media("not-there", &[]).is_err());
}

#[test]
fn media_duplicates_locations_and_keyset_preserve_identity() {
    let (_temporary, mut database) = database();
    let mut repositories = database.repositories();
    let mut media = repositories.media();
    media.insert(&input("m1")).unwrap();
    media.insert(&input("m2")).unwrap();
    let mut duplicate_path = input("m3");
    duplicate_path.file_path = "/m1.jpg".into();
    assert!(media.insert(&duplicate_path).is_err());
    let first = media.list(None, 1).unwrap();
    assert_eq!(first[0].id, "m2");
    let cursor = MediaCursor {
        imported_at: first[0].imported_at,
        id: first[0].id.clone(),
    };
    assert_eq!(media.list(Some(&cursor), 1).unwrap()[0].id, "m1");
    assert!(media.list(None, 201).is_err());
    media
        .update_location("m1", "/renamed.jpg", "renamed.jpg", 50)
        .unwrap();
    let renamed = media.get("m1").unwrap().unwrap();
    assert_eq!(renamed.imported_at, 20);
    assert_eq!(renamed.last_seen_at, 50);
    assert_eq!(renamed.fingerprint.as_deref(), Some("same-fingerprint"));
    media.set_missing("m1", true).unwrap();
    assert!(media.get("m1").unwrap().unwrap().missing);
}

#[test]
fn observed_system_directory_tree_checks_physical_parent_without_scanning() {
    let (temporary, mut database) = database();
    let root = temporary.path().join("observed");
    let root_text = root.to_string_lossy().into_owned();
    let mut repositories = database.repositories();
    let mut tags = repositories.system_tags();
    tags.record_observed_directory(&ObservedDirectory {
        id: "root".into(),
        parent_id: None,
        source_root: root_text.clone(),
        directory_path: root_text.clone(),
        name: "observed".into(),
        last_seen_at: 1,
    })
    .unwrap();
    let mut parent = "root".to_string();
    let mut directory = root.clone();
    for number in 0..32 {
        let id = format!("d{number}");
        directory.push(&id);
        tags.record_observed_directory(&ObservedDirectory {
            id: id.clone(),
            parent_id: Some(parent.clone()),
            source_root: root_text.clone(),
            directory_path: directory.to_string_lossy().into_owned(),
            name: id.clone(),
            last_seen_at: 2,
        })
        .unwrap();
        parent = id;
    }
    assert_eq!(tags.children(Some("root"), None, 100).unwrap()[0].id, "d0");
    let wrong = ObservedDirectory {
        id: "wrong".into(),
        parent_id: Some("root".into()),
        source_root: root_text,
        directory_path: root.join("a").join("b").to_string_lossy().into_owned(),
        name: "b".into(),
        last_seen_at: 3,
    };
    assert!(tags.record_observed_directory(&wrong).is_err());
    let traversal = ObservedDirectory {
        id: "traversal".into(),
        parent_id: Some("root".into()),
        source_root: root.to_string_lossy().into_owned(),
        directory_path: root.join("..").to_string_lossy().into_owned(),
        name: "..".into(),
        last_seen_at: 4,
    };
    assert!(tags.record_observed_directory(&traversal).is_err());
    // Persisting synthetic observations does not create actual directories.
    assert!(!root.exists());
}

#[test]
fn quick_snapshot_storage_has_atomic_items_order_seed_and_cascade() {
    let (_temporary, mut database) = database();
    let mut repositories = database.repositories();
    repositories.media().insert(&input("m1")).unwrap();
    repositories.media().insert(&input("m2")).unwrap();
    let mut sessions = repositories.quick_browse();
    sessions
        .create("s", "random", Some("stable"), &json!({"type":"image"}), 1)
        .unwrap();
    let items = vec![
        QuickBrowseItem {
            ordinal: 0,
            media_id: "m2".into(),
            content_version_id: 1,
        },
        QuickBrowseItem {
            ordinal: 1,
            media_id: "m1".into(),
            content_version_id: 1,
        },
    ];
    let invalid = vec![
        items[0].clone(),
        QuickBrowseItem {
            ordinal: 1,
            media_id: "absent".into(),
            content_version_id: 1,
        },
    ];
    assert!(sessions.add_items("s", &invalid).is_err());
    assert!(sessions.items("s", -1, 100).unwrap().is_empty());
    sessions.add_items("s", &items).unwrap();
    sessions.publish("s").unwrap();
    assert_eq!(sessions.items("s", -1, 100).unwrap(), items);
    assert_eq!(
        sessions.get("s").unwrap().unwrap().seed.as_deref(),
        Some("stable")
    );
    assert!(sessions
        .add_items(
            "s",
            &[QuickBrowseItem {
                ordinal: 2,
                media_id: "m1".into(),
                content_version_id: 1
            }]
        )
        .is_err());
    sessions.delete("s").unwrap();
    assert!(sessions.items("s", -1, 100).unwrap().is_empty());
}

#[test]
fn settings_roundtrip_structured_json_and_delete() {
    let (_temporary, mut database) = database();
    let mut repositories = database.repositories();
    let mut settings = repositories.settings();
    let value = json!({"columns": 4, "types": ["image", "video"]});
    settings.set("layout", &value, 1).unwrap();
    assert_eq!(settings.get("layout").unwrap(), Some(value));
    settings.set("layout", &json!(false), 2).unwrap();
    assert_eq!(settings.get("layout").unwrap(), Some(json!(false)));
    assert!(settings.set(" ", &json!(null), 3).is_err());
    settings.delete("layout").unwrap();
    assert_eq!(settings.get("layout").unwrap(), None);
}

#[test]
fn event_repository_reads_source_rows_without_an_event_service() {
    let (temporary, mut database) = database();
    database
        .repositories()
        .media()
        .insert(&input("m1"))
        .unwrap();
    // Fixture only: no production repository exposes event insertion.
    let connection =
        rusqlite::Connection::open(temporary.path().join("data/library.sqlite3")).unwrap();
    connection
        .execute_batch(
            "PRAGMA foreign_keys=ON; BEGIN;
        UPDATE db_sequence SET value=value+1 WHERE id=1;
        INSERT INTO like_events(id,ingest_seq,request_id,media_id,occurred_at,recorded_at)
        SELECT 'l1',value,'r1','m1',40,41 FROM db_sequence WHERE id=1; COMMIT;",
        )
        .unwrap();
    let repositories = database.repositories();
    let events = repositories.events();
    assert_eq!(events.counts("m1").unwrap().unwrap().likes, 1);
    assert_eq!(events.likes("m1", 0, 100).unwrap()[0].request_id, "r1");
    assert!(events.views("m1", 0, 100).unwrap().is_empty());
    assert!(events.plays("m1", 0, 100).unwrap().is_empty());
    assert!(events.counts("absent").unwrap().is_none());
}
