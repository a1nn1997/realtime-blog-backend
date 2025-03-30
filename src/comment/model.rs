use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use thiserror;
use utoipa::ToSchema;
use uuid::Uuid;

/// Database model for a comment
#[derive(Debug, FromRow, Clone)]
pub struct Comment {
    pub id: i64,
    pub post_id: i64,
    pub user_id: Uuid,
    pub parent_comment_id: Option<i64>,
    pub content: String,
    pub content_html: String,
    pub is_deleted: bool,
    pub deleted_by: Option<Uuid>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub markdown_enabled: bool,
    pub nesting_level: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request to create a new comment
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateCommentRequest {
    /// The comment content in markdown or plain text
    #[schema(example = "This is a great post!")]
    pub content: String,

    /// ID of the parent comment if this is a reply
    #[schema(example = "null")]
    pub parent_comment_id: Option<i64>,

    /// Whether markdown is enabled for this comment
    #[schema(example = "true")]
    pub markdown_enabled: bool,
}

/// User information in comment responses
#[derive(Debug, Serialize, Deserialize, FromRow, ToSchema)]
pub struct CommentAuthor {
    /// User's UUID
    #[schema(value_type = UuidWrapper)]
    #[schema(example = "a1b2c3d4-e5f6-7890-abcd-1234567890ab")]
    pub id: Uuid,

    /// User's display name
    #[schema(example = "John Doe")]
    pub name: String,
}

/// Response format for a single comment
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CommentResponse {
    /// Comment ID
    #[schema(example = "123")]
    pub id: i64,

    /// HTML rendered content
    #[schema(example = "<p>This is a great post!</p>")]
    pub content_html: String,

    /// Author information
    pub author: CommentAuthor,

    /// When the comment was created
    #[schema(value_type = DateTimeWrapper)]
    #[schema(example = "2023-01-01T12:00:00Z")]
    pub created_at: DateTime<Utc>,

    /// Parent comment ID if this is a reply
    #[schema(example = "null")]
    pub parent_comment_id: Option<i64>,

    /// Nested replies
    pub replies: Option<Vec<CommentResponse>>,
}

/// Response for a list of comments
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CommentsListResponse {
    /// List of comments
    pub comments: Vec<CommentResponse>,

    /// Total number of comments
    #[schema(example = "42")]
    pub total_count: i64,
}

/// Possible comment errors
#[derive(Debug, thiserror::Error)]
pub enum CommentError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Comment not found")]
    NotFound,

    #[error("Post not found")]
    PostNotFound,

    #[error("Not authorized to perform this action")]
    Unauthorized,

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Invalid comment")]
    InvalidComment,

    #[error("Maximum nesting depth reached")]
    MaxNestingDepthReached,

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Parent comment not found")]
    ParentCommentNotFound,

    #[error("Cache error: {0}")]
    CacheError(#[from] redis::RedisError),

    #[error("Deserialization error")]
    DeserializationError,

    #[error("Internal server error: {0}")]
    InternalError(String),
}

/// Error response for the API
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CommentErrorResponse {
    /// Error message
    #[schema(example = "Comment not found")]
    pub error: String,

    /// Error code
    #[schema(example = "NOT_FOUND")]
    pub code: String,
}

impl From<CommentError> for CommentErrorResponse {
    fn from(err: CommentError) -> Self {
        match err {
            CommentError::NotFound => Self {
                error: "Comment not found".to_string(),
                code: "NOT_FOUND".to_string(),
            },
            CommentError::PostNotFound => Self {
                error: "Post not found".to_string(),
                code: "POST_NOT_FOUND".to_string(),
            },
            CommentError::Unauthorized => Self {
                error: "Not authorized to perform this action".to_string(),
                code: "UNAUTHORIZED".to_string(),
            },
            CommentError::RateLimitExceeded => Self {
                error: "Rate limit exceeded".to_string(),
                code: "RATE_LIMIT_EXCEEDED".to_string(),
            },
            CommentError::InvalidComment => Self {
                error: "Invalid comment".to_string(),
                code: "INVALID_COMMENT".to_string(),
            },
            CommentError::MaxNestingDepthReached => Self {
                error: "Maximum nesting depth reached for comments".to_string(),
                code: "MAX_DEPTH".to_string(),
            },
            CommentError::ValidationError(msg) => Self {
                error: msg,
                code: "VALIDATION_ERROR".to_string(),
            },
            CommentError::ParentCommentNotFound => Self {
                error: "Parent comment not found".to_string(),
                code: "PARENT_NOT_FOUND".to_string(),
            },
            CommentError::CacheError(_) => Self {
                error: "Internal server error".to_string(),
                code: "INTERNAL_ERROR".to_string(),
            },
            CommentError::DatabaseError(_) => Self {
                error: "Internal server error".to_string(),
                code: "INTERNAL_ERROR".to_string(),
            },
            CommentError::DeserializationError => Self {
                error: "Failed to process comment data".to_string(),
                code: "DESERIALIZATION_ERROR".to_string(),
            },
            CommentError::InternalError(msg) => Self {
                error: msg,
                code: "INTERNAL_ERROR".to_string(),
            },
        }
    }
}
