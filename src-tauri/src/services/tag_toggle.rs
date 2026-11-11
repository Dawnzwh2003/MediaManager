//! Custom Tag use case. The snapshot only supplies the current stable media identity.
use crate::{
    db::Database,
    domain::quick_browse::*,
    error::{AppError, AppResult},
    services::activity::{ActivityClock, SystemClock},
};

pub fn toggle_for_quick_browse(
    db: &mut Database,
    request: &ToggleTagRequest,
) -> AppResult<ToggleTagResult> {
    super::quick_browse::validate_id(&request.request_id)?;
    if !(1..=9).contains(&request.digit) || request.ordinal < 0 {
        return Err(AppError::Validation("invalid tag shortcut".into()));
    }
    let gate = db.activity_gate.clone();
    let _permit = gate.lock().map_err(|_| AppError::Internal)?;
    db.quick_tag_store().transaction(|transaction| {
        if let Some(previous) = transaction.previous(&request.request_id)? {
            if previous.session_id != request.session_id
                || previous.ordinal != request.ordinal
                || previous.digit != request.digit
            {
                return Err(AppError::Validation("tag shortcut request conflict".into()));
            }
            return Ok(previous.result);
        }
        let media = transaction
            .current_media(request)?
            .ok_or(AppError::StaleState)?;
        let preferences = transaction.preferences()?;
        let binding = preferences
            .bindings
            .iter()
            .find(|binding| binding.digit == request.digit)
            .ok_or(AppError::NotFound)?;
        let name = transaction
            .tag(&binding.tag_id)?
            .ok_or(AppError::NotFound)?;
        let attached = !transaction.attached(&media, &binding.tag_id)?;
        transaction.set_link(&media, &binding.tag_id, attached)?;
        let result = ToggleTagResult {
            media_id: media,
            tag_id: binding.tag_id.clone(),
            tag_name: name,
            attached,
        };
        transaction.receipt(request, &result, SystemClock.now_ms())?;
        Ok(result)
    })
}
