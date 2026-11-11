//! Bounded read models and tag transactions. No filesystem access or event writes.
use crate::{
    domain::library_query::*,
    error::{AppError, AppResult},
};
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};

use super::{
    catalog_query::{row_key, QueryPlan},
    random_order,
};

fn setup_query(connection: &Connection, filter: &LibraryFilter) -> AppResult<QueryPlan> {
    let plan = QueryPlan::new(filter)?;
    if matches!(filter.sort, LibrarySort::Random) {
        random_order::initialize(connection, filter.random.as_ref())?;
    }
    Ok(plan)
}

fn prepare_order(tx: &rusqlite::Transaction<'_>, filter: &LibraryFilter) -> AppResult<()> {
    if matches!(filter.sort, LibrarySort::Random) {
        let order = filter.random.as_ref().ok_or(AppError::Internal)?;
        random_order::populate(tx, order)?;
    }
    Ok(())
}

fn query_signature(filter: &LibraryFilter) -> AppResult<String> {
    let mut digest = Sha256::new();
    digest.update(b"MediaManager/library-query/v8");
    digest.update(serde_json::to_vec(filter)?);
    Ok(format!("{:x}", digest.finalize()))
}

fn query_generation(tx: &rusqlite::Transaction<'_>) -> AppResult<i64> {
    Ok(tx.query_row(
        "SELECT value FROM library_generation WHERE id=1",
        [],
        |row| row.get(0),
    )?)
}

