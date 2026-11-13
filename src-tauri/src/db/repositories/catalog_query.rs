use crate::{
    domain::library_query::*,
    error::{AppError, AppResult},
};
use rusqlite::{
    params_from_iter,
    types::{Value, ValueRef},
    Row,
};
use std::collections::BTreeSet;

pub(super) const MAX_FILTER_TAGS: usize = 20;

pub(super) struct QueryPlan {
    from: String,
    pub key: &'static str,
    id: &'static str,
    ascending: bool,
    text_key: bool,
    predicates: Vec<String>,
    values: Vec<Value>,
}

fn bind(values: &mut Vec<Value>, value: impl Into<Value>) -> String {
    values.push(value.into());
    format!("?{}", values.len())
}

fn system_members(seed_condition: &str) -> String {
    format!(
        "EXISTS (SELECT 1 FROM media_system_tags mt WHERE mt.media_id=m.id AND mt.tag_id IN (
        WITH RECURSIVE subtree(id) AS (
            SELECT id FROM system_tags WHERE {seed_condition}
            UNION SELECT t.id FROM system_tags t JOIN subtree s ON t.parent_id=s.id
        ) SELECT id FROM subtree))"
    )
}

impl QueryPlan {
    pub fn snapshot_statement(&self) -> (String, Vec<Value>) {
        let direction = if self.ascending { "ASC" } else { "DESC" };
        let sql = format!(
            "SELECT m.id,m.current_content_version_id FROM {} WHERE {} ORDER BY {} {direction},{} {direction}",
            self.from, self.predicates.join(" AND "), self.key, self.id,
        );
        (sql, self.values.clone())
    }
    pub fn new(filter: &LibraryFilter) -> AppResult<Self> {
        use LibrarySort::*;
        let (key, index, ascending, text_key) = match filter.sort {
            Recent | ImportedAsc => (
                "m.imported_at",
                if filter.media_type.is_some() {
                    "media_library_type_recent"
                } else {
                    "media_library_recent"
                },
                matches!(filter.sort, ImportedAsc),
                false,
            ),
            Name | NameDesc => (
                "m.filename_sort",
                "media_library_name",
                matches!(filter.sort, Name),
                true,
            ),
            Likes | LikesAsc => (
                "m.like_count",
                "media_library_likes",
                matches!(filter.sort, LikesAsc),
                false,
            ),
            Views | ViewsAsc => (
                "m.view_count",
                "media_library_views",
                matches!(filter.sort, ViewsAsc),
                false,
            ),
            Plays | PlaysAsc => (
                "m.play_count",
                "media_library_plays",
                matches!(filter.sort, PlaysAsc),
                false,
            ),
            Created | CreatedAsc => (
                "m.created_sort",
                "media_library_created",
                matches!(filter.sort, CreatedAsc),
                false,
            ),
            RecentlyViewed => ("m.viewed_sort", "media_library_viewed", false, false),
            RecentlyPlayed => ("m.played_sort", "media_library_played", false, false),
            Random => ("r.sort_key", "", true, true),
        };
        let random = matches!(filter.sort, Random);
        let from = if random {
            "temp.library_random_order r INDEXED BY library_random_rank CROSS JOIN media m ON m.id=r.media_id".into()
        } else {
            format!("media m INDEXED BY {index}")
        };
        let mut plan = Self {
            from,
            key,
            id: if random { "r.media_id" } else { "m.id" },
            ascending,
            text_key,
            predicates: vec!["m.state='active'".into()],
            values: Vec::new(),
        };
        if filter.search.chars().count() > 200 || filter.custom_tag_ids.len() > MAX_FILTER_TAGS {
            return Err(AppError::Validation(
                "library filter exceeds its limits".into(),
            ));
        }
        if let Some(kind) = &filter.media_type {
            if !matches!(kind.as_str(), "image" | "video") {
                return Err(AppError::Validation("invalid media type".into()));
            }
            let value = bind(&mut plan.values, kind.clone());
            plan.predicates.push(format!("m.media_type={value}"));
        }
        if let Some(tag) = &filter.system_tag_id {
            validate_tag_id(tag)?;
            let value = bind(&mut plan.values, tag.clone());
            plan.predicates.push(system_members(&format!("id={value}")));
        }
        plan.add_custom_tags(filter)?;
        plan.add_search(filter);
        for (column, range, count) in [
            ("m.like_count", filter.likes, true),
            ("m.view_count", filter.views, true),
            ("m.play_count", filter.plays, true),
            ("m.imported_at", filter.imported, false),
        ] {
            if let Some(range) = range {
                plan.add_range(column, range, count)?;
            }
        }
        match filter.viewed {
            ViewedStatus::All => (),
            ViewedStatus::Viewed => plan.predicates.push("m.view_count>0".into()),
            ViewedStatus::Unviewed => plan.predicates.push("m.view_count=0".into()),
        }
        Ok(plan)
    }

