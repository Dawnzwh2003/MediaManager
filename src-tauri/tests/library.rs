use media_manager::{
    db::Database,
    domain::{
        library::{MediaType, NewMedia, ObservedDirectory},
        library_query::*,
    },
    error::AppError,
    platform::paths::AppPaths,
    services::library::mutate_tag,
};
use std::{fs, time::Instant};
fn fixture() -> (tempfile::TempDir, AppPaths, Database) {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::prepare(&temp.path().join("app")).unwrap();
    let mut db = Database::open(&paths).unwrap();
    for (id, name, kind) in [
        ("a", "same.png", MediaType::Image),
        ("b", "same.png", MediaType::Image),
        ("c", "clip.mp4", MediaType::Video),
        ("d", "100%_literal.jpg", MediaType::Image),
    ] {
        let file = temp.path().join(id);
        fs::write(&file, b"original sentinel").unwrap();
        db.repositories()
            .media()
            .insert(&NewMedia {
                id: id.into(),
                fingerprint: Some("same-fingerprint".into()),
                file_path: file.to_str().unwrap().into(),
                file_name: name.into(),
                media_type: kind,
                file_size: 17,
                created_time: None,
                modified_time: 1,
                imported_at: 1,
                last_seen_at: 1,
                width: Some(160),
                height: Some(90),
                duration: None,
            })
            .unwrap();
    }
    let root = temp.path().join("source");
    let child = root.join("child");
    let leaf = child.join("leaf");
    fs::create_dir_all(&leaf).unwrap();
    for (id, parent, path) in [
        ("root", None, &root),
        ("child", Some("root"), &child),
        ("leaf", Some("child"), &leaf),
    ] {
        db.repositories()
            .system_tags()
            .record_observed_directory(&ObservedDirectory {
                id: id.into(),
                parent_id: parent.map(str::to_owned),
                source_root: root.to_str().unwrap().into(),
                directory_path: path.to_str().unwrap().into(),
                name: id.into(),
                last_seen_at: 1,
            })
            .unwrap();
    }
    db.repositories()
        .system_tags()
        .replace_observed_media_tags("a", &["root".into()])
        .unwrap();
    db.repositories()
        .system_tags()
        .replace_observed_media_tags("b", &["child".into()])
        .unwrap();
    db.repositories()
        .system_tags()
        .replace_observed_media_tags("c", &["leaf".into()])
        .unwrap();
    (temp, paths, db)
}
fn query(
    db: &mut Database,
    filter: &LibraryFilter,
    cursor: Option<&LibraryCursor>,
    limit: u32,
) -> LibraryPage {
    db.repositories()
        .library()
        .query(filter, cursor, limit)
        .unwrap()
}
fn tag_query(kind: &str, parent: Option<&str>, media: Option<&str>) -> TagQuery {
    TagQuery {
        kind: kind.into(),
        parent_id: parent.map(str::to_owned),
        after_id: None,
        media_id: media.map(str::to_owned),
        search: String::new(),
        limit: 100,
    }
}
#[test]
fn descendant_filter_and_tag_intersection_are_recursive_and_deduplicated() {
    let (_temp, _paths, mut db) = fixture();
    let mut f = LibraryFilter {
        system_tag_id: Some("root".into()),
        ..Default::default()
    };
    assert_eq!(query(&mut db, &f, None, 100).items.len(), 3);
    f.system_tag_id = Some("child".into());
    assert_eq!(query(&mut db, &f, None, 100).items.len(), 2);
    f.media_type = Some("video".into());
    assert_eq!(query(&mut db, &f, None, 100).items[0].id, "c");
    db.repositories()
        .custom_tags()
        .create("tag", "Travel", 1)
        .unwrap();
    db.repositories().custom_tags().attach("b", "tag").unwrap();
    f.media_type = None;
    f.custom_tag_id = Some("tag".into());
    assert_eq!(query(&mut db, &f, None, 100).items[0].id, "b");
    let tree = db
        .repositories()
        .library()
        .tags(&tag_query("system", Some("root"), None))
        .unwrap();
    assert_eq!(tree.items[0].id, "child");
    assert!(tree.items[0].has_children);
    let tags = db
        .repositories()
        .library()
        .tags(&tag_query("custom", None, Some("b")))
        .unwrap();
    assert!(tags.items[0].attached);
    assert!(tags.items[0].parent_id.is_none());
}
#[test]
fn stable_keyset_ties_search_validation_and_generation_invalidation() {
    let (_temp, _paths, mut db) = fixture();
    for sort in [LibrarySort::Recent, LibrarySort::Name, LibrarySort::Likes] {
        let f = LibraryFilter {
            sort,
            ..Default::default()
        };
        let first = query(&mut db, &f, None, 2);
        let second = query(&mut db, &f, first.next_cursor.as_ref(), 2);
        let mut ids = first
            .items
            .iter()
            .chain(second.items.iter())
            .map(|m| m.id.clone())
            .collect::<Vec<_>>();
        ids.sort();
        assert_eq!(ids, vec!["a", "b", "c", "d"]);
        assert!(second.next_cursor.is_none());
    }
    let literal = LibraryFilter {
        search: "%_".into(),
        ..Default::default()
    };
    assert_eq!(query(&mut db, &literal, None, 100).items[0].id, "d");
    let f = LibraryFilter::default();
    let first = query(&mut db, &f, None, 1);
    let cursor = first.next_cursor.unwrap();
    assert!(matches!(
        db.repositories()
            .library()
            .query(&literal, Some(&cursor), 100),
        Err(AppError::StaleCursor)
    ));
    db.repositories()
        .custom_tags()
        .create("new", "New", 1)
        .unwrap();
    assert!(matches!(
        db.repositories().library().query(&f, Some(&cursor), 100),
        Err(AppError::StaleCursor)
    ));
    for limit in [0, 201] {
        assert!(db.repositories().library().query(&f, None, limit).is_err());
    }
    assert!(db
        .repositories()
        .library()
        .query(
            &LibraryFilter {
                media_type: Some("invalid".into()),
                ..Default::default()
            },
            None,
            100
        )
        .is_err());
    assert!(db
        .repositories()
        .library()
        .query(
            &LibraryFilter {
                search: "x".repeat(201),
                ..Default::default()
            },
            None,
            100
        )
        .is_err());
}
#[test]
fn custom_crud_and_batch_are_atomic_and_never_change_files_system_tags_or_events() {
    let (temp, paths, mut db) = fixture();
    let created = mutate_tag(
        &mut db,
        TagMutation::Create {
            name: "  Travel  ".into(),
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(created.name, "Travel");
    let links = vec!["a".into(), "b".into()];
    let tags = vec![created.id.clone()];
    for _ in 0..2 {
        mutate_tag(
            &mut db,
            TagMutation::Attach {
                media_ids: links.clone(),
                tag_ids: tags.clone(),
            },
        )
        .unwrap();
    }
    assert_eq!(
        db.repositories()
            .custom_tags()
            .media_ids(&created.id, None, 100)
            .unwrap()
            .len(),
        2
    );
    assert!(mutate_tag(
        &mut db,
        TagMutation::Attach {
            media_ids: vec!["c".into(), "missing".into()],
            tag_ids: tags.clone()
        }
    )
    .is_err());
    assert_eq!(
        db.repositories()
            .custom_tags()
            .for_media("c", None, 100)
            .unwrap()
            .len(),
        0
    );
    let sql = rusqlite::Connection::open(&paths.database).unwrap();
    sql.execute_batch("CREATE TRIGGER injected_failure BEFORE INSERT ON media_custom_tags WHEN NEW.media_id='c' BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    assert!(mutate_tag(
        &mut db,
        TagMutation::Attach {
            media_ids: vec!["d".into(), "c".into()],
            tag_ids: tags.clone()
        }
    )
    .is_err());
    assert!(db
        .repositories()
        .custom_tags()
        .for_media("d", None, 100)
        .unwrap()
        .is_empty());
    mutate_tag(
        &mut db,
        TagMutation::Rename {
            id: created.id.clone(),
            name: "Journey".into(),
        },
    )
    .unwrap();
    mutate_tag(
        &mut db,
        TagMutation::Detach {
            media_ids: links.clone(),
            tag_ids: tags.clone(),
        },
    )
    .unwrap();
    assert!(db
        .repositories()
        .custom_tags()
        .media_ids(&created.id, None, 100)
        .unwrap()
        .is_empty());
    mutate_tag(
        &mut db,
        TagMutation::Attach {
            media_ids: links,
            tag_ids: tags,
        },
    )
    .unwrap();
    mutate_tag(&mut db, TagMutation::Delete { id: created.id }).unwrap();
    for id in ["a", "b", "c", "d"] {
        assert_eq!(
            fs::read(temp.path().join(id)).unwrap(),
            b"original sentinel"
        );
        let m = db.repositories().media().get(id).unwrap().unwrap();
        assert_eq!((m.like_count, m.view_count, m.play_count), (0, 0, 0));
    }
    assert_eq!(
        db.repositories()
            .system_tags()
            .get("leaf")
            .unwrap()
            .unwrap()
            .parent_id
            .as_deref(),
        Some("child")
    );
    assert_eq!(sql.query_row("SELECT (SELECT count(*) FROM like_events)+(SELECT count(*) FROM view_events)+(SELECT count(*) FROM play_events)",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    assert!(serde_json::from_str::<TagMutation>(
        r#"{"action":"moveSystemTag","id":"leaf","parentId":"root"}"#
    )
    .is_err());
}
#[test]
fn tags_details_limits_missing_and_reopen_recover_without_losing_associations() {
    let (_temp, paths, mut db) = fixture();
    for i in 0..12 {
        let id = format!("tag{i:02}");
        db.repositories()
            .custom_tags()
            .create(&id, &format!("Tag {i}"), 1)
            .unwrap();
        db.repositories().custom_tags().attach("a", &id).unwrap();
    }
    let details = db.repositories().library().details("a").unwrap();
    assert_eq!(details.media.custom_tags.len(), 8);
    assert_eq!(details.media.custom_tag_count, 12);
    let mut q = tag_query("custom", None, Some("a"));
    q.limit = 5;
    let first = db.repositories().library().tags(&q).unwrap();
    assert_eq!(first.items.len(), 5);
    q.after_id = first.next_after;
    let second = db.repositories().library().tags(&q).unwrap();
    assert_eq!(second.items.len(), 5);
    assert_ne!(first.items[0].id, second.items[0].id);
    assert!(mutate_tag(&mut db, TagMutation::Create { name: " ".into() }).is_err());
    assert!(mutate_tag(
        &mut db,
        TagMutation::Create {
            name: "x".repeat(129)
        }
    )
    .is_err());
    assert!(mutate_tag(
        &mut db,
        TagMutation::Attach {
            media_ids: vec!["a".into(); 201],
            tag_ids: vec!["tag00".into()]
        }
    )
    .is_err());
    db.repositories().media().set_missing("b", true).unwrap();
    assert!(db.repositories().library().details("b").is_err());
    assert_eq!(
        query(&mut db, &LibraryFilter::default(), None, 100)
            .items
            .len(),
        3
    );
    drop(db);
    let mut db = Database::open(&paths).unwrap();
    assert_eq!(
        db.repositories()
            .library()
            .details("a")
            .unwrap()
            .media
            .custom_tag_count,
        12
    );
}
#[test]
fn hundred_thousand_media_keyset_pages_have_no_gaps_and_use_the_cursor_index() {
    let temp = tempfile::tempdir().unwrap();
    let paths = AppPaths::prepare(temp.path()).unwrap();
    let mut db = Database::open(&paths).unwrap();
    let sql = rusqlite::Connection::open(&paths.database).unwrap();
    sql.pragma_update(None, "foreign_keys", true).unwrap();
    sql.execute_batch("WITH RECURSIVE numbers(n) AS (VALUES(1) UNION ALL SELECT n+1 FROM numbers WHERE n<100000) INSERT INTO media(id,file_path,file_name,media_type,file_size,modified_time,imported_at,last_seen_at,created_seq) SELECT printf('m%06d',n),printf('/synthetic/%06d.jpg',n),printf('%06d.jpg',n),'image',1,1,n,1,n FROM numbers;").unwrap();
    let filter = LibraryFilter::default();
    let mut cursor = None;
    let mut count = 0;
    let mut timings = Vec::new();
    let mut last = 100001;
    loop {
        let start = Instant::now();
        let page = query(&mut db, &filter, cursor.as_ref(), 100);
        timings.push(start.elapsed().as_secs_f64() * 1000.0);
        for m in page.items {
            let n = m.id[1..].parse::<usize>().unwrap();
            assert_eq!(n, last - 1);
            last = n;
            count += 1;
        }
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(count, 100000);
    let deep = *timings.last().unwrap();
    timings.sort_by(f64::total_cmp);
    eprintln!(
        "SCALE 100000 media / 1000 pages: p50={:.3}ms p95={:.3}ms last={deep:.3}ms",
        timings[500], timings[950]
    );
    let plan=sql.prepare("EXPLAIN QUERY PLAN SELECT id FROM media INDEXED BY media_library_recent WHERE state='active' AND (imported_at,id)<(500,'m000500') ORDER BY imported_at DESC,id DESC LIMIT 100").unwrap().query_map([],|r|r.get::<_,String>(3)).unwrap().collect::<Result<Vec<_>,_>>().unwrap();
    assert!(
        plan.iter()
            .any(|s| s.contains("SEARCH") && s.contains("media_library_recent")),
        "{plan:?}"
    );
}
