//! Activity policy lives here; repositories only persist facts and transactions.
use crate::{
    db::{
        repositories::activity::{ActivityTransaction, PlayerWork},
        Database,
    },
    domain::activity::*,
    error::{AppError, AppResult},
};
use std::{
    sync::OnceLock,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
pub trait ActivityClock {
    fn now_ms(&self) -> i64;
}
pub struct SystemClock;
impl ActivityClock for SystemClock {
    fn now_ms(&self) -> i64 {
        static ORIGIN: OnceLock<(Instant, i64)> = OnceLock::new();
        let (tick, epoch) = ORIGIN.get_or_init(|| {
            (
                Instant::now(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            )
        });
        epoch + tick.elapsed().as_millis() as i64
    }
}
fn valid_id(id: &str) -> AppResult<()> {
    if id.is_empty()
        || id.len() > 160
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
    {
        return Err(AppError::Validation("invalid activity identity".into()));
    }
    Ok(())
}
pub fn completed(position: f64, duration: Option<f64>) -> bool {
    duration
        .filter(|d| d.is_finite() && *d > 0.0)
        .is_some_and(|d| {
            let position = position.clamp(0.0, d);
            position / d >= 0.95 || d - position <= 10.0_f64.min(d * 0.05)
        })
}
fn transaction<T>(
    db: &mut Database,
    work: impl FnOnce(&ActivityTransaction<'_>) -> AppResult<T>,
) -> AppResult<T> {
    let gate = db.activity_gate.clone();
    let _permit = gate.lock().map_err(|_| AppError::Internal)?;
    let tx = ActivityTransaction(
        db.activity_store()
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?,
    );
    let result = work(&tx)?;
    tx.0.commit()?;
    Ok(result)
}
pub fn add_like(db: &mut Database, media_id: &str, request_id: &str) -> AppResult<ActivityResult> {
    valid_id(request_id)?;
    transaction(db, |tx| {
        if let Some(old) = tx.like_media(request_id)? {
            if old != media_id {
                return Err(AppError::Validation("request identity conflict".into()));
            }
        } else {
            tx.active(media_id, None, false)?;
            tx.insert_like(media_id, request_id, SystemClock.now_ms())?;
        }
        tx.result(media_id, request_id)
    })
}
fn finish(
    tx: &ActivityTransaction<'_>,
    p: &mut PlayerWork,
    now: i64,
    reason: &str,
) -> AppResult<()> {
    if p.ended.is_none() {
        p.ended = Some(now);
        p.playing = false;
        p.reason = Some(reason.into());
        tx.store_player(p)?;
        tx.finalize(p, completed(p.position, p.duration))?;
    }
    Ok(())
}
pub fn record_view(db: &mut Database, a: &Activation) -> AppResult<ActivityResult> {
    for id in [&a.id, &a.viewer_id] {
        valid_id(id)?;
    }
    if a.ordinal < 1 {
        return Err(AppError::Validation("invalid activation ordinal".into()));
    }
    transaction(db, |tx| {
        if let Some(old) = tx.activation(&a.id)? {
            if serde_json::to_string(&old)? != serde_json::to_string(a)? {
                return Err(AppError::Validation("activation identity conflict".into()));
            }
            return tx.result(&a.media_id, &a.id);
        }
        tx.active(&a.media_id, Some(a.content_version_id), false)?;
        if let Some((previous, ordinal)) = tx.current_activation(&a.viewer_id)? {
            if ordinal >= a.ordinal {
                return Err(AppError::StaleState);
            }
            if let Some(id) = tx.player_for_activation(&previous)? {
                if let Some(mut p) = tx.player(&id)? {
                    finish(tx, &mut p, SystemClock.now_ms(), "switch")?;
                }
            }
            tx.close_activation(&previous)?;
        }
        tx.insert_view(a, SystemClock.now_ms())?;
        tx.result(&a.media_id, &a.id)
    })
}
pub fn open_player_session(db: &mut Database, activation_id: &str) -> AppResult<String> {
    transaction(db, |tx| {
        if let Some(id) = tx.player_for_activation(activation_id)? {
            return Ok(id);
        }
        let a = tx.activation(activation_id)?.ok_or(AppError::NotFound)?;
        tx.active(&a.media_id, Some(a.content_version_id), true)?;
        tx.open_player(&a.id, activation_id, SystemClock.now_ms())?;
        tx.player(&a.id)?.ok_or(AppError::StaleState)?;
        Ok(a.id)
    })
}
pub fn record_play(db: &mut Database, e: &PlaybackEvidence) -> AppResult<PlayResult> {
    record_play_with_clock(db, e, &SystemClock)
}
pub fn record_play_with_clock(
    db: &mut Database,
    e: &PlaybackEvidence,
    clock: &dyn ActivityClock,
) -> AppResult<PlayResult> {
    if e.sequence < 1
        || !e.clock_ms.is_finite()
        || e.clock_ms < 0.0
        || !e.position.is_finite()
        || e.position < 0.0
        || !e.rate.is_finite()
        || !(0.25..=4.0).contains(&e.rate)
        || e.duration.is_some_and(|d| !d.is_finite() || d <= 0.0)
    {
        return Err(AppError::Validation("invalid playback evidence".into()));
    }
    transaction(db, |tx| {
        let mut p = tx.player(&e.session_id)?.ok_or(AppError::NotFound)?;
        let payload = serde_json::to_string(e)?;
        if e.sequence == p.sequence && p.payload == payload {
            return Ok(PlayResult {
                session_id: p.id.clone(),
                qualified: tx.qualified(&p.id)?,
                watched_seconds: p.watched,
                finalized: p.ended.is_some(),
            });
        }
        if e.sequence <= p.sequence || p.ended.is_some() || e.clock_ms < p.clock {
            return Err(AppError::StaleState);
        }
        tx.active(&p.media_id, Some(p.version), true)?;
        let now = clock.now_ms();
        let elapsed = ((e.clock_ms - p.clock) / 1000.0)
            .min((now - p.received).max(0) as f64 / 1000.0)
            .clamp(0.0, 2.0);
        let advance = e.position - p.position;
        // Stop boundaries retain the preceding played interval. Seek/restore discontinuities never do.
        let continuous = p.sequence > 0
            && p.playing
            && e.phase != PlaybackPhase::Seeking
            && advance > 0.0
            && advance <= elapsed * p.rate + 0.25;
        if continuous {
            let watched = elapsed.min(advance / p.rate);
            p.watched += watched;
            p.covered += advance.min(watched * p.rate);
            p.tail_position = Some(e.position);
        }
        if e.phase == PlaybackPhase::Seeking {
            p.tail_position = None;
        }
        if p.started.is_none() && e.phase == PlaybackPhase::Playing {
            p.started = Some(now);
            p.start_position = e.position;
        }
        p.duration = e.duration.or(p.duration);
        p.position = p.duration.map_or(e.position, |d| e.position.min(d));
        p.clock = e.clock_ms;
        p.received = now;
        p.sequence = e.sequence;
        p.payload = payload;
        p.rate = e.rate;
        p.playing = e.phase == PlaybackPhase::Playing;
        let short_ended = e.phase == PlaybackPhase::Ended
            && p.tail_position
                .is_some_and(|position| (position - e.position).abs() < 0.01)
            && p.duration
                .is_some_and(|d| d < 5.0 && p.watched > 0.0 && p.position >= d - 0.05);
        let qualifies = p.started.is_some()
            && if p.duration.is_some_and(|d| d < 5.0) {
                short_ended
            } else {
                p.watched >= 5.0
            };
        if qualifies && !tx.qualified(&p.id)? {
            tx.insert_play(&p, now, completed(p.position, p.duration))?;
        }
        if e.phase == PlaybackPhase::Closed {
            finish(tx, &mut p, now, "close")?;
        } else {
            tx.store_player(&p)?;
        }
        Ok(PlayResult {
            session_id: p.id.clone(),
            qualified: tx.qualified(&p.id)?,
            watched_seconds: p.watched,
            finalized: p.ended.is_some(),
        })
    })
}
pub fn finalize_play(db: &mut Database, id: &str) -> AppResult<()> {
    transaction(db, |tx| {
        if let Some(mut p) = tx.player(id)? {
            finish(tx, &mut p, SystemClock.now_ms(), "close")?;
        }
        tx.close_activation(id)?;
        Ok(())
    })
}
pub fn recover_interrupted(db: &mut Database) -> AppResult<()> {
    while recover_batch(db, i64::MAX)? {}
    Ok(())
}
pub(crate) fn recover_batch(db: &mut Database, before: i64) -> AppResult<bool> {
    let ids = transaction(db, |tx| tx.interrupted_ids(before))?;
    let more = !ids.is_empty();
    for id in ids {
        transaction(db, |tx| {
            if let Some(mut p) = tx.player(&id)? {
                let received = p.received;
                finish(tx, &mut p, received, "interrupted")?;
            }
            Ok(())
        })?;
    }
    Ok(more)
}