    fn add_custom_tags(&mut self, filter: &LibraryFilter) -> AppResult<()> {
        let ids: BTreeSet<&String> = filter
            .custom_tag_ids
            .iter()
            .chain(filter.custom_tag_id.iter())
            .collect();
        if ids.len() > MAX_FILTER_TAGS {
            return Err(AppError::Validation("too many custom tag filters".into()));
        }
        let mut values = Vec::with_capacity(ids.len());
        for id in ids {
            validate_tag_id(id)?;
            values.push(bind(&mut self.values, id.clone()));
        }
        if values.is_empty() {
            return Ok(());
        }
        let link = "SELECT 1 FROM media_custom_tags mt WHERE mt.media_id=m.id";
        match filter.custom_tag_mode {
            TagMode::And => self.predicates.extend(
                values
                    .into_iter()
                    .map(|value| format!("EXISTS ({link} AND mt.tag_id={value})")),
            ),
            TagMode::Or => self.predicates.push(format!(
                "EXISTS ({link} AND mt.tag_id IN ({}))",
                values.join(",")
            )),
        }
        Ok(())
    }

    fn add_search(&mut self, filter: &LibraryFilter) {
        if filter.search.is_empty() {
            return;
        }
        let term = bind(&mut self.values, filter.search.clone());
        let mut fields = Vec::new();
        if matches!(
            filter.search_scope,
            SearchScope::All | SearchScope::Filename
        ) {
            fields.push(format!("instr(m.filename_sort,lower({term}))>0"));
        }
        if matches!(
            filter.search_scope,
            SearchScope::All | SearchScope::SystemTag
        ) {
            fields.push(system_members(&format!(
                "instr(lower(name),lower({term}))>0"
            )));
        }
        if matches!(
            filter.search_scope,
            SearchScope::All | SearchScope::CustomTag
        ) {
            fields.push(format!("EXISTS (SELECT 1 FROM media_custom_tags mt WHERE mt.media_id=m.id
                AND mt.tag_id IN (SELECT id FROM custom_tags WHERE instr(lower(name),lower({term}))>0))"));
        }
        self.predicates.push(format!("({})", fields.join(" OR ")));
    }

    fn add_range(&mut self, column: &str, range: InclusiveRange, count: bool) -> AppResult<()> {
        if range.min.zip(range.max).is_some_and(|(min, max)| min > max)
            || (count
                && range
                    .min
                    .into_iter()
                    .chain(range.max)
                    .any(|value| value < 0))
        {
            return Err(AppError::Validation("invalid library range".into()));
        }
        for (operator, bound) in [(">=", range.min), ("<=", range.max)] {
            if let Some(bound) = bound {
                let value = bind(&mut self.values, bound);
                self.predicates.push(format!("{column}{operator}{value}"));
            }
        }
        Ok(())
    }

    pub fn statement(
        &self,
        fields: &str,
        cursor: Option<&LibraryCursor>,
        limit: u32,
        reverse: bool,
    ) -> AppResult<(String, Vec<Value>)> {
        let mut values = self.values.clone();
        let mut predicates = self.predicates.clone();
        let ascending = self.ascending != reverse;
        let direction = if ascending { "ASC" } else { "DESC" };
        if let Some(cursor) = cursor {
            let key = if self.text_key {
                if self.key == "r.sort_key" && !super::random_order::is_hex(&cursor.key, 64) {
                    return Err(AppError::StaleCursor);
                }
                Value::Text(cursor.key.clone())
            } else {
                Value::Integer(cursor.key.parse().map_err(|_| AppError::StaleCursor)?)
            };
            let key = bind(&mut values, key);
            let id = bind(&mut values, cursor.id.clone());
            let operator = if ascending { ">" } else { "<" };
            predicates.push(format!("({},{}){operator}({key},{id})", self.key, self.id));
            predicates.push(format!("{}<>{id}", self.id));
        }
        let limit = bind(&mut values, i64::from(limit));
        let sql = format!("SELECT {fields},{} AS page_sort_key FROM {} WHERE {} ORDER BY {} {direction},{} {direction} LIMIT {limit}", self.key, self.from, predicates.join(" AND "), self.key, self.id);
        Ok((sql, values))
    }

    pub fn anchor_key(&self, tx: &rusqlite::Transaction<'_>, id: &str) -> AppResult<String> {
        use rusqlite::OptionalExtension;
        let from = if self.key == "r.sort_key" {
            "media m JOIN temp.library_random_order r ON r.media_id=m.id"
        } else {
            "media m"
        };
        let sql = format!(
            "SELECT {} AS page_sort_key FROM {from} WHERE m.id=?1 AND m.state='active'",
            self.key
        );
        tx.query_row(&sql, params_from_iter([id]), row_key)
            .optional()?
            .ok_or(AppError::NotFound)
    }
}

pub(super) fn row_key(row: &Row<'_>) -> rusqlite::Result<String> {
    match row.get_ref("page_sort_key")? {
        ValueRef::Integer(value) => Ok(value.to_string()),
        _ => row.get("page_sort_key"),
    }
}

fn validate_tag_id(id: &str) -> AppResult<()> {
    if id.trim().is_empty() || id.len() > 256 {
        return Err(AppError::Validation("invalid tag filter identity".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn filtered_keysets_keep_media_seeks_and_relationship_lookups_indexed() {
        let directory = tempfile::tempdir().unwrap();
        let mut c = Connection::open_in_memory().unwrap();
        crate::db::migrations::migrate(&mut c, directory.path()).unwrap();
        let base = LibraryFilter::default();
        for (name, filter) in [
            (
                "filename",
                LibraryFilter {
                    search: "rare".into(),
                    search_scope: SearchScope::Filename,
                    ..base.clone()
                },
            ),
            (
                "system",
                LibraryFilter {
                    system_tag_id: Some("root".into()),
                    ..base.clone()
                },
            ),
            (
                "customAnd",
                LibraryFilter {
                    custom_tag_ids: vec!["a".into(), "b".into()],
                    ..base.clone()
                },
            ),
            (
                "customOr",
                LibraryFilter {
                    custom_tag_ids: vec!["a".into(), "b".into()],
                    custom_tag_mode: TagMode::Or,
                    ..base.clone()
                },
            ),
            (
                "tagSearch",
                LibraryFilter {
                    search: "Travel".into(),
                    search_scope: SearchScope::CustomTag,
                    ..base.clone()
                },
            ),
            (
                "combined",
                LibraryFilter {
                    search: "rare".into(),
                    system_tag_id: Some("root".into()),
                    custom_tag_ids: vec!["a".into()],
                    media_type: Some("video".into()),
                    ..base.clone()
                },
            ),
        ] {
            let plan = QueryPlan::new(&filter).unwrap();
            let cursor = LibraryCursor {
                generation: 0,
                signature: String::new(),
                id: "m050000".into(),
                key: "50000".into(),
            };
            let (sql, values) = plan.statement("m.id", Some(&cursor), 100, false).unwrap();
            let lines = c
                .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                .unwrap()
                .query_map(params_from_iter(values), |r| r.get::<_, String>(3))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert!(
                lines.iter().any(|p| p.contains("SEARCH m USING")
                    && p.contains("imported_at")
                    && p.contains('<')),
                "{name}: {lines:?}"
            );
            assert!(
                !lines.iter().any(|p| p.contains("SCAN m")
                    || p.contains("SCAN mt")
                    || p.contains("TEMP B-TREE FOR ORDER BY")),
                "{name}: {lines:?}"
            );
            println!("FILTER_PLAN {name}: {lines:?}");
        }
    }

    #[test]
    fn production_keysets_seek_indexes_without_a_temporary_sort() {
        use LibrarySort::*;
        let directory = tempfile::tempdir().unwrap();
        let mut connection = Connection::open_in_memory().unwrap();
        crate::db::migrations::migrate(&mut connection, directory.path()).unwrap();
        let random = RandomOrder {
            seed: "0".repeat(32),
            version: 1,
        };
        crate::db::repositories::random_order::initialize(&connection, Some(&random)).unwrap();

        for (sort, index, media_type) in [
            (Recent, "media_library_recent", None),
            (ImportedAsc, "media_library_recent", None),
            (Recent, "media_library_type_recent", Some("image")),
            (Name, "media_library_name", None),
            (NameDesc, "media_library_name", None),
            (Likes, "media_library_likes", None),
            (LikesAsc, "media_library_likes", None),
            (Views, "media_library_views", None),
            (ViewsAsc, "media_library_views", None),
            (Plays, "media_library_plays", None),
            (PlaysAsc, "media_library_plays", None),
            (Created, "media_library_created", None),
            (CreatedAsc, "media_library_created", None),
            (RecentlyViewed, "media_library_viewed", None),
            (RecentlyPlayed, "media_library_played", None),
            (Random, "library_random_rank", None),
        ] {
            let plan = QueryPlan::new(&LibraryFilter {
                sort,
                media_type: media_type.map(str::to_owned),
                ..Default::default()
            })
            .unwrap();
            let cursor = LibraryCursor {
                generation: 0,
                signature: String::new(),
                id: "synthetic".into(),
                key: if matches!(sort, Random) {
                    "0".repeat(64)
                } else {
                    "0".into()
                },
            };
            for reverse in [false, true] {
                let (sql, values) = plan.statement("m.id", Some(&cursor), 100, reverse).unwrap();
                let lines = connection
                    .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                    .unwrap()
                    .query_map(params_from_iter(values), |row| row.get::<_, String>(3))
                    .unwrap()
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap();
                assert!(
                    lines
                        .iter()
                        .any(|line| line.contains("SEARCH") && line.contains(index)),
                    "{sort:?}: {lines:?}"
                );
                assert!(
                    !lines.iter().any(|line| line.contains("TEMP B-TREE")),
                    "{sort:?}: {lines:?}"
                );
            }
        }
    }
}
