//! Parameterized, bounded reporting reads; no event writes or qualification.
use crate::{
    domain::reports::*,
    error::{AppError, AppResult},
};
use rusqlite::{params, params_from_iter, types::Value, Connection, Transaction};
use sha2::{Digest, Sha256};

pub struct ReportsRepository<'a> {
    pub(crate) connection: &'a mut Connection,
}

fn generation(tx: &Transaction<'_>) -> AppResult<i64> {
    Ok(tx.query_row("SELECT l.value+r.value FROM library_generation l,report_generation r WHERE l.id=1 AND r.id=1", [], |r| r.get(0))?)
}

fn signature(query: &ReportQuery) -> AppResult<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(query)?)))
}

/// All ORDER BY expressions are selected from the closed ReportKind enum.
fn statement(
    query: &ReportQuery,
    cursor: Option<&ReportCursor>,
    limit: u32,
) -> (String, Vec<Value>) {
    use ReportKind::*;
    let mut values = vec![
        Value::Integer(query.window.start.unwrap_or(0)),
        Value::Integer(query.window.end),
    ];
    let (cte, from, count, last, position, duration, key) = match query.kind {
        ContinueWatching => (
            String::new(),
            "current_playback_progress p JOIN media m ON m.id=p.media_id WHERE p.position_seconds>0 AND p.completed=0 AND p.updated_at>=?1 AND p.updated_at<?2".to_owned(),
            "0", "p.updated_at", "p.position_seconds", "p.duration_seconds", "p.updated_at",
        ),
        RecentlyAdded => (
            String::new(),
            "media m WHERE m.state='active' AND m.imported_at>=?1 AND m.imported_at<?2".to_owned(),
            "0", "m.imported_at", "NULL", "m.duration", "m.imported_at",
        ),
        kind => {
            let (table, time) = match kind {
                RecentlyViewed | MostViewed => ("view_events", "occurred_at"),
                RecentlyPlayed | MostPlayed => ("play_events", "qualified_at"),
                _ => ("like_events", "occurred_at"),
            };
            let cte = format!("WITH ranked AS (SELECT media_id,count(*) AS event_count,max({time}) AS last_at FROM {table} WHERE {time}>=?1 AND {time}<?2 GROUP BY media_id) ");
            (cte, "ranked r JOIN media m ON m.id=r.media_id WHERE 1=1".to_owned(), "r.event_count", "r.last_at", "NULL", "m.duration",
                if matches!(kind, RecentlyViewed | RecentlyPlayed) { "r.last_at" } else { "r.event_count" })
        },
    };
    let mut sql = format!("{cte}SELECT m.id,m.file_name,m.media_type,m.state,{count},{last},{position},{duration},{key} FROM {from}");
    let sort_id = if matches!(query.kind, ContinueWatching) {
        "p.media_id"
    } else {
        "m.id"
    };
    if let Some(cursor) = cursor {
        values.extend([Value::Integer(cursor.key), Value::Text(cursor.id.clone())]);
        sql.push_str(&format!(" AND ({key},{sort_id})<(?3,?4)"));
    }
    values.push(Value::Integer(i64::from(limit)));
    sql.push_str(&format!(
        " ORDER BY {key} DESC,{sort_id} DESC LIMIT ?{}",
        values.len()
    ));
    (sql, values)
}

fn read(
    tx: &Transaction<'_>,
    query: &ReportQuery,
    cursor: Option<&ReportCursor>,
    limit: u32,
) -> AppResult<Vec<(ReportItem, i64)>> {
    let (sql, values) = statement(query, cursor, limit);
    let mut stmt = tx.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(values), |r| {
        Ok((
            ReportItem {
                id: r.get(0)?,
                file_name: r.get(1)?,
                media_type: r.get(2)?,
                state: r.get(3)?,
                count: r.get(4)?,
                last_at: r.get(5)?,
                position: r.get(6)?,
                duration: r.get(7)?,
            },
            r.get(8)?,
        ))
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

impl ReportsRepository<'_> {
    pub fn query(
        &mut self,
        query: &ReportQuery,
        cursor: Option<&ReportCursor>,
        limit: u32,
    ) -> AppResult<ReportPage> {
        let limit = super::page_size(limit)?;
        let signature = signature(query)?;
        // SQLite can spill grouping/sorting to disk; Rust only holds limit+1 rows.
        self.connection
            .execute_batch("PRAGMA temp_store=FILE; PRAGMA cache_size=-4096;")?;
        let tx = self.connection.transaction()?;
        let generation = generation(&tx)?;
        if cursor.is_some_and(|c| {
            c.generation != generation || c.signature != signature || c.id.len() > 256 || c.key < 0
        }) {
            return Err(AppError::StaleCursor);
        }
        let mut rows = read(&tx, query, cursor, limit + 1)?;
        let more = rows.len() > limit as usize;
        rows.truncate(limit as usize);
        let next_cursor = if more {
            rows.last().map(|(item, key)| ReportCursor {
                generation,
                signature,
                key: *key,
                id: item.id.clone(),
            })
        } else {
            None
        };
        tx.commit()?;
        Ok(ReportPage {
            items: rows.into_iter().map(|(item, _)| item).collect(),
            next_cursor,
        })
    }

    pub fn dashboard(&mut self, today: TimeWindow) -> AppResult<Dashboard> {
        self.connection
            .execute_batch("PRAGMA temp_store=FILE; PRAGMA cache_size=-4096;")?;
        let tx = self.connection.transaction()?;
        let (total_media,images,videos) = tx.query_row("SELECT count(*),coalesce(sum(media_type='image'),0),coalesce(sum(media_type='video'),0) FROM media WHERE state IN ('active','missing')", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        let total_likes = tx.query_row(
            "SELECT count(*) FROM like_events WHERE occurred_at<?1",
            [today.end],
            |r| r.get(0),
        )?;
        let views_today = tx.query_row(
            "SELECT count(*) FROM view_events WHERE occurred_at>=?1 AND occurred_at<?2",
            params![today.start, today.end],
            |r| r.get(0),
        )?;
        let preview = |kind| -> AppResult<Vec<ReportItem>> {
            Ok(read(
                &tx,
                &ReportQuery {
                    kind,
                    window: TimeWindow {
                        start: None,
                        end: today.end,
                    },
                },
                None,
                6,
            )?
            .into_iter()
            .map(|(item, _)| item)
            .collect())
        };
        let result = Dashboard {
            total_media,
            images,
            videos,
            total_likes,
            views_today,
            recently_added: preview(ReportKind::RecentlyAdded)?,
            recently_viewed: preview(ReportKind::RecentlyViewed)?,
            continue_watching: preview(ReportKind::ContinueWatching)?,
            most_liked: preview(ReportKind::MostLiked)?,
            most_viewed: preview(ReportKind::MostViewed)?,
        };
        tx.commit()?;
        Ok(result)
    }
}
