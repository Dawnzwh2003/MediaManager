//! Reporting use cases. All aggregation stays in SQLite, off the UI thread.
use crate::{
    db::Database,
    domain::reports::*,
    error::{AppError, AppResult},
};

fn validate(window: TimeWindow) -> AppResult<()> {
    const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;
    if !(1..=MAX_SAFE_INTEGER).contains(&window.end)
        || window
            .start
            .is_some_and(|start| start < 0 || start >= window.end)
    {
        return Err(AppError::Validation("invalid reporting time window".into()));
    }
    Ok(())
}

pub fn query(
    db: &mut Database,
    query: &ReportQuery,
    cursor: Option<&ReportCursor>,
    limit: u32,
) -> AppResult<ReportPage> {
    validate(query.window)?;
    db.repositories().reports().query(query, cursor, limit)
}

pub fn dashboard(db: &mut Database, today: TimeWindow) -> AppResult<Dashboard> {
    validate(today)?;
    if today.start.is_none() {
        return Err(AppError::Validation(
            "dashboard needs a local day boundary".into(),
        ));
    }
    db.repositories().reports().dashboard(today)
}
