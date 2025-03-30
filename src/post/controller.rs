use crate::auth::middleware::AuthUser;
use crate::cache::redis::RedisCache;
use crate::post::model::{CreatePostRequest, UpdatePostRequest};
use crate::post::service::{PostError as ServiceError, PostService};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    Extension,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{error, info};
use utoipa::ToSchema;

#[derive(Debug, Deserialize)]
pub struct IdOrSlugPathParam {
    id_or_slug: String,
}

#[derive(Debug, Deserialize)]
pub struct PostIdPathParam {
    id: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PopularPostsParams {
    /// Maximum number of posts to retrieve
    #[schema(example = "10", default = "10", minimum = 1, maximum = 100)]
    limit: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
}

/// Create a new blog post
///
/// Creates a new blog post with the provided data and associates it with the authenticated user.
#[utoipa::path(
    post,
    path = "/api/posts",
    request_body = CreatePostRequest,
    responses(
        (status = 201, description = "Post created successfully", body = PostResponse),
        (status = 400, description = "Invalid request data", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 409, description = "Conflict - slug or title already exists", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "posts"
)]
pub async fn create_post(
    user: AuthUser,
    State((pool, redis_cache)): State<(PgPool, Option<RedisCache>)>,
    Json(post_data): Json<CreatePostRequest>,
) -> Response {
    info!("Creating post with title: {}", post_data.title);

    let service = PostService::new(pool, redis_cache);

    // Use the UUID directly instead of converting to i64
    let user_id = user.user_id;

    match service.create_post(user_id, post_data).await {
        Ok(post) => {
            // Get the complete post with author info and tags
            match service.get_post_by_id(post.id).await {
                Ok(post_response) => {
                    info!("Successfully created post with ID: {}", post.id);
                    (StatusCode::CREATED, Json(post_response)).into_response()
                }
                Err(e) => {
                    error!("Error retrieving created post: {:?}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: "Error retrieving created post".to_string(),
                            code: "INTERNAL_ERROR".to_string(),
                        }),
                    )
                        .into_response()
                }
            }
        }
        Err(e) => {
            error!("Error creating post: {:?}", e);
            let (status, error_response) = match e {
                ServiceError::SlugExists => (
                    StatusCode::CONFLICT,
                    ErrorResponse {
                        error: "Post with this slug already exists".to_string(),
                        code: "SLUG_EXISTS".to_string(),
                    },
                ),
                ServiceError::TitleExists => (
                    StatusCode::CONFLICT,
                    ErrorResponse {
                        error: "Post with this title already exists".to_string(),
                        code: "TITLE_EXISTS".to_string(),
                    },
                ),
                ServiceError::InvalidInput(msg) => (
                    StatusCode::BAD_REQUEST,
                    ErrorResponse {
                        error: msg,
                        code: "INVALID_INPUT".to_string(),
                    },
                ),
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorResponse {
                        error: "Failed to create post".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    },
                ),
            };

            (status, Json(error_response)).into_response()
        }
    }
}

/// Get post by ID or slug
///
/// Retrieves a post by its ID (numeric) or slug (string)
#[utoipa::path(
    get,
    path = "/api/posts/view/{id_or_slug}",
    params(
        ("id_or_slug" = String, Path, description = "Post ID or slug")
    ),
    responses(
        (status = 200, description = "Post retrieved successfully", body = PostResponse),
        (status = 404, description = "Post not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "posts"
)]
pub async fn get_post(
    Extension(_user): Extension<Option<AuthUser>>,
    Path(params): Path<IdOrSlugPathParam>,
    State((pool, redis_cache)): State<(PgPool, Option<RedisCache>)>,
) -> Response {
    let id_or_slug = params.id_or_slug;
    info!("Getting post with ID/slug: {}", id_or_slug);

    let service = PostService::new(pool, redis_cache);

    // Check if the parameter is an ID (numeric) or slug (string)
    let result = if let Ok(id) = id_or_slug.parse::<i64>() {
        service.get_post_by_id(id).await
    } else {
        service.get_post_by_slug(&id_or_slug).await
    };

    match result {
        Ok(post) => {
            info!("Successfully retrieved post with ID: {}", post.id);
            (StatusCode::OK, Json(post)).into_response()
        }
        Err(e) => {
            error!("Error retrieving post: {:?}", e);
            let (status, error_response) = match e {
                ServiceError::NotFound => (
                    StatusCode::NOT_FOUND,
                    ErrorResponse {
                        error: "Post not found".to_string(),
                        code: "NOT_FOUND".to_string(),
                    },
                ),
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorResponse {
                        error: "Failed to retrieve post".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    },
                ),
            };

            (status, Json(error_response)).into_response()
        }
    }
}

