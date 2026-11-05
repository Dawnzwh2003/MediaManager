use rusqlite::{params, Connection, OptionalExtension, Row};

use super::{changed, nonempty, page_size};
use crate::{
    domain::library::{CustomTag, ObservedDirectory, SystemTag},
    error::{AppError, AppResult},
    platform::observation::is_valid_directory_observation,
};

pub struct CustomTagRepository<'a> {
    pub(crate) connection: &'a mut Connection,
}
pub struct SystemTagRepository<'a> {
    pub(crate) connection: &'a mut Connection,
}

fn custom_tag(row: &Row<'_>) -> rusqlite::Result<CustomTag> {
    Ok(CustomTag {
        id: row.get(0)?,
        name: row.get(1)?,
        created_at: row.get(2)?,
        updated_at: row.get(3)?,
    })
}
fn system_tag(row: &Row<'_>) -> rusqlite::Result<SystemTag> {
    Ok(SystemTag {
        id: row.get(0)?,
        parent_id: row.get(1)?,
        source_root: row.get(2)?,
        directory_path: row.get(3)?,
        name: row.get(4)?,
        last_seen_at: row.get(5)?,
    })
}

impl CustomTagRepository<'_> {
    pub fn create(&mut self, id: &str, name: &str, now: i64) -> AppResult<()> {
        nonempty(id)?;
        nonempty(name)?;
        self.connection.execute(
            "INSERT INTO custom_tags(id,name,created_at,updated_at) VALUES(?1,?2,?3,?3)",
            params![id, name.trim(), now],
        )?;
        Ok(())
    }
    pub fn get(&self, id: &str) -> AppResult<Option<CustomTag>> {
        Ok(self
            .connection
            .query_row(
                "SELECT id,name,created_at,updated_at FROM custom_tags WHERE id=?1",
                [id],
                custom_tag,
            )
            .optional()?)
    }
    pub fn rename(&mut self, id: &str, name: &str, now: i64) -> AppResult<()> {
        nonempty(name)?;
        changed(self.connection.execute(
            "UPDATE custom_tags SET name=?2,updated_at=?3 WHERE id=?1",
            params![id, name.trim(), now],
        )?)
    }
    pub fn delete(&mut self, id: &str) -> AppResult<()> {
        changed(
            self.connection
                .execute("DELETE FROM custom_tags WHERE id=?1", [id])?,
        )
    }
    pub fn attach(&mut self, media_id: &str, tag_id: &str) -> AppResult<()> {
        self.connection.execute("INSERT INTO media_custom_tags(media_id,tag_id) VALUES(?1,?2) ON CONFLICT(media_id,tag_id) DO NOTHING", params![media_id,tag_id])?;
        Ok(())
    }
    pub fn detach(&mut self, media_id: &str, tag_id: &str) -> AppResult<()> {
        self.connection.execute(
            "DELETE FROM media_custom_tags WHERE media_id=?1 AND tag_id=?2",
            params![media_id, tag_id],
        )?;
        Ok(())
    }
    pub fn set_for_media(&mut self, media_id: &str, tag_ids: &[String]) -> AppResult<()> {
        let transaction = self.connection.transaction()?;
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM media WHERE id=?1)",
            [media_id],
            |r| r.get(0),
        )?;
        if !exists {
            return Err(crate::error::AppError::NotFound);
        }
        transaction.execute(
            "DELETE FROM media_custom_tags WHERE media_id=?1",
            [media_id],
        )?;
        for id in tag_ids {
            transaction.execute("INSERT INTO media_custom_tags(media_id,tag_id) VALUES(?1,?2) ON CONFLICT(media_id,tag_id) DO NOTHING", params![media_id,id])?;
        }
        transaction.commit()?;
        Ok(())
    }
    pub fn for_media(
        &self,
        media_id: &str,
        after_id: Option<&str>,
        limit: u32,
    ) -> AppResult<Vec<CustomTag>> {
        let mut statement = self.connection.prepare("SELECT t.id,t.name,t.created_at,t.updated_at FROM custom_tags t JOIN media_custom_tags mt ON mt.tag_id=t.id WHERE mt.media_id=?1 AND t.id>coalesce(?2,'') ORDER BY t.id LIMIT ?3")?;
        let rows =
            statement.query_map(params![media_id, after_id, page_size(limit)?], custom_tag)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
    pub fn media_ids(
        &self,
        tag_id: &str,
        after_id: Option<&str>,
        limit: u32,
    ) -> AppResult<Vec<String>> {
        let mut statement = self.connection.prepare("SELECT media_id FROM media_custom_tags WHERE tag_id=?1 AND media_id>coalesce(?2,'') ORDER BY media_id LIMIT ?3")?;
        let rows =
            statement.query_map(params![tag_id, after_id, page_size(limit)?], |r| r.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

impl SystemTagRepository<'_> {
    /// Internal Rust persistence seam for a future scanner's real observations.
    /// Never expose this method as a user-facing Tauri command.
    pub fn record_observed_directory(&mut self, directory: &ObservedDirectory) -> AppResult<()> {
        for value in [
            &directory.id,
            &directory.source_root,
            &directory.directory_path,
            &directory.name,
        ] {
            nonempty(value)?;
        }
        let path = std::path::Path::new(&directory.directory_path);
        let root = std::path::Path::new(&directory.source_root);
        if !is_valid_directory_observation(root, path) {
            return Err(AppError::Validation(
                "directory observation must be inside an absolute root".into(),
            ));
        }
        if let Some(parent_id) = &directory.parent_id {
            let parent = self.get(parent_id)?.ok_or(AppError::NotFound)?;
            if parent.source_root != directory.source_root
                || path.parent() != Some(std::path::Path::new(&parent.directory_path))
            {
                return Err(AppError::Validation(
                    "parent must be the observed physical parent directory".into(),
                ));
            }
        } else if path != root {
            return Err(AppError::Validation(
                "a root tag must describe the root directory".into(),
            ));
        }
        self.connection.execute(
            "INSERT INTO system_tags(id,parent_id,source_root,directory_path,name,last_seen_at) VALUES(?1,?2,?3,?4,?5,?6)
             ON CONFLICT(id) DO UPDATE SET parent_id=excluded.parent_id,source_root=excluded.source_root,directory_path=excluded.directory_path,name=excluded.name,last_seen_at=excluded.last_seen_at",
            params![directory.id,directory.parent_id,directory.source_root,directory.directory_path,directory.name,directory.last_seen_at],
        )?;
        Ok(())
    }
    pub fn get(&self, id: &str) -> AppResult<Option<SystemTag>> {
        Ok(self.connection.query_row("SELECT id,parent_id,source_root,directory_path,name,last_seen_at FROM system_tags WHERE id=?1", [id], system_tag).optional()?)
    }
    pub fn children(
        &self,
        parent_id: Option<&str>,
        after_id: Option<&str>,
        limit: u32,
    ) -> AppResult<Vec<SystemTag>> {
        let mut statement = self.connection.prepare("SELECT id,parent_id,source_root,directory_path,name,last_seen_at FROM system_tags WHERE parent_id IS ?1 AND id>coalesce(?2,'') ORDER BY id LIMIT ?3")?;
        let rows =
            statement.query_map(params![parent_id, after_id, page_size(limit)?], system_tag)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
    pub fn for_media(
        &self,
        media_id: &str,
        after_id: Option<&str>,
        limit: u32,
    ) -> AppResult<Vec<SystemTag>> {
        let mut statement = self.connection.prepare("SELECT t.id,t.parent_id,t.source_root,t.directory_path,t.name,t.last_seen_at FROM system_tags t JOIN media_system_tags mt ON mt.tag_id=t.id WHERE mt.media_id=?1 AND t.id>coalesce(?2,'') ORDER BY t.id LIMIT ?3")?;
        let rows =
            statement.query_map(params![media_id, after_id, page_size(limit)?], system_tag)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
    pub fn replace_observed_media_tags(
        &mut self,
        media_id: &str,
        tag_ids: &[String],
    ) -> AppResult<()> {
        let transaction = self.connection.transaction()?;
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM media WHERE id=?1)",
            [media_id],
            |r| r.get(0),
        )?;
        if !exists {
            return Err(crate::error::AppError::NotFound);
        }
        transaction.execute(
            "DELETE FROM media_system_tags WHERE media_id=?1",
            [media_id],
        )?;
        for id in tag_ids {
            transaction.execute("INSERT INTO media_system_tags(media_id,tag_id) VALUES(?1,?2) ON CONFLICT(media_id,tag_id) DO NOTHING", params![media_id,id])?;
        }
        transaction.commit()?;
        Ok(())
    }
}
