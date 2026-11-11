use crate::{
    domain::{library::QuickBrowseItem, library_query::LibraryFilter, quick_browse::*},
    error::{AppError, AppResult},
};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
pub(crate) struct BrowseStore<'a>(pub &'a mut Connection);

impl BrowseStore<'_> {
    pub fn save_preferences(
        &mut self,
        preferences: &BrowsePreferences,
        session: Option<&str>,
        now: i64,
    ) -> AppResult<()> {
        let transaction = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        for binding in &preferences.bindings {
            if !transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM custom_tags WHERE id=?1)",
                [&binding.tag_id],
                |row| row.get::<_, bool>(0),
            )? {
                return Err(AppError::NotFound);
            }
        }
        transaction.execute("INSERT INTO app_settings(key,value_json,updated_at) VALUES('quick_browse.v1',?1,?2) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json,updated_at=excluded.updated_at",params![serde_json::to_string(preferences)?,now])?;
        if let Some(id) = session {
            let changed=transaction.execute("UPDATE quick_browse_sessions SET autoplay=?2,muted=?3 WHERE id=?1 AND state='ready'",params![id,preferences.autoplay,preferences.muted])?;
            if changed != 1 {
                return Err(AppError::StaleState);
            }
        }
        transaction.commit()?;
        Ok(())
    }
    pub fn get(&self, id: &str) -> AppResult<BrowseSession> {
        let mut session=self.0.query_row("SELECT id,started_at,ended_at,query_sort,filter_json,seed,algorithm_version,state,build_phase,item_count,build_count,current_ordinal,autoplay,muted,error,parent_session_id FROM quick_browse_sessions WHERE id=?1",[id],|row|{
            let filter:String=row.get(4)?;
            let session=BrowseSession {id:row.get(0)?,started_at:row.get(1)?,ended_at:row.get(2)?,sort_mode:row.get(3)?,filter:LibraryFilter::default(),random_seed:row.get(5)?,algorithm_version:row.get(6)?,state:row.get(7)?,phase:row.get(8)?,item_count:row.get(9)?,built_count:row.get(10)?,current_ordinal:row.get(11)?,autoplay:row.get(12)?,muted:row.get(13)?,error:row.get(14)?,parent_session_id:row.get(15)?};
            Ok((session,filter))
        }).optional()?.ok_or(AppError::NotFound)?;
        session.0.filter = serde_json::from_str(&session.1)?;
        Ok(session.0)
    }
    pub fn find(&self, id: &str) -> AppResult<bool> {
        Ok(self.0.query_row(
            "SELECT EXISTS(SELECT 1 FROM quick_browse_sessions WHERE id=?1)",
            [id],
            |row| row.get(0),
        )?)
    }
    pub fn original_request(&self, id: &str) -> AppResult<String> {
        Ok(self.0.query_row(
            "SELECT request_json FROM quick_browse_sessions WHERE id=?1",
            [id],
            |row| row.get(0),
        )?)
    }
    pub fn insert(
        &mut self,
        request: &QuickBrowseRequest,
        parent: Option<&str>,
        now: i64,
    ) -> AppResult<()> {
        let transaction = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if transaction.query_row("SELECT EXISTS(SELECT 1 FROM quick_browse_sessions WHERE state='building' AND build_phase!='complete')",[],|row|row.get::<_,bool>(0))? {return Err(AppError::Busy)}
        if let Some(parent) = parent {
            if !transaction.query_row("SELECT EXISTS(SELECT 1 FROM quick_browse_sessions WHERE id=?1 AND state='ready' AND ended_at IS NULL)",[parent],|row|row.get::<_,bool>(0))? {return Err(AppError::StaleState)}
        }
        let sort = serde_json::to_value(request.filter.sort)?;
        let random = matches!(
            request.filter.sort,
            crate::domain::library_query::LibrarySort::Random
        );
        transaction.execute("INSERT INTO quick_browse_sessions(id,sort_mode,seed,filter_json,created_at,started_at,query_sort,autoplay,muted,build_phase,parent_session_id,request_json) VALUES(?1,?2,?3,?4,?5,?5,?6,?7,?8,'pending',?9,?10)",params![request.request_id,if random{"random"}else{"imported"},request.filter.random.as_ref().map(|order|&order.seed),serde_json::to_string(&request.filter)?,now,sort.as_str(),request.autoplay,request.muted,parent,serde_json::to_string(request)?])?;
        transaction.commit()?;
        Ok(())
    }
    pub fn new_seed(&self) -> AppResult<String> {
        Ok(self
            .0
            .query_row("SELECT lower(hex(randomblob(16)))", [], |row| row.get(0))?)
    }
    pub fn next_job(&self) -> AppResult<Option<String>> {
        Ok(self.0.query_row("SELECT id FROM quick_browse_sessions WHERE state='building' AND build_phase IN('pending','cleaning') ORDER BY created_at,id LIMIT 1",[],|row|row.get(0)).optional()?)
    }
    pub fn recover(&self) -> AppResult<()> {
        self.0.execute("UPDATE quick_browse_sessions SET build_phase='cleaning',cancel_requested=1,error='snapshot_interrupted' WHERE state='building' AND build_phase IN('pending','spooling','publishing')",[])?;
        Ok(())
    }
    pub fn cancelled(&self, id: &str) -> AppResult<bool> {
        Ok(self.0.query_row(
            "SELECT cancel_requested OR state!='building' FROM quick_browse_sessions WHERE id=?1",
            [id],
            |row| row.get(0),
        )?)
    }
    pub fn spooling(&self, id: &str) -> AppResult<()> {
        self.0.execute("UPDATE quick_browse_sessions SET build_phase='spooling' WHERE id=?1 AND state='building'",[id])?;
        Ok(())
    }
    pub fn progress(&self, id: &str, count: i64) -> AppResult<()> {
        self.0.execute(
            "UPDATE quick_browse_sessions SET build_count=?2 WHERE id=?1 AND state='building'",
            params![id, count],
        )?;
        Ok(())
    }
    pub fn publishing(&self, id: &str, count: i64, generation: i64) -> AppResult<()> {
        self.0.execute("UPDATE quick_browse_sessions SET build_phase='publishing',build_count=?2,snapshot_generation=?3 WHERE id=?1 AND state='building'",params![id,count,generation])?;
        Ok(())
    }
    pub fn add_batch(&mut self, id: &str, items: &[QuickBrowseItem]) -> AppResult<()> {
        let transaction = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (state, cancel, expected): (String, bool, i64) = transaction.query_row(
            "SELECT state,cancel_requested,published_count FROM quick_browse_sessions WHERE id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        if state != "building" || cancel {
            return Err(AppError::StaleState);
        }
        if items.len() > 1000 || items.first().is_some_and(|item| item.ordinal != expected) {
            return Err(AppError::Validation("invalid snapshot batch".into()));
        }
        for (offset, item) in items.iter().enumerate() {
            if item.ordinal != expected + offset as i64 {
                return Err(AppError::Validation("snapshot ordinal gap".into()));
            }
            transaction.execute(
                "INSERT INTO quick_browse_session_items VALUES(?1,?2,?3,?4)",
                params![id, item.ordinal, item.media_id, item.content_version_id],
            )?;
        }
        transaction.execute(
            "UPDATE quick_browse_sessions SET published_count=published_count+?2 WHERE id=?1",
            params![id, items.len() as i64],
        )?;
        transaction.commit()?;
        Ok(())
    }
    pub fn publish(&mut self, id: &str, now: i64) -> AppResult<()> {
        let transaction = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (count,published,parent,cancel):(i64,i64,Option<String>,bool)=transaction.query_row("SELECT build_count,published_count,parent_session_id,cancel_requested FROM quick_browse_sessions WHERE id=?1 AND state='building'",[id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)))?;
        if cancel || count != published {
            return Err(AppError::StaleState);
        }
        if let Some(parent) = parent {
            let changed=transaction.execute("UPDATE quick_browse_sessions SET state='closed',ended_at=?2 WHERE id=?1 AND state='ready' AND ended_at IS NULL",params![parent,now])?;
            if changed != 1 {
                return Err(AppError::StaleState);
            }
        }
        transaction.execute("UPDATE quick_browse_sessions SET state='ready',item_count=?2,current_ordinal=CASE WHEN ?2>0 THEN 0 END,build_phase='complete' WHERE id=?1",params![id,count])?;
        transaction.commit()?;
        Ok(())
    }
    pub fn close(&self, id: &str, now: i64) -> AppResult<()> {
        self.0.execute("UPDATE quick_browse_sessions SET cancel_requested=1,build_phase=CASE WHEN state='building' THEN build_phase ELSE 'complete' END,ended_at=CASE WHEN state='ready' THEN ?2 ELSE ended_at END,state=CASE WHEN state='ready' THEN 'closed' ELSE state END WHERE id=?1 AND ended_at IS NULL",params![id,now])?;
        Ok(())
    }
    pub fn fail(&self, id: &str, error: &str) -> AppResult<()> {
        self.0.execute("UPDATE quick_browse_sessions SET build_phase='cleaning',error=?2,cancel_requested=1 WHERE id=?1 AND state='building'",params![id,error])?;
        Ok(())
    }
    pub fn cleanup_batch(&mut self, id: &str, now: i64) -> AppResult<bool> {
        let transaction = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute("DELETE FROM quick_browse_session_items WHERE session_id=?1 AND ordinal IN(SELECT ordinal FROM quick_browse_session_items WHERE session_id=?1 LIMIT 1000)",[id])?;
        let remain: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM quick_browse_session_items WHERE session_id=?1)",
            [id],
            |row| row.get(0),
        )?;
        if !remain {
            transaction.execute("UPDATE quick_browse_sessions SET state='closed',ended_at=coalesce(ended_at,?2),build_phase='complete',published_count=0 WHERE id=?1 AND state='building'",params![id,now])?;
        }
        transaction.commit()?;
        Ok(!remain)
    }
    pub fn items(&self, id: &str, after: i64, limit: u32) -> AppResult<Vec<BrowseItem>> {
        let limit = super::page_size(limit)?;
        let mut statement=self.0.prepare("SELECT i.ordinal,i.media_id,i.content_version_id,m.file_name,m.state='active' AND m.current_content_version_id=i.content_version_id FROM quick_browse_session_items i JOIN media m ON m.id=i.media_id JOIN quick_browse_sessions s ON s.id=i.session_id WHERE i.session_id=?1 AND s.state='ready' AND i.ordinal>?2 ORDER BY i.ordinal LIMIT ?3")?;
        let rows = statement.query_map(params![id, after, limit], |row| {
            Ok(BrowseItem {
                ordinal: row.get(0)?,
                media_id: row.get(1)?,
                content_version_id: row.get(2)?,
                file_name: row.get(3)?,
                available: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
    pub fn select(&self, id: &str, ordinal: i64) -> AppResult<()> {
        let changed=self.0.execute("UPDATE quick_browse_sessions SET current_ordinal=?2 WHERE id=?1 AND state='ready' AND EXISTS(SELECT 1 FROM quick_browse_session_items WHERE session_id=?1 AND ordinal=?2)",params![id,ordinal])?;
        if changed != 1 {
            return Err(AppError::NotFound);
        }
        Ok(())
    }
}
