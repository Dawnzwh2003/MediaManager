use crate::{
    domain::library::QuickBrowseItem,
    error::{AppError, AppResult},
};
use rusqlite::{params, Connection, OpenFlags};
use std::path::Path;

pub(crate) struct BrowseSpool(Connection);
impl BrowseSpool {
    pub fn create(path: &Path, quota: u64) -> AppResult<Self> {
        let connection = Connection::open(path)?;
        connection.pragma_update(None, "page_size", 4096)?;
        let pages = i64::try_from((quota / 4096).max(1))
            .map_err(|_| AppError::Validation("invalid spool quota".into()))?;
        connection.pragma_update(None, "max_page_count", pages)?;
        connection.execute_batch("PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF;
            PRAGMA cache_size=-4096;
            CREATE TABLE snapshot(ordinal INTEGER PRIMARY KEY,media_id TEXT NOT NULL,content_version_id INTEGER NOT NULL) STRICT;")?;
        Ok(Self(connection))
    }
    pub fn write(
        &mut self,
        work: impl FnOnce(&rusqlite::Transaction<'_>) -> AppResult<(i64, i64)>,
    ) -> AppResult<(i64, i64)> {
        let transaction = self.0.transaction()?;
        let result = work(&transaction)?;
        transaction.commit()?;
        Ok(result)
    }
    pub fn reopen(path: &Path) -> AppResult<Self> {
        Ok(Self(Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?))
    }
    pub fn page(&self, after: i64) -> AppResult<Vec<QuickBrowseItem>> {
        let mut statement = self.0.prepare("SELECT ordinal,media_id,content_version_id FROM snapshot WHERE ordinal>?1 ORDER BY ordinal LIMIT 1000")?;
        let rows = statement.query_map([after], |row| {
            Ok(QuickBrowseItem {
                ordinal: row.get(0)?,
                media_id: row.get(1)?,
                content_version_id: row.get(2)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
    pub fn insert(
        statement: &mut rusqlite::Statement<'_>,
        item: &QuickBrowseItem,
    ) -> AppResult<()> {
        statement.execute(params![
            item.ordinal,
            item.media_id,
            item.content_version_id
        ])?;
        Ok(())
    }
}
