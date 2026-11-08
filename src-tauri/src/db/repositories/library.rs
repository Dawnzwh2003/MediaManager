//! Bounded read models and tag transactions. No filesystem access or event writes.
use crate::{
    domain::library_query::*,
    error::{AppError, AppResult},
};
use rusqlite::{params, types::Value, Connection, OptionalExtension};
use sha2::{Digest, Sha256};

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
        if filter.search.chars().count() > 200
            || filter
                .media_type
                .as_deref()
                .is_some_and(|s| s != "image" && s != "video")
        {
            return Err(AppError::Validation("invalid library filter".into()));
        }
        let signature = format!("{:x}", Sha256::digest(serde_json::to_vec(filter)?));
        let tx = self.connection.transaction()?;
        let generation =
            tx.query_row("SELECT value FROM library_generation WHERE id=1", [], |r| {
                r.get::<_, i64>(0)
            })?;
        if cursor.is_some_and(|c| c.generation != generation || c.signature != signature) {
            return Err(AppError::StaleCursor);
        }
        let (column, index, direction, op, last) = match filter.sort {
            LibrarySort::Recent => (
                "imported_at",
                "media_library_recent",
                "DESC",
                "<",
                cursor.map(|c| Value::Integer(c.imported_at)),
            ),
            LibrarySort::Name => (
                "file_name",
                "media_library_name",
                "ASC",
                ">",
                cursor.map(|c| Value::Text(c.file_name.clone())),
            ),
            LibrarySort::Likes => (
                "like_count",
                "media_library_likes",
                "DESC",
                "<",
                cursor.map(|c| Value::Integer(c.like_count)),
            ),
        };
        let seek = if cursor.is_some() {
            format!("AND (m.{column},m.id){op}(?5,?6)")
        } else {
            String::new()
        };
        let sql=format!("WITH RECURSIVE subtree(id) AS (SELECT id FROM system_tags WHERE id=?3 UNION SELECT t.id FROM system_tags t JOIN subtree s ON t.parent_id=s.id)
            SELECT {FIELDS} FROM media m INDEXED BY {index} WHERE m.state='active'
            AND (?1='' OR instr(lower(m.file_name),lower(?1))>0)
            AND (?2 IS NULL OR m.media_type=?2)
            AND (?3 IS NULL OR EXISTS(SELECT 1 FROM media_system_tags mt JOIN subtree s ON s.id=mt.tag_id WHERE mt.media_id=m.id))
            AND (?4 IS NULL OR EXISTS(SELECT 1 FROM media_custom_tags mt WHERE mt.media_id=m.id AND mt.tag_id=?4))
            {seek} ORDER BY m.{column} {direction},m.id {direction} LIMIT ?7");
        let mut items = tx
            .prepare(&sql)?
            .query_map(
                params![
                    filter.search,
                    filter.media_type,
                    filter.system_tag_id,
                    filter.custom_tag_id,
                    last.unwrap_or(Value::Null),
                    cursor.map(|c| &c.id),
                    limit + 1
                ],
                media,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        let has_more = items.len() > limit as usize;
        items.truncate(limit as usize);
        let next_cursor = if has_more {
            items.last().map(|m| LibraryCursor {
                generation,
                signature,
                id: m.id.clone(),
                imported_at: m.imported_at,
                file_name: m.file_name.clone(),
                like_count: m.like_count,
            })
        } else {
            None
        };
        tx.commit()?;
        Ok(LibraryPage {
            items,
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
