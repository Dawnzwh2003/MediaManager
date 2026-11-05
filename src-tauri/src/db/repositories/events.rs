use super::page_size;
use crate::{
    domain::library::{EventCounts, LikeEvent, PlayEvent, ViewEvent},
    error::AppResult,
};
use rusqlite::{params, Connection, OptionalExtension};

/// Read-only. Qualification, event insertion and rebuild services are outside Phase 2.
pub struct EventRepository<'a> {
    pub(crate) connection: &'a Connection,
}

impl EventRepository<'_> {
    pub fn counts(&self, media_id: &str) -> AppResult<Option<EventCounts>> {
        Ok(self
            .connection
            .query_row(
                "SELECT (SELECT count(*) FROM like_events WHERE media_id=m.id),
                    (SELECT count(*) FROM view_events WHERE media_id=m.id),
                    (SELECT count(*) FROM play_events WHERE media_id=m.id)
             FROM media m WHERE m.id=?1",
                [media_id],
                |r| {
                    Ok(EventCounts {
                        likes: r.get(0)?,
                        views: r.get(1)?,
                        plays: r.get(2)?,
                    })
                },
            )
            .optional()?)
    }
    pub fn likes(&self, media_id: &str, after_seq: i64, limit: u32) -> AppResult<Vec<LikeEvent>> {
        let mut statement = self.connection.prepare("SELECT id,ingest_seq,request_id,media_id,occurred_at,recorded_at,policy_version FROM like_events WHERE media_id=?1 AND ingest_seq>?2 ORDER BY ingest_seq LIMIT ?3")?;
        let rows = statement.query_map(params![media_id, after_seq, page_size(limit)?], |r| {
            Ok(LikeEvent {
                id: r.get(0)?,
                ingest_seq: r.get(1)?,
                request_id: r.get(2)?,
                media_id: r.get(3)?,
                occurred_at: r.get(4)?,
                recorded_at: r.get(5)?,
                policy_version: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
    pub fn views(&self, media_id: &str, after_seq: i64, limit: u32) -> AppResult<Vec<ViewEvent>> {
        let mut statement = self.connection.prepare("SELECT id,ingest_seq,request_id,media_id,activation_id,occurred_at,recorded_at,policy_version FROM view_events WHERE media_id=?1 AND ingest_seq>?2 ORDER BY ingest_seq LIMIT ?3")?;
        let rows = statement.query_map(params![media_id, after_seq, page_size(limit)?], |r| {
            Ok(ViewEvent {
                id: r.get(0)?,
                ingest_seq: r.get(1)?,
                request_id: r.get(2)?,
                media_id: r.get(3)?,
                activation_id: r.get(4)?,
                occurred_at: r.get(5)?,
                recorded_at: r.get(6)?,
                policy_version: r.get(7)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
    pub fn plays(&self, media_id: &str, after_seq: i64, limit: u32) -> AppResult<Vec<PlayEvent>> {
        let mut statement = self.connection.prepare("SELECT id,ingest_seq,request_id,media_id,content_version_id,player_session_id,started_at,qualified_at,ended_at,start_position,end_position,watched_seconds,completed,end_reason,policy_version FROM play_events WHERE media_id=?1 AND ingest_seq>?2 ORDER BY ingest_seq LIMIT ?3")?;
        let rows = statement.query_map(params![media_id, after_seq, page_size(limit)?], |r| {
            Ok(PlayEvent {
                id: r.get(0)?,
                ingest_seq: r.get(1)?,
                request_id: r.get(2)?,
                media_id: r.get(3)?,
                content_version_id: r.get(4)?,
                player_session_id: r.get(5)?,
                started_at: r.get(6)?,
                qualified_at: r.get(7)?,
                ended_at: r.get(8)?,
                start_position: r.get(9)?,
                end_position: r.get(10)?,
                watched_seconds: r.get(11)?,
                completed: r.get(12)?,
                end_reason: r.get(13)?,
                policy_version: r.get(14)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}
