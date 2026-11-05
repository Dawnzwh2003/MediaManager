use super::{changed, nonempty, page_size};
use crate::{
    domain::library::{QuickBrowseItem, QuickBrowseSession},
    error::AppResult,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

/// Persists a caller-supplied snapshot. Does not select, shuffle or activate media.
pub struct QuickBrowseRepository<'a> {
    pub(crate) connection: &'a mut Connection,
}

impl QuickBrowseRepository<'_> {
    pub fn create(
        &mut self,
        id: &str,
        sort_mode: &str,
        seed: Option<&str>,
        filter: &Value,
        created_at: i64,
    ) -> AppResult<()> {
        nonempty(id)?;
        self.connection.execute(
            "INSERT INTO quick_browse_sessions(id,sort_mode,seed,filter_json,created_at) VALUES(?1,?2,?3,?4,?5)",
            params![id,sort_mode,seed,filter.to_string(),created_at],
        )?;
        Ok(())
    }
    pub fn get(&self, id: &str) -> AppResult<Option<QuickBrowseSession>> {
        Ok(self.connection.query_row(
            "SELECT id,seed,sort_mode,algorithm_version,filter_json,state,item_count,current_ordinal,created_at FROM quick_browse_sessions WHERE id=?1", [id],
            |r| Ok(QuickBrowseSession {
                id: r.get(0)?, seed: r.get(1)?, sort_mode: r.get(2)?, algorithm_version: r.get(3)?, filter_json: r.get(4)?,
                state: r.get(5)?, item_count: r.get(6)?, current_ordinal: r.get(7)?, created_at: r.get(8)?,
            }),
        ).optional()?)
    }
    pub fn add_items(&mut self, id: &str, items: &[QuickBrowseItem]) -> AppResult<()> {
        let transaction = self.connection.transaction()?;
        for item in items {
            transaction.execute("INSERT INTO quick_browse_session_items(session_id,ordinal,media_id,content_version_id) VALUES(?1,?2,?3,?4)",
                params![id,item.ordinal,item.media_id,item.content_version_id])?;
        }
        transaction.commit()?;
        Ok(())
    }
    pub fn publish(&mut self, id: &str) -> AppResult<()> {
        changed(self.connection.execute(
            "UPDATE quick_browse_sessions SET item_count=(SELECT count(*) FROM quick_browse_session_items WHERE session_id=?1),state='ready' WHERE id=?1 AND state='building'", [id],
        )?)
    }
    pub fn items(
        &self,
        id: &str,
        after_ordinal: i64,
        limit: u32,
    ) -> AppResult<Vec<QuickBrowseItem>> {
        let mut statement = self.connection.prepare("SELECT ordinal,media_id,content_version_id FROM quick_browse_session_items WHERE session_id=?1 AND ordinal>?2 ORDER BY ordinal LIMIT ?3")?;
        let rows = statement.query_map(params![id, after_ordinal, page_size(limit)?], |r| {
            Ok(QuickBrowseItem {
                ordinal: r.get(0)?,
                media_id: r.get(1)?,
                content_version_id: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
    pub fn delete(&mut self, id: &str) -> AppResult<()> {
        changed(
            self.connection
                .execute("DELETE FROM quick_browse_sessions WHERE id=?1", [id])?,
        )
    }
}
