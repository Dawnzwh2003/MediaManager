//! Parameterized storage only; decoding and source/cache validation live in services.
use crate::{
    error::AppResult,
    metadata::{Failure, Metadata},
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivativeStatus {
    pub status: String,
    pub metadata_status: String,
    pub metadata_error: Option<String>,
    pub thumbnail_error: Option<String>,
    pub thumbnail_path: Option<String>,
    pub attempts: i64,
}

pub(crate) struct Item {
    pub id: String,
    pub version: i64,
    pub fingerprint: Option<String>,
    pub path: String,
    pub size: i64,
    pub modified: i64,
    pub video: bool,
    pub stat: Option<String>,
}
pub(crate) struct DerivativeStore<'a>(pub &'a mut Connection);
impl DerivativeStore<'_> {
    pub fn item(&self, id: Option<&str>) -> AppResult<Option<Item>> {
        let (from, filter) = if id.is_some() {
            ("media_derivatives d", "m.state='active' AND m.id=?1")
        } else {
            (
                "media_derivatives d INDEXED BY derivatives_pending",
                "d.status='pending' AND m.state='active' ORDER BY d.media_id LIMIT 1",
            )
        };
        let query=format!("SELECT m.id,m.current_content_version_id,m.fingerprint,m.file_path,m.file_size,m.modified_time,m.media_type,e.stat FROM {from} JOIN media m ON m.id=d.media_id LEFT JOIN scan_evidence e ON e.media_id=m.id WHERE {filter}");
        let map = |r: &rusqlite::Row<'_>| {
            Ok(Item {
                id: r.get(0)?,
                version: r.get(1)?,
                fingerprint: r.get(2)?,
                path: r.get(3)?,
                size: r.get(4)?,
                modified: r.get(5)?,
                video: r.get::<_, String>(6)? == "video",
                stat: r.get(7)?,
            })
        };
        Ok(if let Some(id) = id {
            self.0.query_row(&query, [id], map).optional()?
        } else {
            self.0.query_row(&query, [], map).optional()?
        })
    }
    pub fn status(&self, id: &str) -> AppResult<Option<DerivativeStatus>> {
        Ok(self.0.query_row("SELECT d.status,d.metadata_status,d.metadata_error,d.thumbnail_error,CASE WHEN d.status='ready' THEN m.thumbnail_path END,d.attempts FROM media_derivatives d JOIN media m ON m.id=d.media_id WHERE m.id=?1",[id],|r| Ok(DerivativeStatus {status:r.get(0)?,metadata_status:r.get(1)?,metadata_error:r.get(2)?,thumbnail_error:r.get(3)?,thumbnail_path:r.get(4)?,attempts:r.get(5)?})).optional()?)
    }
    pub fn recover(&self, version: i64) -> AppResult<()> {
        self.0.execute("UPDATE media_derivatives SET status='pending' WHERE status='running' OR generator_version!=?1",[version])?;
        Ok(())
    }
    pub fn queue(&mut self, id: &str) -> AppResult<()> {
        let tx = self.0.transaction()?;
        tx.execute("UPDATE media_derivatives SET status='pending',thumbnail_error=NULL,cache_key=NULL WHERE media_id=?1 AND status!='running'",[id])?;
        tx.execute("UPDATE media SET thumbnail_path=NULL WHERE id=?1 AND EXISTS(SELECT 1 FROM media_derivatives WHERE media_id=?1 AND status='pending')",[id])?;
        tx.commit()?;
        Ok(())
    }
    pub fn claim(&self, item: &Item, key: &str, version: i64) -> AppResult<bool> {
        Ok(self.0.execute("UPDATE media_derivatives SET status='running',cache_key=?2,generator_version=?3,attempts=attempts+1 WHERE media_id=?1 AND status='pending'",params![item.id,key,version])?==1)
    }
    pub fn finish(
        &mut self,
        item: &Item,
        key: &str,
        metadata: &Result<Metadata, Failure>,
        thumbnail: &Result<String, Failure>,
    ) -> AppResult<bool> {
        let tx = self.0.transaction()?;
        let current:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM media m JOIN media_derivatives d ON d.media_id=m.id WHERE m.id=?1 AND m.current_content_version_id=?2 AND m.fingerprint IS ?3 AND m.file_size=?4 AND m.modified_time=?5 AND m.file_path=?6 AND m.state='active' AND d.status='running' AND d.cache_key=?7)",params![item.id,item.version,item.fingerprint,item.size,item.modified,item.path,key],|r|r.get(0))?;
        if !current {
            tx.execute("UPDATE media_derivatives SET status='pending',cache_key=NULL WHERE media_id=?1 AND status='running' AND cache_key=?2",params![item.id,key])?;
            tx.commit()?;
            return Ok(false);
        }
        let cancelled = matches!(thumbnail, Err(Failure::Cancelled));
        tx.execute("UPDATE media_derivatives SET status=?2,metadata_status=?3,metadata_error=?4,thumbnail_error=?5 WHERE media_id=?1",params![item.id,
            if cancelled { "pending" } else if thumbnail.is_ok() { "ready" } else { "failed" },
            if metadata.is_ok() {"ready"} else {"failed"}, metadata.as_ref().err().map(|e|e.code()),thumbnail.as_ref().err().map(|e|e.code())])?;
        if let Ok(m) = metadata {
            tx.execute("UPDATE media SET width=?2,height=?3,duration=?4,video_codec=?5,audio_codec=?6,container=?7,frame_rate=?8 WHERE id=?1",params![item.id,m.width,m.height,m.duration,m.video_codec,m.audio_codec,m.container,m.frame_rate])?;
        }
        tx.execute(
            "UPDATE media SET thumbnail_path=?2 WHERE id=?1",
            params![item.id, thumbnail.as_ref().ok()],
        )?;
        tx.commit()?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let mut c = Connection::open_in_memory().unwrap();
        c.pragma_update(None, "foreign_keys", true).unwrap();
        crate::db::migrations::migrate(&mut c, dir.path()).unwrap();
        c.execute_batch("INSERT INTO media(id,file_path,file_name,media_type,file_size,modified_time,imported_at,last_seen_at,created_seq) VALUES('id','/synthetic.png','synthetic.png','image',10,1,1,1,1); INSERT INTO playback_states VALUES('id',1,3,10,0,1,1);").unwrap();
        (dir, c)
    }
    fn metadata() -> Result<Metadata, Failure> {
        Ok(Metadata {
            width: 160,
            height: 90,
            duration: None,
            video_codec: None,
            audio_codec: None,
            container: Some("png_pipe".into()),
            frame_rate: None,
        })
    }
    #[test]
    fn unavailable_work_leaves_pending_index_and_returns_on_restore() {
        let (_dir, mut c) = fixture();
        c.execute("UPDATE media SET missing=1,state='missing'", [])
            .unwrap();
        assert!(DerivativeStore(&mut c).item(None).unwrap().is_none());
        assert_eq!(
            DerivativeStore(&mut c)
                .status("id")
                .unwrap()
                .unwrap()
                .thumbnail_error
                .as_deref(),
            Some("source_unavailable")
        );
        c.execute("UPDATE media SET missing=0,state='active'", [])
            .unwrap();
        assert!(DerivativeStore(&mut c).item(None).unwrap().is_some());
        assert_eq!(
            DerivativeStore(&mut c)
                .status("id")
                .unwrap()
                .unwrap()
                .status,
            "pending"
        );
    }
    #[test]
    fn stale_worker_cannot_publish_after_content_version_changes_or_location_moves() {
        let (_dir, mut c) = fixture();
        let item = DerivativeStore(&mut c).item(None).unwrap().unwrap();
        assert!(DerivativeStore(&mut c).claim(&item, "old", 1).unwrap());
        c.execute_batch("INSERT INTO media_revisions VALUES('id',2,'new',20,2); UPDATE media SET current_content_version_id=2,fingerprint='new',file_size=20,width=999,height=999,thumbnail_path='old.jpg' WHERE id='id';").unwrap();
        assert!(!DerivativeStore(&mut c)
            .finish(&item, "old", &metadata(), &Ok("stale.jpg".into()))
            .unwrap());
        let (w, path, position): (Option<i64>, Option<String>, f64) = c
            .query_row(
                "SELECT width,thumbnail_path,playback_position FROM media",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!((w, path, position), (None, None, 0.0));
        assert_eq!(
            DerivativeStore(&mut c)
                .status("id")
                .unwrap()
                .unwrap()
                .status,
            "pending"
        );
        let item = DerivativeStore(&mut c).item(None).unwrap().unwrap();
        assert!(DerivativeStore(&mut c).claim(&item, "new", 1).unwrap());
        c.execute("UPDATE media SET file_path='/moved.png' WHERE id='id'", [])
            .unwrap();
        assert!(!DerivativeStore(&mut c)
            .finish(&item, "new", &metadata(), &Ok("stale.jpg".into()))
            .unwrap());
        assert_eq!(
            DerivativeStore(&mut c)
                .status("id")
                .unwrap()
                .unwrap()
                .status,
            "pending"
        );
        assert_eq!(
            c.query_row(
                "SELECT position_seconds FROM playback_states WHERE content_version_id=1",
                [],
                |r| r.get::<_, f64>(0)
            )
            .unwrap(),
            3.0
        );
    }
    #[test]
    fn publication_metadata_status_and_path_roll_back_together_then_recover() {
        let (_dir, mut c) = fixture();
        let item = DerivativeStore(&mut c).item(None).unwrap().unwrap();
        assert!(DerivativeStore(&mut c).claim(&item, "key", 1).unwrap());
        c.execute_batch("CREATE TRIGGER fail_metadata BEFORE UPDATE OF width ON media BEGIN SELECT RAISE(ABORT,'injected failure'); END;").unwrap();
        assert!(DerivativeStore(&mut c)
            .finish(&item, "key", &metadata(), &Ok("cache.jpg".into()))
            .is_err());
        let s = DerivativeStore(&mut c).status("id").unwrap().unwrap();
        assert_eq!(s.status, "running");
        assert!(s.thumbnail_path.is_none());
        assert_eq!(
            c.query_row("SELECT width FROM media", [], |r| r
                .get::<_, Option<i64>>(0))
                .unwrap(),
            None
        );
        c.execute_batch("DROP TRIGGER fail_metadata;").unwrap();
        DerivativeStore(&mut c).recover(1).unwrap();
        let item = DerivativeStore(&mut c).item(None).unwrap().unwrap();
        assert!(DerivativeStore(&mut c).claim(&item, "key", 1).unwrap());
        assert!(DerivativeStore(&mut c)
            .finish(&item, "key", &metadata(), &Ok("cache.jpg".into()))
            .unwrap());
        assert_eq!(
            DerivativeStore(&mut c)
                .status("id")
                .unwrap()
                .unwrap()
                .status,
            "ready"
        );
        let (position, plays): (f64, i64) = c
            .query_row("SELECT playback_position,play_count FROM media", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!((position, plays), (3.0, 0));
    }
}
