use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, FromRow, Clone, ToSchema)]
pub struct Post {
    pub id: i64,
    pub title: String,
    pub slug: String,
    pub content: String,
    pub content_html: String,
    #[schema(value_type = UuidWrapper)]
    pub user_id: Uuid,
    pub views: i64,
    pub likes: i64,
    pub is_draft: bool,
    pub is_deleted: bool,
    pub cover_image_url: Option<String>,
    #[schema(value_type = DateTimeWrapper)]
    pub created_at: DateTime<Utc>,
    #[schema(value_type = DateTimeWrapper)]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreatePostRequest {
    pub title: String,
    pub slug: String,
    pub content: String,
    pub tags: Vec<String>,
    pub cover_image_url: Option<String>,
    pub is_draft: bool,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdatePostRequest {
    pub title: Option<String>,
    pub slug: Option<String>,
    pub content: Option<String>,
    pub tags: Option<Vec<String>>,
    pub cover_image_url: Option<String>,
    pub is_draft: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PostResponse {
    pub id: i64,
    pub title: String,
    pub slug: String,
    pub content: String,
    pub content_html: String,
    pub author: UserBrief,
    pub tags: Vec<String>,
    pub views: i64,
    pub likes: i64,
    pub cover_image_url: Option<String>,
    pub is_draft: bool,
    #[schema(value_type = DateTimeWrapper)]
    pub created_at: DateTime<Utc>,
    #[schema(value_type = DateTimeWrapper)]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, FromRow, ToSchema)]
pub struct UserBrief {
    #[schema(value_type = UuidWrapper)]
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Tag {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PostError {
    pub error: String,
    pub code: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PopularPostsResponse {
    pub posts: Vec<PostResponse>,
}
