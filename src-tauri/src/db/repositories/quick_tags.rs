use crate::{domain::quick_browse::*, error::AppResult};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

pub(crate) struct QuickTagStore<'a>(pub &'a mut Connection);
pub(crate) struct QuickTagTransaction<'a>(Transaction<'a>);
pub(crate) struct PreviousToggle {
    pub session_id: String,
    pub ordinal: i64,
    pub digit: u8,
    pub result: ToggleTagResult,
}
impl QuickTagStore<'_> {
    pub fn transaction<T>(
        &mut self,
        work: impl FnOnce(&QuickTagTransaction<'_>) -> AppResult<T>,
    ) -> AppResult<T> {
        let transaction = QuickTagTransaction(
            self.0
                .transaction_with_behavior(TransactionBehavior::Immediate)?,
        );
        let result = work(&transaction)?;
        transaction.0.commit()?;
        Ok(result)
    }
}
impl QuickTagTransaction<'_> {
    pub fn previous(&self, id: &str) -> AppResult<Option<PreviousToggle>> {
        Ok(self.0.query_row("SELECT session_id,ordinal,digit,media_id,tag_id,tag_name,attached FROM quick_tag_operations WHERE request_id=?1",[id],|row|Ok(PreviousToggle {
            session_id:row.get(0)?,ordinal:row.get(1)?,digit:row.get(2)?,result:ToggleTagResult {media_id:row.get(3)?,tag_id:row.get(4)?,tag_name:row.get(5)?,attached:row.get(6)?},
        })).optional()?)
    }
    pub fn current_media(&self, request: &ToggleTagRequest) -> AppResult<Option<String>> {
        Ok(self.0.query_row("SELECT m.id FROM quick_browse_sessions s JOIN quick_browse_session_items i ON i.session_id=s.id JOIN media m ON m.id=i.media_id WHERE s.id=?1 AND s.state='ready' AND s.current_ordinal=?2 AND i.ordinal=?2 AND m.state='active' AND m.current_content_version_id=i.content_version_id",params![request.session_id,request.ordinal],|row|row.get(0)).optional()?)
    }
    pub fn preferences(&self) -> AppResult<BrowsePreferences> {
        let json: Option<String> = self
            .0
            .query_row(
                "SELECT value_json FROM app_settings WHERE key='quick_browse.v1'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(json
            .map(|value| serde_json::from_str(&value))
            .transpose()?
            .unwrap_or_default())
    }
    pub fn tag(&self, id: &str) -> AppResult<Option<String>> {
        Ok(self
            .0
            .query_row("SELECT name FROM custom_tags WHERE id=?1", [id], |row| {
                row.get(0)
            })
            .optional()?)
    }
    pub fn attached(&self, media: &str, tag: &str) -> AppResult<bool> {
        Ok(self.0.query_row(
            "SELECT EXISTS(SELECT 1 FROM media_custom_tags WHERE media_id=?1 AND tag_id=?2)",
            params![media, tag],
            |row| row.get(0),
        )?)
    }
    pub fn set_link(&self, media: &str, tag: &str, attached: bool) -> AppResult<()> {
        if attached {
            self.0.execute(
                "INSERT INTO media_custom_tags(media_id,tag_id) VALUES(?1,?2)",
                params![media, tag],
            )?;
        } else {
            self.0.execute(
                "DELETE FROM media_custom_tags WHERE media_id=?1 AND tag_id=?2",
                params![media, tag],
            )?;
        }
        Ok(())
    }
    pub fn receipt(
        &self,
        request: &ToggleTagRequest,
        result: &ToggleTagResult,
        now: i64,
    ) -> AppResult<()> {
        self.0.execute(
            "INSERT INTO quick_tag_operations VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                request.request_id,
                request.session_id,
                request.ordinal,
                request.digit,
                result.media_id,
                result.tag_id,
                result.tag_name,
                result.attached,
                now
            ],
        )?;
        Ok(())
    }
}
