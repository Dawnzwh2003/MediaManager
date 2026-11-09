//! Bounded high-water/base/delta storage protocol. No event qualification decisions.
use crate::{
    domain::activity::StatisticsJob,
    error::{AppError, AppResult},
};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
pub(crate) struct StatisticsStore<'a>(pub &'a mut Connection);
impl StatisticsStore<'_> {
    pub fn status(&self, id: &str) -> AppResult<StatisticsJob> {
        Ok(self.0.query_row(
            "SELECT id,state,phase,error FROM statistics_jobs WHERE id=?1",
            [id],
            |r| {
                Ok(StatisticsJob {
                    id: r.get(0)?,
                    state: r.get(1)?,
                    phase: r.get(2)?,
                    error: r.get(3)?,
                })
            },
        )?)
    }
    pub fn active(&self) -> AppResult<Option<String>> {
        Ok(self
            .0
            .query_row(
                "SELECT id FROM statistics_jobs j WHERE state IN('running','cleaning') OR (state='failed' AND (EXISTS(SELECT 1 FROM statistics_base WHERE job_id=j.id) OR EXISTS(SELECT 1 FROM statistics_delta WHERE job_id=j.id))) ORDER BY CASE WHEN state='failed' THEN 1 ELSE 0 END LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?)
    }
    pub fn start(&mut self, id: &str, kind: &str, media: Option<&str>) -> AppResult<StatisticsJob> {
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let prior: Option<(String, Option<String>)> = tx
            .query_row(
                "SELECT kind,media_id FROM statistics_jobs WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some(old) = prior {
            if old != (kind.into(), media.map(String::from)) {
                return Err(AppError::Validation("rebuild request conflict".into()));
            }
        } else {
            if self_active(&tx)? {
                return Err(AppError::Busy);
            }
            if let Some(media) = media {
                if !tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM media WHERE id=?1)",
                    [media],
                    |r| r.get::<_, bool>(0),
                )? {
                    return Err(AppError::NotFound);
                }
            }
            tx.execute("INSERT INTO statistics_jobs(id,kind,media_id,high_water) SELECT ?1,?2,?3,value FROM db_sequence WHERE id=1",params![id,kind,media])?;
        }
        tx.commit()?;
        self.status(id)
    }
    pub fn step(&mut self, id: &str) -> AppResult<StatisticsJob> {
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (state,phase,cursor,high,kind,media,published):(String,i64,i64,i64,String,Option<String>,String)=tx.query_row("SELECT state,phase,scan_cursor,high_water,kind,media_id,publish_cursor FROM statistics_jobs WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?)))?;
        if state == "cleaning" || state == "failed" {
            for table in ["statistics_base", "statistics_delta"] {
                tx.execute(&format!("DELETE FROM {table} WHERE (job_id,media_id) IN(SELECT job_id,media_id FROM {table} WHERE job_id=?1 LIMIT 1000)"),[id])?;
            }
            let remain:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM statistics_base WHERE job_id=?1) OR EXISTS(SELECT 1 FROM statistics_delta WHERE job_id=?1)",[id],|r|r.get(0))?;
            if !remain && state != "failed" {
                tx.execute("UPDATE statistics_jobs SET state=CASE WHEN error='cancelled' THEN 'cancelled' ELSE 'complete' END WHERE id=?1",[id])?;
            }
        } else if state == "running" && phase < 3 {
            let table = ["like_events", "view_events", "play_events"][phase as usize];
            let timestamp = if phase == 1 {
                "occurred_at"
            } else if phase == 2 {
                "qualified_at"
            } else {
                "NULL"
            };
            let rows:Vec<(i64,String,Option<i64>)>=tx.prepare(&format!("SELECT ingest_seq,media_id,{timestamp} FROM {table} WHERE ingest_seq>?1 AND ingest_seq<=?2 ORDER BY ingest_seq LIMIT 1000"))?.query_map(params![cursor,high],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?.collect::<Result<_,_>>()?;
            for (_, m, at) in &rows {
                tx.execute("INSERT INTO statistics_base(job_id,media_id,likes,views,plays,last_view,last_play) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(job_id,media_id) DO UPDATE SET likes=likes+excluded.likes,views=views+excluded.views,plays=plays+excluded.plays,last_view=max(coalesce(last_view,excluded.last_view),coalesce(excluded.last_view,last_view)),last_play=max(coalesce(last_play,excluded.last_play),coalesce(excluded.last_play,last_play))",params![id,m,i64::from(phase==0),i64::from(phase==1),i64::from(phase==2),if phase==1{*at}else{None},if phase==2{*at}else{None}])?;
            }
            if let Some((last, _, _)) = rows.last() {
                tx.execute(
                    "UPDATE statistics_jobs SET scan_cursor=?2 WHERE id=?1",
                    params![id, last],
                )?;
            } else {
                tx.execute(
                    "UPDATE statistics_jobs SET phase=phase+1,scan_cursor=0 WHERE id=?1",
                    [id],
                )?;
            }
        } else if state == "running" {
            let ids:Vec<String>=tx.prepare("SELECT id FROM media WHERE id>?1 AND created_seq<=?2 AND (?3 IS NULL OR id=?3) ORDER BY id LIMIT 100")?.query_map(params![published,high,media],|r|r.get(0))?.collect::<Result<_,_>>()?;
            for m in &ids {
                tx.execute("UPDATE media SET
    like_count=CASE WHEN ?3 IN('all','like') THEN coalesce((SELECT likes FROM statistics_base WHERE job_id=?1 AND media_id=?2),0)+coalesce((SELECT likes FROM statistics_delta WHERE job_id=?1 AND media_id=?2),0) ELSE like_count END,
    view_count=CASE WHEN ?3 IN('all','view') THEN coalesce((SELECT views FROM statistics_base WHERE job_id=?1 AND media_id=?2),0)+coalesce((SELECT views FROM statistics_delta WHERE job_id=?1 AND media_id=?2),0) ELSE view_count END,
    play_count=CASE WHEN ?3 IN('all','play') THEN coalesce((SELECT plays FROM statistics_base WHERE job_id=?1 AND media_id=?2),0)+coalesce((SELECT plays FROM statistics_delta WHERE job_id=?1 AND media_id=?2),0) ELSE play_count END,
    last_viewed_at=CASE WHEN ?3 IN('all','view') THEN (SELECT max(last_view) FROM(SELECT last_view FROM statistics_base WHERE job_id=?1 AND media_id=?2 UNION ALL SELECT last_view FROM statistics_delta WHERE job_id=?1 AND media_id=?2)) ELSE last_viewed_at END,
    last_played_at=CASE WHEN ?3 IN('all','play') THEN (SELECT max(last_play) FROM(SELECT last_play FROM statistics_base WHERE job_id=?1 AND media_id=?2 UNION ALL SELECT last_play FROM statistics_delta WHERE job_id=?1 AND media_id=?2)) ELSE last_played_at END
    WHERE id=?2",params![id,m,kind])?;
            }
            if let Some(last) = ids.last() {
                tx.execute(
                    "UPDATE statistics_jobs SET publish_cursor=?2 WHERE id=?1",
                    params![id, last],
                )?;
            } else {
                tx.execute(
                    "UPDATE statistics_jobs SET state='cleaning' WHERE id=?1",
                    [id],
                )?;
            }
        }
        tx.commit()?;
        self.status(id)
    }
    pub fn cancel(&self, id: &str) -> AppResult<()> {
        self.0.execute("UPDATE statistics_jobs SET state='cleaning',error='cancelled' WHERE id=?1 AND state='running'",[id])?;
        Ok(())
    }
    pub fn fail(&self, id: &str) -> AppResult<()> {
        self.0.execute("UPDATE statistics_jobs SET state='failed',error='Statistics rebuild failed; retry with a new request.' WHERE id=?1 AND state IN('running','cleaning')",[id])?;
        Ok(())
    }
}
fn self_active(c: &Connection) -> AppResult<bool> {
    Ok(c.query_row(
        "SELECT EXISTS(SELECT 1 FROM statistics_jobs WHERE state IN('running','cleaning'))",
        [],
        |r| r.get(0),
    )?)
}
