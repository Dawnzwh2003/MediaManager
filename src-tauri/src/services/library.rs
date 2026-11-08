use crate::{
    db::Database,
    domain::library_query::*,
    error::{AppError, AppResult},
};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn mutate_tag(database: &mut Database, mutation: TagMutation) -> AppResult<Option<TagLabel>> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AppError::Internal)?
        .as_millis() as i64;
    let name = |value: String| -> AppResult<String> {
        let value = value.trim();
        if value.is_empty() || value.chars().count() > 128 {
            return Err(AppError::Validation(
                "tag name must contain 1–128 characters".into(),
            ));
        }
        Ok(value.to_owned())
    };
    match mutation {
        TagMutation::Create { name: input } => {
            let name = name(input)?;
            let id = database.repositories().library().new_tag_id()?;
            database
                .repositories()
                .custom_tags()
                .create(&id, &name, now)?;
            Ok(Some(TagLabel { id, name }))
        }
        TagMutation::Rename { id, name: input } => {
            let name = name(input)?;
            database
                .repositories()
                .custom_tags()
                .rename(&id, &name, now)?;
            Ok(Some(TagLabel { id, name }))
        }
        TagMutation::Delete { id } => {
            database.repositories().custom_tags().delete(&id)?;
            Ok(None)
        }
        TagMutation::Attach { media_ids, tag_ids } => {
            database
                .repositories()
                .library()
                .batch_tags(&media_ids, &tag_ids, true)?;
            Ok(None)
        }
        TagMutation::Detach { media_ids, tag_ids } => {
            database
                .repositories()
                .library()
                .batch_tags(&media_ids, &tag_ids, false)?;
            Ok(None)
        }
    }
}
pub fn thumbnails(
    database: &mut Database,
    ids: Vec<String>,
    retry: bool,
) -> AppResult<Vec<ThumbnailResult>> {
    if ids.len() > 100 {
        return Err(AppError::Validation("thumbnail batch exceeds 100".into()));
    }
    ids.into_iter()
        .map(|id| match database.request_thumbnail(&id, retry) {
            Ok(s) => Ok(ThumbnailResult {
                media_id: id,
                status: s.status,
                path: s.thumbnail_path,
                error: s.thumbnail_error,
            }),
            Err(AppError::NotFound) => Ok(ThumbnailResult {
                media_id: id,
                status: "unavailable".into(),
                path: None,
                error: None,
            }),
            Err(e) => Err(e),
        })
        .collect()
}
