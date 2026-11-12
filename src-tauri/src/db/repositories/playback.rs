//! Versioned recovery writes only. No filesystem I/O or Event Log access.
use crate::{
    domain::viewer::{PlaybackState, ProgressWrite},
    error::{AppError, AppResult},
};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
pub struct PlaybackRepository<'a> {
    pub(crate) connection: &'a mut Connection,
}
fn state(c: &Connection, id: &str, version: i64) -> AppResult<PlaybackState> {
    Ok(c.query_row("SELECT position_seconds,duration_seconds,state_version FROM playback_states WHERE media_id=?1 AND content_version_id=?2",params![id,version],|r|Ok(PlaybackState {position_seconds:r.get(0)?,duration_seconds:r.get(1)?,state_version:r.get(2)?}))?)
}
impl PlaybackRepository<'_> {
    pub fn get(&self, id: &str, version: i64) -> AppResult<Option<PlaybackState>> {
        Ok(self.connection.query_row("SELECT s.position_seconds,s.duration_seconds,s.state_version FROM playback_states s JOIN media m ON m.id=s.media_id WHERE s.media_id=?1 AND s.content_version_id=?2 AND m.current_content_version_id=?2 AND m.state='active'",params![id,version],|r|Ok(PlaybackState {position_seconds:r.get(0)?,duration_seconds:r.get(1)?,state_version:r.get(2)?})).optional()?)
    }
    pub fn claim(
        &mut self,
        id: &str,
        version: i64,
        now: i64,
    ) -> AppResult<(String, PlaybackState)> {
        let tx = self.connection.transaction()?;
        let duration:Option<Option<f64>>=tx.query_row("SELECT duration FROM media WHERE id=?1 AND current_content_version_id=?2 AND state='active' AND media_type='video'",params![id,version],|r|r.get(0)).optional()?;
        let duration = duration.ok_or(AppError::NotFound)?;
        tx.execute("INSERT INTO playback_states(media_id,content_version_id,position_seconds,duration_seconds,completed,updated_at) VALUES(?1,?2,0,?3,0,?4) ON CONFLICT DO NOTHING",params![id,version,duration,now])?;
        let owner: String = tx.query_row("SELECT lower(hex(randomblob(16)))", [], |r| r.get(0))?;
        tx.execute("INSERT INTO playback_owners(media_id,content_version_id,owner_id) VALUES(?1,?2,?3) ON CONFLICT(media_id,content_version_id) DO UPDATE SET owner_id=excluded.owner_id,last_sequence=0,last_position=NULL,last_duration=NULL",params![id,version,owner])?;
        let result = state(&tx, id, version)?;
        tx.commit()?;
        Ok((owner, result))
    }
    pub fn save(
        &mut self,
        id: &str,
        version: i64,
        request: &ProgressWrite,
        now: i64,
    ) -> AppResult<PlaybackState> {
        if request.sequence < 1
            || request.expected_version < 1
            || !request.position_seconds.is_finite()
            || request.position_seconds < 0.0
            || request
                .duration_seconds
                .is_some_and(|d| !d.is_finite() || d <= 0.0)
            || request
                .duration_seconds
                .is_some_and(|d| request.position_seconds > d + 0.001)
        {
            return Err(AppError::Validation(
                "invalid playback recovery state".into(),
            ));
        }
        // Acquire the short write transaction before reading owner/CAS facts.
        // A deferred read cannot safely upgrade while an activity writer commits.
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let owner:Option<(i64,Option<f64>,Option<f64>)>=tx.query_row("SELECT o.last_sequence,o.last_position,o.last_duration FROM playback_owners o JOIN media m ON m.id=o.media_id WHERE o.media_id=?1 AND o.content_version_id=?2 AND o.owner_id=?3 AND m.current_content_version_id=?2 AND m.state='active'",params![id,version,request.token],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        let (sequence, position, duration) = owner.ok_or(AppError::StaleState)?;
        if request.sequence == sequence
            && position == Some(request.position_seconds)
            && duration == request.duration_seconds
        {
            return state(&tx, id, version);
        }
        if request.sequence <= sequence {
            return Err(AppError::StaleState);
        }
        let changed=tx.execute("UPDATE playback_states SET position_seconds=?3,duration_seconds=coalesce(?4,duration_seconds),updated_at=?5,state_version=state_version+1 WHERE media_id=?1 AND content_version_id=?2 AND state_version=?6",params![id,version,request.position_seconds,request.duration_seconds,now,request.expected_version])?;
        if changed != 1 {
            return Err(AppError::StaleState);
        }
        tx.execute("UPDATE playback_owners SET last_sequence=?2,last_position=?3,last_duration=?4 WHERE owner_id=?1",params![request.token,request.sequence,request.position_seconds,request.duration_seconds])?;
        let result = state(&tx, id, version)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn release(&mut self, owner: &str) -> AppResult<()> {
        self.connection
            .execute("DELETE FROM playback_owners WHERE owner_id=?1", [owner])?;
        Ok(())
    }
}
