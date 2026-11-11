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
                imported_at: match id {
                    "a" => 100,
                    "b" | "c" => 200,
                    _ => 300,
                },
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

fn ids(page: LibraryPage) -> Vec<String> {
    page.items.into_iter().map(|media| media.id).collect()
}

fn seed_query_facts(paths: &AppPaths) {
    let sql = rusqlite::Connection::open(&paths.database).unwrap();
    sql.execute_batch("UPDATE media SET
        file_name=CASE id WHEN 'a' THEN 'Zebra.JPG' WHEN 'b' THEN 'alpine.PNG' WHEN 'c' THEN 'Clip.mp4' ELSE file_name END,
        like_count=CASE id WHEN 'a' THEN 2 WHEN 'd' THEN 0 ELSE 5 END,
        view_count=CASE id WHEN 'a' THEN 0 WHEN 'c' THEN 2 ELSE 4 END,
        play_count=CASE id WHEN 'b' THEN 1 WHEN 'c' THEN 2 ELSE 0 END,
        created_time=CASE id WHEN 'a' THEN NULL WHEN 'c' THEN 200 ELSE 300 END,
        last_viewed_at=CASE id WHEN 'b' THEN 200 WHEN 'c' THEN 300 WHEN 'd' THEN 400 END,
        last_played_at=CASE id WHEN 'b' THEN 600 WHEN 'c' THEN 500 END;").unwrap();
}

#[test]
fn scoped_search_combines_literal_names_tree_tags_and_custom_tag_modes_with_ranges() {
    let (_temp, paths, mut db) = fixture();
    seed_query_facts(&paths);
    for (id, name, members) in [
        ("travel", "Travel", ["b", "c"]),
        ("night", "Night", ["c", "d"]),
    ] {
        db.repositories().custom_tags().create(id, name, 1).unwrap();
        for media in members {
            db.repositories().custom_tags().attach(media, id).unwrap();
        }
    }
    for (scope, term, expected) in [
        (SearchScope::Filename, "%_", vec!["d"]),
        (SearchScope::Filename, "CLIP", vec!["c"]),
        (SearchScope::SystemTag, "CHILD", vec!["c", "b"]),
        (SearchScope::CustomTag, "TrAvEl", vec!["c", "b"]),
        (SearchScope::All, "child", vec!["c", "b"]),
        (SearchScope::Filename, "child", vec![]),
        (SearchScope::CustomTag, "clip", vec![]),
    ] {
        let filter = LibraryFilter {
            search_scope: scope,
            search: term.into(),
            ..Default::default()
        };
        assert_eq!(ids(query(&mut db, &filter, None, 100)), expected);
    }
    let mut filter = LibraryFilter {
        custom_tag_ids: vec!["travel".into(), "night".into(), "travel".into()],
        ..Default::default()
    };
    assert_eq!(ids(query(&mut db, &filter, None, 100)), ["c"]);
    filter.custom_tag_mode = TagMode::Or;
    assert_eq!(ids(query(&mut db, &filter, None, 100)), ["d", "c", "b"]);
    filter.system_tag_id = Some("root".into());
    assert_eq!(ids(query(&mut db, &filter, None, 100)), ["c", "b"]);
    filter.media_type = Some("video".into());
    filter.likes = Some(InclusiveRange {
        min: Some(5),
        max: Some(5),
    });
    filter.views = Some(InclusiveRange {
        min: Some(2),
        max: Some(4),
    });
    filter.plays = Some(InclusiveRange {
        min: Some(2),
        max: None,
    });
    filter.imported = Some(InclusiveRange {
        min: Some(200),
        max: Some(200),
    });
    filter.viewed = ViewedStatus::Viewed;
    assert_eq!(ids(query(&mut db, &filter, None, 100)), ["c"]);
    assert_eq!(
        ids(query(
            &mut db,
            &LibraryFilter {
                viewed: ViewedStatus::Unviewed,
                ..Default::default()
            },
            None,
            100
        )),
        ["a"]
    );
    filter.plays = Some(InclusiveRange {
        min: Some(5),
        max: Some(2),
    });
    assert!(db
        .repositories()
        .library()
        .query(&filter, None, 100)
        .is_err());
}

#[test]
fn every_regular_sort_seeks_through_ties_and_nulls_and_supports_reverse_navigation() {
    use LibrarySort::*;
    let (_temp, paths, mut db) = fixture();
    seed_query_facts(&paths);
    for (sort, expected) in [
        (Recent, ["d", "c", "b", "a"]),
        (ImportedAsc, ["a", "b", "c", "d"]),
        (Name, ["d", "b", "c", "a"]),
        (NameDesc, ["a", "c", "b", "d"]),
        (Likes, ["c", "b", "a", "d"]),
        (LikesAsc, ["d", "a", "b", "c"]),
        (Views, ["d", "b", "c", "a"]),
        (ViewsAsc, ["a", "c", "b", "d"]),
        (Plays, ["c", "b", "d", "a"]),
        (PlaysAsc, ["a", "d", "b", "c"]),
        (Created, ["d", "b", "c", "a"]),
        (CreatedAsc, ["a", "c", "b", "d"]),
        (RecentlyViewed, ["d", "c", "b", "a"]),
        (RecentlyPlayed, ["b", "c", "d", "a"]),
    ] {
        let filter = LibraryFilter {
            sort,
            ..Default::default()
        };
        let mut cursor = None;
        for id in expected {
            let page = query(&mut db, &filter, cursor.as_ref(), 1);
            cursor = page.next_cursor.clone();
            assert_eq!(ids(page), [id], "{sort:?}");
        }
        assert!(cursor.is_none());
        let nav = db
            .repositories()
            .library()
            .navigation(&filter, expected[1], None, None)
            .unwrap();
        assert_eq!(nav.previous_id.as_deref(), Some(expected[0]));
        assert_eq!(nav.next_id.as_deref(), Some(expected[2]));
    }
    let filter = LibraryFilter {
        sort: RecentlyViewed,
        ..Default::default()
    };
    let before = db
        .repositories()
        .library()
        .navigation(&filter, "c", None, None)
        .unwrap();
    let old_cursor = query(&mut db, &filter, None, 1).next_cursor.unwrap();
    rusqlite::Connection::open(&paths.database)
        .unwrap()
        .execute(
            "UPDATE media SET view_count=view_count+1,last_viewed_at=900 WHERE id='c'",
            [],
        )
        .unwrap();
    assert!(matches!(
        db.repositories()
            .library()
            .query(&filter, Some(&old_cursor), 100),
        Err(AppError::StaleCursor)
    ));
    let after = db
        .repositories()
        .library()
        .navigation(&filter, "c", None, Some(&before.key))
        .unwrap();
    assert_eq!(
        (after.previous_id, after.next_id),
        (before.previous_id, before.next_id)
    );
}

fn random_filter(seed: &str) -> LibraryFilter {
    LibraryFilter {
        sort: LibrarySort::Random,
        random: Some(RandomOrder {
            seed: seed.repeat(32),
            version: 1,
        }),
        ..Default::default()
    }
}

#[test]
fn random_seed_survives_filters_reconnect_and_new_media_without_reordering_survivors() {
    let (_temp, _paths, mut db) = fixture();
    let original = random_filter("0");
    let first = query(&mut db, &original, None, 2);
    let cursor = first.next_cursor.clone().unwrap();
    let full = ids(query(&mut db, &original, None, 100));
    assert_eq!(
        [ids(first), ids(query(&mut db, &original, Some(&cursor), 2))].concat(),
        full
    );
    let mut other = db.background_connection().unwrap();
    assert_eq!(ids(query(&mut other, &original, None, 100)), full);
    let filtered = LibraryFilter {
        media_type: Some("image".into()),
        ..original.clone()
    };
    assert_eq!(
        ids(query(&mut db, &filtered, None, 100)),
        full.iter()
            .filter(|id| *id != "c")
            .cloned()
            .collect::<Vec<_>>()
    );
    let shuffled = random_filter("1");
    assert!(matches!(
        db.repositories()
            .library()
            .query(&shuffled, Some(&cursor), 100),
        Err(AppError::StaleCursor)
    ));
    assert_ne!(ids(query(&mut db, &shuffled, None, 100)), full);
    assert_eq!(ids(query(&mut db, &original, None, 100)), full);
    db.repositories()
        .media()
        .insert(&NewMedia {
            id: "added".into(),
            file_path: "/synthetic/added.png".into(),
            file_name: "Added.png".into(),
            media_type: MediaType::Image,
            fingerprint: None,
            file_size: 1,
            created_time: None,
            modified_time: 1,
            imported_at: 400,
            last_seen_at: 1,
            width: None,
            height: None,
            duration: None,
        })
        .unwrap();
    let expanded = ids(query(&mut db, &original, None, 100));
    assert_eq!(expanded.len(), 5);
    assert_eq!(
        expanded
            .into_iter()
            .filter(|id| id != "added")
            .collect::<Vec<_>>(),
        full
    );
    let mut invalid = random_filter("q");
    assert!(db
        .repositories()
        .library()
        .query(&invalid, None, 100)
        .is_err());
    invalid.random = Some(RandomOrder {
        seed: "0".repeat(32),
        version: 2,
    });
    assert!(db
        .repositories()
        .library()
        .query(&invalid, None, 100)
        .is_err());
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

    let filter = random_filter("0");
    let start = Instant::now();
    let first = query(&mut db, &filter, None, 100);
    let initial_ms = start.elapsed().as_secs_f64() * 1000.0;
    let first_ids = first
        .items
        .iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    let mut seen = vec![false; 100001];
    let mut total = 0;
    let mut warm = Vec::new();
    let mut page = first;
    loop {
        for item in page.items {
            let id: usize = item.id[1..].parse().unwrap();
            assert!(!seen[id], "duplicate random media {id}");
            seen[id] = true;
            total += 1;
        }
        let Some(cursor) = page.next_cursor else {
            break;
        };
        let start = Instant::now();
        page = query(&mut db, &filter, Some(&cursor), 100);
        warm.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    assert_eq!(total, 100000);
    assert_eq!(warm.len(), 999);
    assert_eq!(ids(query(&mut db, &filter, None, 100)), first_ids);
    let deep = *warm.last().unwrap();
    warm.sort_by(f64::total_cmp);
    eprintln!("SCALE Random 100000 media / 1000 pages: build+first={initial_ms:.3}ms p50={:.3}ms p95={:.3}ms last={deep:.3}ms", warm[499], warm[949]);

    sql.execute_batch("BEGIN;
        INSERT INTO system_tags(id,source_root,directory_path,name,last_seen_at) VALUES('root','/fixture','/fixture','Synthetic collection',1);
        INSERT INTO media_system_tags SELECT id,'root' FROM media WHERE imported_at%2=0;
        INSERT INTO custom_tags VALUES('samples','Sample tag',1,1);
        INSERT INTO media_custom_tags SELECT id,'samples' FROM media WHERE imported_at%10=0;
        COMMIT;").unwrap();
    let filter = LibraryFilter {
        search: "sample tag".into(),
        search_scope: SearchScope::All,
        system_tag_id: Some("root".into()),
        custom_tag_ids: vec!["samples".into()],
        media_type: Some("image".into()),
        viewed: ViewedStatus::Unviewed,
        likes: Some(InclusiveRange {
            min: Some(0),
            max: Some(0),
        }),
        imported: Some(InclusiveRange {
            min: Some(40000),
            max: Some(60000),
        }),
        ..Default::default()
    };
    let start = Instant::now();
    let mut page = query(&mut db, &filter, None, 100);
    let first_ms = start.elapsed().as_secs_f64() * 1000.0;
    let mut expected = 60000;
    let mut pages = 1;
    loop {
        for media in page.items {
            assert_eq!(media.imported_at, expected);
            expected -= 10;
        }
        let Some(cursor) = page.next_cursor else {
            break;
        };
        page = query(&mut db, &filter, Some(&cursor), 100);
        pages += 1;
    }
    assert_eq!(expected, 39990);
    assert_eq!(pages, 21);
    eprintln!("SCALE combined search/tag/type/count/date/viewed: 2001 matches, 21 pages, first={first_ms:.3}ms total={:.3}ms", start.elapsed().as_secs_f64() * 1000.0);
}
