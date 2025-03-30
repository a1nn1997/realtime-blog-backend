use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// Enum for different types of user interactions
#[derive(Debug, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[sqlx(type_name = "VARCHAR", rename_all = "lowercase")]
pub enum InteractionType {
    View,
    Like,
    Comment,
    Share,
    Bookmark,
}

impl std::fmt::Display for InteractionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InteractionType::View => write!(f, "view"),
            InteractionType::Like => write!(f, "like"),
            InteractionType::Comment => write!(f, "comment"),
            InteractionType::Share => write!(f, "share"),
            InteractionType::Bookmark => write!(f, "bookmark"),
        }
    }
}

/// User interaction record
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct UserInteraction {
    pub id: i64,
    pub user_id: Option<Uuid>,
    pub interaction_type: String,
    pub post_id: Option<i64>,
    pub comment_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
}

/// User engagement metrics
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct UserEngagement {
    #[schema(value_type = String, format = "uuid", example = "123e4567-e89b-12d3-a456-426614174000")]
    pub user_id: Uuid,
    pub views: i64,
    pub likes: i64,
    pub comments: i64,
    pub total_interactions: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(nullable = true, value_type = String, format = "date-time", example = "2025-03-26T12:00:00Z")]
    pub day: Option<DateTime<Utc>>,
}

/// Post statistics
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct PostStats {
    pub post_id: i64,
    pub views: i64,
    pub likes: i64,
    pub comments: i64,
    pub total_interactions: i64,
    pub engagement_rate: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(nullable = true, value_type = String, format = "date-time", example = "2025-03-26T12:00:00Z")]
    pub day: Option<DateTime<Utc>>,
}

/// Time range for analytics queries
#[derive(Debug, Serialize, Deserialize)]
pub enum TimeRange {
    Day,
    Week,
    Month,
    Year,
    Custom {
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    },
}

/// Query parameters for engagement analytics
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[into_params(style = Form)]
pub struct EngagementParams {
    /// Time range: "day", "week", "month", "year"
    #[schema(example = "day", default = "month")]
    pub time_range: Option<String>,

    /// Start date for custom range (format: YYYY-MM-DD)
    #[schema(value_type = String, format = "date", example = "2025-03-19")]
    pub start_date: Option<String>,

    /// End date for custom range (format: YYYY-MM-DD)
    #[schema(value_type = String, format = "date", example = "2025-03-26")]
    pub end_date: Option<String>,

    /// Maximum number of results
    #[schema(example = "100", default = "100", minimum = 1, maximum = 1000)]
    pub limit: Option<i64>,

    /// Offset for pagination
    #[schema(example = "0", default = "0", minimum = 0)]
    pub offset: Option<i64>,
}

/// Query parameters for post statistics
#[derive(Debug, Deserialize, Clone, ToSchema, IntoParams)]
#[into_params(style = Form)]
pub struct PostStatsParams {
    /// Specific post ID to get stats for
    #[schema(example = "123")]
    pub post_id: Option<i64>,

    /// Time range: "day", "week", "month", "year"
    #[schema(example = "week", default = "month")]
    pub time_range: Option<String>,

    /// Start date for custom range (format: YYYY-MM-DD)
    #[schema(value_type = String, format = "date", example = "2025-03-19")]
    pub start_date: Option<String>,

    /// End date for custom range (format: YYYY-MM-DD)
    #[schema(value_type = String, format = "date", example = "2025-03-26")]
    pub end_date: Option<String>,

    /// Maximum number of results
    #[schema(example = "100", default = "100", minimum = 1, maximum = 1000)]
    pub limit: Option<i64>,

    /// Offset for pagination
    #[schema(example = "0", default = "0", minimum = 0)]
    pub offset: Option<i64>,
}

/// Error types for analytics operations
#[derive(Debug, thiserror::Error)]
pub enum AnalyticsError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Cache error: {0}")]
    CacheError(#[from] redis::RedisError),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Not found")]
    NotFound,

    #[error("Unauthorized")]
    Unauthorized,
}
