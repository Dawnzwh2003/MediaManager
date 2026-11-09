//! Parameterized storage and atomic event/cache transactions; no playback qualification policy.
use crate::{
    domain::activity::*,
    error::{AppError, AppResult},
};
use rusqlite::{params, OptionalExtension, Transaction};
pub(crate) struct ActivityTransaction<'a>(pub Transaction<'a>);
#[derive(Clone, Debug)]
pub(crate) struct PlayerWork {
    pub tail_position: Option<f64>,
    pub id: String,
    pub media_id: String,
    pub version: i64,
    pub sequence: i64,
    pub payload: String,
    pub clock: f64,
    pub received: i64,
    pub position: f64,
    pub duration: Option<f64>,
    pub rate: f64,
    pub playing: bool,
    pub watched: f64,
    pub covered: f64,
    pub started: Option<i64>,
    pub start_position: f64,
    pub ended: Option<i64>,
    pub reason: Option<String>,
}
impl ActivityTransaction<'_> {
    pub fn interrupted_ids(&self, before: i64) -> AppResult<Vec<String>> {
        Ok(self.0.prepare("SELECT id FROM player_sessions WHERE ended_at IS NULL AND received_at<?1 ORDER BY id LIMIT 100")?.query_map([before],|r|r.get(0))?.collect::<Result<_,_>>()?)
    }
    pub fn active(&self, id: &str, version: Option<i64>, video: bool) -> AppResult<()> {
        let found:bool=self.0.query_row("SELECT EXISTS(SELECT 1 FROM media WHERE id=?1 AND state='active' AND (?2 IS NULL OR current_content_version_id=?2) AND (?3=0 OR media_type='video'))",params![id,version,video],|r|r.get(0))?;
        if !found {
            return Err(AppError::NotFound);
        }
        Ok(())
    }
    pub fn result(&self, id: &str, event: &str) -> AppResult<ActivityResult> {
        Ok(self.0.query_row(
            "SELECT like_count,view_count,play_count FROM media WHERE id=?1",
            [id],
            |r| {
                Ok(ActivityResult {
                    id: event.into(),
                    like_count: r.get(0)?,
                    view_count: r.get(1)?,
                    play_count: r.get(2)?,
                })
            },
        )?)
    }
    pub fn like_media(&self, request: &str) -> AppResult<Option<String>> {
        Ok(self
            .0
            .query_row(
                "SELECT media_id FROM like_events WHERE request_id=?1",
                [request],
                |r| r.get(0),
            )
            .optional()?)
    }
    pub fn sequence(&self) -> AppResult<i64> {
        Ok(self.0.query_row(
            "UPDATE db_sequence SET value=value+1 WHERE id=1 RETURNING value",
            [],
            |r| r.get(0),
        )?)
    }
    pub fn insert_like(&self, id: &str, request: &str, now: i64) -> AppResult<()> {
        self.0.execute("INSERT INTO like_events(id,ingest_seq,request_id,media_id,occurred_at,recorded_at) VALUES(?1,?2,?1,?3,?4,?4)",params![request,self.sequence()?,id,now])?;
        Ok(())
    }
    pub fn activation(&self, id: &str) -> AppResult<Option<Activation>> {
        let value:Option<(String,i64,String,i64,String)>=self.0.query_row("SELECT viewer_id,ordinal,media_id,content_version_id,source FROM viewer_activations WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional()?;
        value
            .map(
                |(viewer_id, ordinal, media_id, content_version_id, source)| {
                    Ok(Activation {
                        id: id.into(),
                        viewer_id,
                        ordinal,
                        media_id,
                        content_version_id,
                        source: serde_json::from_value(serde_json::Value::String(source))?,
                    })
                },
            )
            .transpose()
    }
    pub fn current_activation(&self, viewer: &str) -> AppResult<Option<(String, i64)>> {
        Ok(self.0.query_row("SELECT id,ordinal FROM viewer_activations WHERE viewer_id=?1 ORDER BY ordinal DESC LIMIT 1",[viewer],|r|Ok((r.get(0)?,r.get(1)?))).optional()?)
    }
    pub fn close_activation(&self, id: &str) -> AppResult<()> {
        self.0
            .execute("UPDATE viewer_activations SET closed=1 WHERE id=?1", [id])?;
        Ok(())
    }
    pub fn insert_view(&self, a: &Activation, now: i64) -> AppResult<()> {
        self.0.execute("INSERT INTO viewer_activations(id,viewer_id,ordinal,media_id,content_version_id,source) VALUES(?1,?2,?3,?4,?5,?6)",params![a.id,a.viewer_id,a.ordinal,a.media_id,a.content_version_id,a.source.key()])?;
        self.0.execute("INSERT INTO view_events(id,ingest_seq,request_id,media_id,activation_id,occurred_at,recorded_at,source) VALUES(?1,?2,?1,?3,?1,?4,?4,?5)",params![a.id,self.sequence()?,a.media_id,now,a.source.key()])?;
        Ok(())
    }
    pub fn open_player(&self, id: &str, activation: &str, now: i64) -> AppResult<()> {
        self.0.execute("INSERT INTO player_sessions(id,activation_id,media_id,content_version_id,received_at) SELECT ?1,id,media_id,content_version_id,?3 FROM viewer_activations WHERE id=?2 AND closed=0",params![id,activation,now])?;
        Ok(())
    }
    pub fn player(&self, id: &str) -> AppResult<Option<PlayerWork>> {
        Ok(self.0.query_row("SELECT id,media_id,content_version_id,sequence,payload,clock_ms,received_at,position,duration,rate,playing,watched,covered,started_at,start_position,ended_at,end_reason,tail_position FROM player_sessions WHERE id=?1",[id],|r|Ok(PlayerWork{id:r.get(0)?,media_id:r.get(1)?,version:r.get(2)?,sequence:r.get(3)?,payload:r.get(4)?,clock:r.get(5)?,received:r.get(6)?,position:r.get(7)?,duration:r.get(8)?,rate:r.get(9)?,playing:r.get(10)?,watched:r.get(11)?,covered:r.get(12)?,started:r.get(13)?,start_position:r.get(14)?,ended:r.get(15)?,reason:r.get(16)?,tail_position:r.get(17)?})).optional()?)
    }
    pub fn player_for_activation(&self, id: &str) -> AppResult<Option<String>> {
        Ok(self
            .0
            .query_row(
                "SELECT id FROM player_sessions WHERE activation_id=?1",
                [id],
                |r| r.get(0),
            )
            .optional()?)
    }
    pub fn store_player(&self, p: &PlayerWork) -> AppResult<()> {
        self.0.execute("UPDATE player_sessions SET sequence=?2,payload=?3,clock_ms=?4,received_at=?5,position=?6,duration=?7,rate=?8,playing=?9,watched=?10,covered=?11,started_at=?12,start_position=?13,ended_at=?14,end_reason=?15,tail_position=?16 WHERE id=?1",params![p.id,p.sequence,p.payload,p.clock,p.received,p.position,p.duration,p.rate,p.playing,p.watched,p.covered,p.started,p.start_position,p.ended,p.reason,p.tail_position])?;
        Ok(())
    }
    pub fn qualified(&self, id: &str) -> AppResult<bool> {
        Ok(self.0.query_row(
            "SELECT EXISTS(SELECT 1 FROM play_events WHERE player_session_id=?1)",
            [id],
            |r| r.get(0),
        )?)
    }
    pub fn insert_play(&self, p: &PlayerWork, now: i64, completed: bool) -> AppResult<()> {
        self.0.execute("INSERT INTO play_events(id,ingest_seq,request_id,media_id,content_version_id,player_session_id,started_at,qualified_at,start_position,end_position,watched_seconds,completed) VALUES(?1,?2,?1,?3,?4,?1,?5,?6,?7,?8,?9,?10)",params![p.id,self.sequence()?,p.media_id,p.version,p.started,now,p.start_position,p.position,p.watched,completed])?;
        Ok(())
    }
    pub fn finalize(&self, p: &PlayerWork, completed: bool) -> AppResult<()> {
        self.0.execute("UPDATE play_events SET ended_at=max(qualified_at,?2),end_position=?3,watched_seconds=?4,completed=?5,end_reason=?6 WHERE player_session_id=?1 AND ended_at IS NULL",params![p.id,p.ended,p.position,p.watched,completed,p.reason])?;
        Ok(())
    }
}
