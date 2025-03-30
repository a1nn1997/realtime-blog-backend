use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationType {
    CommentReply,
    NewComment,
    PostLike,
    FollowerUpdate,
    SystemMessage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationPayload {
    pub recipient_id: Uuid,
    pub notification_type: NotificationType,
    pub object_id: i64,
    pub related_object_id: Option<i64>,
    pub actor_id: Uuid,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: i64,
    pub recipient_id: Uuid,
    pub notification_type: NotificationType,
    pub object_id: i64,
    pub related_object_id: Option<i64>,
    pub actor_id: Uuid,
    pub content: String,
    pub is_read: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum NotificationError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Cache error: {0}")]
    CacheError(#[from] redis::RedisError),

    #[error("Notification not found")]
    NotFound,

    #[error("Internal error: {0}")]
    InternalError(String),
}