/// Update post
///
/// Updates an existing post with the provided data. User must be the post owner or an admin.
#[utoipa::path(
    put,
    path = "/api/posts/edit/{id}",
    params(
        ("id" = i64, Path, description = "Post ID")
    ),
    request_body = UpdatePostRequest,
    responses(
        (status = 200, description = "Post updated successfully", body = PostResponse),
        (status = 400, description = "Invalid request data", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden - user is not the post owner or admin", body = ErrorResponse),
        (status = 404, description = "Post not found", body = ErrorResponse),
        (status = 409, description = "Conflict - slug or title already exists", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "posts"
)]
pub async fn update_post(
    user: AuthUser,
    Path(params): Path<PostIdPathParam>,
    State((pool, redis_cache)): State<(PgPool, Option<RedisCache>)>,
    Json(update_data): Json<UpdatePostRequest>,
) -> Response {
    info!("Updating post with ID: {}", params.id);

    let service = PostService::new(pool, redis_cache);

    // Use the UUID directly instead of converting to i64
    let user_id = user.user_id;

    match service.update_post(params.id, user_id, update_data).await {
        Ok(post) => {
            info!("Successfully updated post with ID: {}", params.id);
            (StatusCode::OK, Json(post)).into_response()
        }
        Err(e) => {
            error!("Error updating post: {:?}", e);
            let (status, error_response) = match e {
                ServiceError::NotFound => (
                    StatusCode::NOT_FOUND,
                    ErrorResponse {
                        error: "Post not found".to_string(),
                        code: "NOT_FOUND".to_string(),
                    },
                ),
                ServiceError::Unauthorized => (
                    StatusCode::FORBIDDEN,
                    ErrorResponse {
                        error: "You do not have permission to update this post".to_string(),
                        code: "FORBIDDEN".to_string(),
                    },
                ),
                ServiceError::SlugExists => (
                    StatusCode::CONFLICT,
                    ErrorResponse {
                        error: "Post with this slug already exists".to_string(),
                        code: "SLUG_EXISTS".to_string(),
                    },
                ),
                ServiceError::TitleExists => (
                    StatusCode::CONFLICT,
                    ErrorResponse {
                        error: "Post with this title already exists".to_string(),
                        code: "TITLE_EXISTS".to_string(),
                    },
                ),
                ServiceError::InvalidInput(msg) => (
                    StatusCode::BAD_REQUEST,
                    ErrorResponse {
                        error: msg,
                        code: "INVALID_INPUT".to_string(),
                    },
                ),
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorResponse {
                        error: "Failed to update post".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    },
                ),
            };

            (status, Json(error_response)).into_response()
        }
    }
}

/// Delete post
///
/// Deletes (soft delete) an existing post. User must be the post owner or an admin.
#[utoipa::path(
    delete,
    path = "/api/posts/delete/{id}",
    params(
        ("id" = i64, Path, description = "Post ID")
    ),
    responses(
        (status = 204, description = "Post deleted successfully"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden - user is not the post owner or admin", body = ErrorResponse),
        (status = 404, description = "Post not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "posts"
)]
pub async fn delete_post(
    user: AuthUser,
    Path(params): Path<PostIdPathParam>,
    State((pool, redis_cache)): State<(PgPool, Option<RedisCache>)>,
) -> Response {
    info!("Deleting post with ID: {}", params.id);

    let service = PostService::new(pool, redis_cache);

    // Use the UUID directly instead of converting to i64
    let user_id = user.user_id;

    match service.delete_post(params.id, user_id).await {
        Ok(_) => {
            info!("Successfully deleted post with ID: {}", params.id);
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => {
            error!("Error deleting post: {:?}", e);
            let (status, error_response) = match e {
                ServiceError::NotFound => (
                    StatusCode::NOT_FOUND,
                    ErrorResponse {
                        error: "Post not found".to_string(),
                        code: "NOT_FOUND".to_string(),
                    },
                ),
                ServiceError::Unauthorized => (
                    StatusCode::FORBIDDEN,
                    ErrorResponse {
                        error: "You do not have permission to delete this post".to_string(),
                        code: "FORBIDDEN".to_string(),
                    },
                ),
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorResponse {
                        error: "Failed to delete post".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    },
                ),
            };

            (status, Json(error_response)).into_response()
        }
    }
}

/// Get popular posts
///
/// Retrieves a list of the most popular posts based on views and engagement
#[utoipa::path(
    get,
    path = "/api/posts/popular",
    params(
        ("limit" = Option<i64>, Query, description = "Maximum number of posts to retrieve", example = "10")
    ),
    responses(
        (status = 200, description = "Popular posts retrieved successfully", body = PopularPostsResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "posts"
)]
pub async fn get_popular_posts(
    Extension(_user): Extension<Option<AuthUser>>,
    State((pool, redis_cache)): State<(PgPool, Option<RedisCache>)>,
    Query(params): Query<PopularPostsParams>,
) -> Response {
    let limit = params.limit.unwrap_or(10);
    info!("Getting popular posts, limit: {}", limit);

    let service = PostService::new(pool, redis_cache);

    match service.get_popular_posts(limit).await {
        Ok(posts) => {
            info!("Successfully retrieved {} popular posts", posts.len());
            (StatusCode::OK, Json(posts)).into_response()
        }
        Err(e) => {
            error!("Error retrieving popular posts: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to retrieve popular posts".to_string(),
                    code: "INTERNAL_ERROR".to_string(),
                }),
            )
                .into_response()
        }
    }
}
