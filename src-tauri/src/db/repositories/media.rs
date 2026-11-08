use rusqlite::{params, types::Type, Connection, OptionalExtension, Row, TransactionBehavior};

use super::{changed, nonempty, page_size};
use crate::{
    domain::library::{Media, MediaCursor, MediaType, NewMedia},
    error::{AppError, AppResult},
};

pub struct MediaRepository<'a> {
    pub(crate) connection: &'a mut Connection,
}

const COLUMNS: &str = "id,fingerprint,file_path,file_name,media_type,file_size,created_time,modified_time,imported_at,last_seen_at,width,height,duration,thumbnail_path,like_count,view_count,play_count,last_viewed_at,last_played_at,playback_position,missing,state,current_content_version_id,video_codec,audio_codec,container,frame_rate";

fn map_media(row: &Row<'_>) -> rusqlite::Result<Media> {
    let kind: String = row.get(4)?;
    let media_type = match kind.as_str() {
        "image" => MediaType::Image,
        "video" => MediaType::Video,
        _ => {
            return Err(rusqlite::Error::InvalidColumnType(
                4,
                "media_type".into(),
                Type::Text,
            ))
        }
    };
    Ok(Media {
        id: row.get(0)?,
        fingerprint: row.get(1)?,
        file_path: row.get(2)?,
        file_name: row.get(3)?,
        media_type,
        file_size: row.get(5)?,
        created_time: row.get(6)?,
        modified_time: row.get(7)?,
        imported_at: row.get(8)?,
        last_seen_at: row.get(9)?,
        width: row.get(10)?,
        height: row.get(11)?,
        duration: row.get(12)?,
        thumbnail_path: row.get(13)?,
        like_count: row.get(14)?,
        view_count: row.get(15)?,
        play_count: row.get(16)?,
        last_viewed_at: row.get(17)?,
        last_played_at: row.get(18)?,
        playback_position: row.get(19)?,
        missing: row.get(20)?,
        state: row.get(21)?,
        content_version_id: row.get(22)?,
        video_codec: row.get(23)?,
        audio_codec: row.get(24)?,
        container: row.get(25)?,
        frame_rate: row.get(26)?,
    })
}

impl MediaRepository<'_> {
    pub fn insert(&mut self, media: &NewMedia) -> AppResult<Media> {
        nonempty(&media.id)?;
        nonempty(&media.file_path)?;
        nonempty(&media.file_name)?;
        if media
            .duration
            .is_some_and(|value| !value.is_finite() || value < 0.0)
        {
            return Err(AppError::Validation(
                "duration must be finite and nonnegative".into(),
            ));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let sequence: i64 = transaction.query_row(
            "UPDATE db_sequence SET value=value+1 WHERE id=1 RETURNING value",
            [],
            |r| r.get(0),
        )?;
        transaction.execute(
            "INSERT INTO media(id,fingerprint,file_path,file_name,media_type,file_size,created_time,modified_time,imported_at,last_seen_at,width,height,duration,created_seq)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            params![media.id,media.fingerprint,media.file_path,media.file_name,media.media_type.as_str(),media.file_size,
                media.created_time,media.modified_time,media.imported_at,media.last_seen_at,media.width,media.height,media.duration,sequence],
        )?;
        transaction.commit()?;
        self.get(&media.id)?.ok_or(AppError::Internal)
    }

    pub fn get(&self, id: &str) -> AppResult<Option<Media>> {
        Ok(self
            .connection
            .query_row(
                &format!("SELECT {COLUMNS} FROM media WHERE id=?1"),
                [id],
                map_media,
            )
            .optional()?)
    }

    pub fn list(&self, after: Option<&MediaCursor>, limit: u32) -> AppResult<Vec<Media>> {
        let limit = page_size(limit)?;
        let mut statement = if after.is_some() {
            self.connection.prepare(&format!("SELECT {COLUMNS} FROM media WHERE (imported_at,id)<(?1,?2) ORDER BY imported_at DESC,id DESC LIMIT ?3"))?
        } else {
            self.connection.prepare(&format!(
                "SELECT {COLUMNS} FROM media ORDER BY imported_at DESC,id DESC LIMIT ?1"
            ))?
        };
        let rows = if let Some(cursor) = after {
            statement.query_map(params![cursor.imported_at, cursor.id, limit], map_media)?
        } else {
            statement.query_map([limit], map_media)?
        };
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Persists an already-decided observation; performs no matching or filesystem work.
    pub fn update_location(
        &mut self,
        id: &str,
        file_path: &str,
        file_name: &str,
        last_seen_at: i64,
    ) -> AppResult<()> {
        nonempty(file_path)?;
        nonempty(file_name)?;
        changed(self.connection.execute(
            "UPDATE media SET file_path=?2,file_name=?3,last_seen_at=?4 WHERE id=?1",
            params![id, file_path, file_name, last_seen_at],
        )?)
    }

    pub fn set_missing(&mut self, id: &str, missing: bool) -> AppResult<()> {
        changed(self.connection.execute(
            "UPDATE media SET missing=?2,state=CASE WHEN ?2 THEN 'missing' ELSE 'active' END WHERE id=?1 AND state!='superseded'",
            params![id,missing],
        )?)
    }
}