fn read_rows(
    tx: &rusqlite::Transaction<'_>,
    plan: &QueryPlan,
    cursor: Option<&LibraryCursor>,
    limit: u32,
    reverse: bool,
) -> AppResult<Vec<(LibraryMedia, String)>> {
    let (sql, values) = plan.statement(FIELDS, cursor, limit, reverse)?;
    let mut statement = tx.prepare(&sql)?;
    let rows = statement.query_map(rusqlite::params_from_iter(values), |row| {
        Ok((media(row)?, row_key(row)?))
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub struct LibraryRepository<'a> {
    pub(crate) connection: &'a mut Connection,
}
const FIELDS:&str="m.id,m.file_name,m.media_type,m.imported_at,m.like_count,m.width,m.height,m.duration,
 (SELECT coalesce(json_group_array(json_object('id',id,'name',name)),'[]') FROM
 (SELECT t.id,t.name FROM custom_tags t JOIN media_custom_tags mt ON mt.tag_id=t.id WHERE mt.media_id=m.id ORDER BY t.id LIMIT 8)),
 (SELECT count(*) FROM media_custom_tags mt WHERE mt.media_id=m.id)";
fn media(r: &rusqlite::Row<'_>) -> rusqlite::Result<LibraryMedia> {
    let json: String = r.get(8)?;
    Ok(LibraryMedia {
        id: r.get(0)?,
        file_name: r.get(1)?,
        media_type: r.get(2)?,
        imported_at: r.get(3)?,
        like_count: r.get(4)?,
        width: r.get(5)?,
        height: r.get(6)?,
        duration: r.get(7)?,
        custom_tags: serde_json::from_str(&json).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(e))
        })?,
        custom_tag_count: r.get(9)?,
    })
}
impl LibraryRepository<'_> {
    pub fn query(
        &mut self,
        filter: &LibraryFilter,
        cursor: Option<&LibraryCursor>,
        limit: u32,
    ) -> AppResult<LibraryPage> {
        let limit = super::page_size(limit)?;
        let plan = setup_query(self.connection, filter)?;
        let signature = query_signature(filter)?;
        let tx = self.connection.transaction()?;
        let generation = query_generation(&tx)?;
        if cursor
            .is_some_and(|cursor| cursor.generation != generation || cursor.signature != signature)
        {
            return Err(AppError::StaleCursor);
        }
        prepare_order(&tx, filter)?;
        let mut rows = read_rows(&tx, &plan, cursor, limit + 1, false)?;
        let has_more = rows.len() > limit as usize;
        rows.truncate(limit as usize);
        let next_cursor = if has_more {
            rows.last().map(|(media, key)| LibraryCursor {
                generation,
                signature,
                id: media.id.clone(),
                key: key.clone(),
            })
        } else {
            None
        };
        tx.commit()?;
        Ok(LibraryPage {
            items: rows.into_iter().map(|(media, _)| media).collect(),
            next_cursor,
            generation,
        })
    }
    pub fn details(&mut self, id: &str) -> AppResult<LibraryDetails> {
        let tx = self.connection.transaction()?;
        let item = tx
            .query_row(
                &format!("SELECT {FIELDS} FROM media m WHERE m.id=?1 AND m.state='active'"),
                [id],
                media,
            )
            .optional()?
            .ok_or(AppError::NotFound)?;
        let details=tx.query_row("SELECT file_path,file_size,video_codec,audio_codec,container,frame_rate FROM media WHERE id=?1",[id],|r|Ok(LibraryDetails {media:item.clone(),file_path:r.get(0)?,file_size:r.get(1)?,video_codec:r.get(2)?,audio_codec:r.get(3)?,container:r.get(4)?,frame_rate:r.get(5)?}))?;
        tx.commit()?;
        Ok(details)
    }
    pub fn navigation(
        &mut self,
        filter: &LibraryFilter,
        id: &str,
        generation: Option<i64>,
        anchor: Option<&str>,
    ) -> AppResult<crate::domain::viewer::ViewerNavigation> {
        let plan = setup_query(self.connection, filter)?;
        let tx = self.connection.transaction()?;
        let current = query_generation(&tx)?;
        if generation.is_some_and(|value| value != current) {
            return Err(AppError::StaleCursor);
        }
        prepare_order(&tx, filter)?;
        let current_key = plan.anchor_key(&tx, id)?;
        let cursor = LibraryCursor {
            generation: current,
            signature: query_signature(filter)?,
            id: id.into(),
            key: anchor.map(str::to_owned).unwrap_or(current_key),
        };
        let previous = read_rows(&tx, &plan, Some(&cursor), 1, true)?;
        let next = read_rows(&tx, &plan, Some(&cursor), 1, false)?;
        tx.commit()?;
        Ok(crate::domain::viewer::ViewerNavigation {
            previous_id: previous.into_iter().next().map(|(media, _)| media.id),
            next_id: next.into_iter().next().map(|(media, _)| media.id),
            generation: current,
            key: cursor.key,
        })
    }
    pub fn tags(&self, q: &TagQuery) -> AppResult<TagPage> {
        let limit = super::page_size(q.limit)?;
        if q.search.chars().count() > 128 {
            return Err(AppError::Validation("tag search too long".into()));
        }
        let sql=match q.kind.as_str() {
            "system"=>"SELECT t.id,t.name,t.parent_id,EXISTS(SELECT 1 FROM system_tags c WHERE c.parent_id=t.id),0 FROM system_tags t WHERE t.parent_id IS ?1 AND t.id>coalesce(?2,'') AND (?3='' OR instr(lower(t.name),lower(?3))>0) AND (?4 IS NULL OR ?4 IS NOT NULL) ORDER BY t.id LIMIT ?5",
            "custom"=>"SELECT t.id,t.name,NULL,0,EXISTS(SELECT 1 FROM media_custom_tags mt WHERE mt.tag_id=t.id AND mt.media_id=?4) FROM custom_tags t WHERE t.id>coalesce(?2,'') AND (?3='' OR instr(lower(t.name),lower(?3))>0) AND (?1 IS NULL OR ?1 IS NOT NULL) ORDER BY t.id LIMIT ?5",
            _=>return Err(AppError::Validation("unknown tag kind".into())),
        };
        let mut items = self
            .connection
            .prepare(sql)?
            .query_map(
                params![q.parent_id, q.after_id, q.search, q.media_id, limit + 1],
                |r| {
                    Ok(LibraryTag {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        parent_id: r.get(2)?,
                        has_children: r.get(3)?,
                        attached: r.get(4)?,
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        let more = items.len() > limit as usize;
        items.truncate(limit as usize);
        let next_after = if more {
            items.last().map(|t| t.id.clone())
        } else {
            None
        };
        Ok(TagPage { items, next_after })
    }
    pub fn new_tag_id(&self) -> AppResult<String> {
        Ok(self
            .connection
            .query_row("SELECT lower(hex(randomblob(16)))", [], |r| r.get(0))?)
    }
    pub fn batch_tags(
        &mut self,
        media_ids: &[String],
        tag_ids: &[String],
        attach: bool,
    ) -> AppResult<()> {
        if media_ids.is_empty()
            || tag_ids.is_empty()
            || media_ids.len() > 200
            || tag_ids.len() > 10
            || media_ids.len() * tag_ids.len() > 1000
        {
            return Err(AppError::Validation("batch exceeds limits".into()));
        }
        let tx = self.connection.transaction()?;
        for id in media_ids {
            if !tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM media WHERE id=?1 AND state='active')",
                [id],
                |r| r.get::<_, bool>(0),
            )? {
                return Err(AppError::NotFound);
            }
        }
        for id in tag_ids {
            if !tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM custom_tags WHERE id=?1)",
                [id],
                |r| r.get::<_, bool>(0),
            )? {
                return Err(AppError::NotFound);
            }
        }
        for media in media_ids {
            for tag in tag_ids {
                if attach {
                    tx.execute(
                        "INSERT INTO media_custom_tags VALUES(?1,?2) ON CONFLICT DO NOTHING",
                        params![media, tag],
                    )?;
                } else {
                    tx.execute(
                        "DELETE FROM media_custom_tags WHERE media_id=?1 AND tag_id=?2",
                        params![media, tag],
                    )?;
                }
            }
        }
        tx.commit()?;
        Ok(())
    }
}
