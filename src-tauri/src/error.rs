use serde::{Serialize, Serializer};
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("filesystem operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("database operation failed: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("invalid repository input: {0}")]
    Validation(String),
    #[error("record was not found")]
    NotFound,
    #[error("library cursor is stale or belongs to another query")]
    StaleCursor,
    #[error("playback state owner, sequence or version is stale")]
    StaleState,
    #[error("media resource is unavailable: {0}")]
    MediaUnavailable(String),
    #[error("invalid persisted JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("migration validation failed: {0}")]
    Migration(String),
    #[error("unsafe application storage: {0}")]
    Storage(String),
    #[error("another application instance is using this database")]
    Busy,
    #[error("internal application state is unavailable")]
    Internal,
}

#[derive(Serialize)]
pub struct ErrorPayload {
    pub code: &'static str,
    pub message: &'static str,
    pub retryable: bool,
}

impl AppError {
    pub fn payload(&self) -> ErrorPayload {
        let (code, message, retryable) = match self {
            Self::Validation(_) => ("invalid_input", "输入的数据不符合约束。", false),
            Self::NotFound => ("not_found", "记录不存在。", false),
            Self::StaleCursor => ("stale_cursor", "媒体库已更新，请刷新当前列表。", true),
            Self::StaleState => (
                "stale_state",
                "播放状态已由更新的会话接管，请重新打开媒体。",
                false,
            ),
            Self::MediaUnavailable(_) => (
                "media_unavailable",
                "媒体资源无法读取或转换失败，请重试。",
                true,
            ),
            Self::Json(_) => ("invalid_data", "保存的数据格式不正确。", false),
            Self::Io(_) => ("storage_unavailable", "无法访问应用运行目录。", true),
            Self::Storage(_) => (
                "invalid_storage",
                "应用运行路径不是安全的目录或文件。",
                false,
            ),
            Self::Migration(_) => (
                "migration_failed",
                "数据库版本校验或升级失败，原有数据已保留。",
                false,
            ),
            Self::Busy => ("busy", "已有应用实例正在使用数据库，请关闭后重试。", true),
            Self::Database(rusqlite::Error::SqliteFailure(error, _))
                if error.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                (
                    "constraint_violation",
                    "数据违反唯一性、类型或关联约束。",
                    false,
                )
            }
            Self::Database(rusqlite::Error::SqliteFailure(error, _))
                if matches!(
                    error.code,
                    rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
                ) =>
            {
                ("busy", "数据库正在处理其他请求，请重试。", true)
            }
            Self::Database(_) => (
                "database_unavailable",
                "数据库无法读取，未创建替代数据库。",
                false,
            ),
            Self::Internal => ("internal", "基础服务暂时不可用，请重新启动应用。", false),
        };
        ErrorPayload {
            code,
            message,
            retryable,
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.payload().serialize(serializer)
    }
}
