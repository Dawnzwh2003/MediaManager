use super::nonempty;
use crate::error::AppResult;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

pub struct SettingsRepository<'a> {
    pub(crate) connection: &'a mut Connection,
}

impl SettingsRepository<'_> {
    pub fn get(&self, key: &str) -> AppResult<Option<Value>> {
        let json: Option<String> = self
            .connection
            .query_row(
                "SELECT value_json FROM app_settings WHERE key=?1",
                [key],
                |r| r.get(0),
            )
            .optional()?;
        Ok(json.map(|value| serde_json::from_str(&value)).transpose()?)
    }
    pub fn set(&mut self, key: &str, value: &Value, updated_at: i64) -> AppResult<()> {
        nonempty(key)?;
        self.connection.execute(
            "INSERT INTO app_settings(key,value_json,updated_at) VALUES(?1,?2,?3)
             ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json,updated_at=excluded.updated_at",
            params![key,value.to_string(),updated_at],
        )?;
        Ok(())
    }
    pub fn delete(&mut self, key: &str) -> AppResult<()> {
        self.connection
            .execute("DELETE FROM app_settings WHERE key=?1", [key])?;
        Ok(())
    }
}
