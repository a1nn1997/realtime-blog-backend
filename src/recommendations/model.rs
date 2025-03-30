use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// Recommendation record for a user
#[derive(Debug, Serialize, Deserialize, FromRow, Clone, ToSchema)]
pub struct Recommendation {
    pub id: i64,
    pub user_id: Uuid,
    pub post_id: i64,
    pub score: f64,
    pub recommendation_type: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// Recommendation response for API
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RecommendationResponse {
    pub post_id: i64,
    pub score: f64,
    pub title: String,
    pub tags: Vec<String>,
}

/// Post recommendation model for API documentation
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PostRecommendation {
    pub post_id: i64,
    pub title: String,
    pub score: f64,
    pub similarity: Option<f64>,
    pub author: String,
    #[schema(value_type = String, format = "date-time", example = "2025-03-26T12:00:00Z")]
    pub created_at: DateTime<Utc>,
    pub tags: Vec<String>,
    pub excerpt: Option<String>,
}

/// Parameters for recommendation requests
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[into_params(style = Form)]
pub struct RecommendationParams {
    /// Maximum number of recommendations
    #[schema(example = "10", default = "10", minimum = 1, maximum = 100)]
    pub limit: Option<i64>,

    /// Offset for pagination
    #[schema(example = "0", default = "0", minimum = 0)]
    pub offset: Option<i64>,

    /// Algorithm to use: "collaborative", "content_based", "hybrid", "popular"
    #[schema(example = "hybrid")]
    pub algorithm: Option<String>,

    /// Tags to include in recommendations (comma-separated)
    #[schema(example = "rust,programming,webdev")]
    pub include_tags: Option<Vec<String>>,

    /// Tags to exclude from recommendations (comma-separated)
    #[schema(example = "deprecated,outdated")]
    pub exclude_tags: Option<Vec<String>>,

    /// Minimum score threshold
    #[schema(example = "0.5", minimum = 0.0, maximum = 1.0)]
    pub min_score: Option<f64>,
}

/// Request to generate recommendations
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct GenerateRecommendationsRequest {
    /// Optional list of specific users (UUIDs)
    #[schema(example = "[\"cede8df7-2893-4186-8948-2b1ee463af68\"]")]
    pub user_ids: Option<Vec<Uuid>>,

    /// Maximum recommendations per user
    #[schema(example = "20", default = "10", minimum = 1, maximum = 100)]
    pub limit_per_user: Option<i64>,

    /// Algorithm to use: "collaborative", "content_based", "popular"
    #[schema(example = "hybrid")]
    pub algorithm: Option<String>,

    /// Whether to replace existing recommendations
    #[schema(example = "true", default = "false")]
    pub refresh_existing: Option<bool>,
}

/// Error types for recommendation operations
#[derive(Debug, thiserror::Error)]
pub enum RecommendationError {
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

    #[error("Generation in progress")]
    GenerationInProgress,
}
