use crate::auth::middleware::AuthUser;
use crate::comment::model::{
    CommentError, CommentErrorResponse, CommentsListResponse, CreateCommentRequest,
};
use crate::comment::service::CommentService;
use axum::http::header::HeaderMap;
use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use tracing::{error, info};
use utoipa::{IntoParams, ToSchema};

// Query parameters for pagination
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct CommentsQueryParams {
    #[schema(example = "1")]
    page: Option<i64>,
}

// Helper function to convert CommentError to HTTP response
fn comment_error_to_response(err: CommentError) -> (StatusCode, Json<CommentErrorResponse>) {
    let (status, error_message, code) = match err {
        CommentError::DatabaseError(e) => {
            error!("Database error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Database error",
                "DB_ERROR",
            )
        }
        CommentError::CacheError(e) => {
            error!("Cache error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Cache error",
                "CACHE_ERROR",
            )
        }
        CommentError::NotFound => (StatusCode::NOT_FOUND, "Comment not found", "NOT_FOUND"),
        CommentError::PostNotFound => (StatusCode::NOT_FOUND, "Post not found", "POST_NOT_FOUND"),
        CommentError::ParentCommentNotFound => (
            StatusCode::NOT_FOUND,
            "Parent comment not found",
            "PARENT_NOT_FOUND",
        ),
        CommentError::Unauthorized => (
            StatusCode::UNAUTHORIZED,
            "Not authorized to perform this action",
            "UNAUTHORIZED",
        ),
        CommentError::RateLimitExceeded => (
            StatusCode::TOO_MANY_REQUESTS,
            "Rate limit exceeded, please try again later",
            "RATE_LIMITED",
        ),
        CommentError::MaxNestingDepthReached => (
            StatusCode::BAD_REQUEST,
            "Maximum nesting depth reached for comments",
            "MAX_DEPTH",
        ),
        CommentError::ValidationError(_) => {
            (StatusCode::BAD_REQUEST, "Invalid input", "VALIDATION_ERROR")
        }
        CommentError::InvalidComment => (
            StatusCode::BAD_REQUEST,
            "Invalid comment",
            "INVALID_COMMENT",
        ),
        CommentError::DeserializationError => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to process comment data",
            "DESERIALIZATION_ERROR",
        ),
        CommentError::InternalError(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error",
            "INTERNAL_SERVER_ERROR",
        ),
    };

    let error_response = CommentErrorResponse {
        error: error_message.to_string(),
        code: code.to_string(),
    };

    (status, Json(error_response))
}

/// Create a new comment for a post
///
/// This endpoint allows authenticated users to add a comment to a specific post.
#[utoipa::path(
    post,
    path = "/api/posts/{id}/comments",
    tag = "comments",
    params(
        ("id" = i64, Path, description = "The ID of the post to comment on")
    ),
    request_body = CreateCommentRequest,
    responses(
        (status = 201, description = "Comment created successfully", body = CommentResponse),
        (status = 400, description = "Invalid input", body = CommentErrorResponse),
        (status = 401, description = "Unauthorized", body = CommentErrorResponse),
        (status = 404, description = "Post not found", body = CommentErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = CommentErrorResponse),
        (status = 500, description = "Internal server error", body = CommentErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn create_comment(
    Path(post_id): Path<i64>,
    Extension(user): Extension<AuthUser>,
    Extension(comment_service): Extension<Arc<CommentService>>,
    Json(comment_data): Json<CreateCommentRequest>,
) -> impl IntoResponse {
    info!(
        "Creating comment for post: {}, user: {}",
        post_id, user.user_id
    );

    // Validate input
    if comment_data.content.trim().is_empty() {
        return comment_error_to_response(CommentError::ValidationError(
            "Comment content cannot be empty".to_string(),
        ))
        .into_response();
    }

    if comment_data.content.len() > 5000 {
        return comment_error_to_response(CommentError::ValidationError(
            "Comment content exceeds maximum length".to_string(),
        ))
        .into_response();
    }

    match comment_service
        .create_comment(post_id, user.user_id, comment_data)
        .await
    {
        Ok(comment) => {
            info!("Successfully created comment with ID: {}", comment.id);
            (StatusCode::CREATED, Json(comment)).into_response()
        }
        Err(e) => comment_error_to_response(e).into_response(),
    }
}

/// Get comments for a post
///
/// This endpoint retrieves all comments for a specific post, with optional pagination.
#[utoipa::path(
    get,
    path = "/api/posts/{id}/comments",
    tag = "comments",
    params(
        ("id" = i64, Path, description = "The ID of the post to get comments for"),
        ("page" = Option<i64>, Query, description = "Page number for pagination", example = "1")
    ),
    responses(
        (status = 200, description = "Comments retrieved successfully", body = CommentsListResponse),
        (status = 404, description = "Post not found", body = CommentErrorResponse),
        (status = 500, description = "Internal server error", body = CommentErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_post_comments(
    Path(post_id): Path<i64>,
    Extension(comment_service): Extension<Arc<CommentService>>,
    Query(params): Query<CommentsQueryParams>,
) -> Result<(StatusCode, Json<CommentsListResponse>), (StatusCode, Json<CommentErrorResponse>)> {
    info!("Getting comments for post: {}", post_id);

    match comment_service
        .get_post_comments(post_id, params.page, true)
        .await
    {
        Ok(comments) => {
            let total_count = match comment_service.get_comment_count(post_id).await {
                Ok(count) => count,
                Err(e) => {
                    error!("Error getting comment count: {:?}", e);
                    0
                }
            };

            let response = CommentsListResponse {
                comments,
                total_count,
            };

            Ok((StatusCode::OK, Json(response)))
        }
        Err(err) => {
            error!("Error getting comments: {:?}", err);
            Err(comment_error_to_response(err))
        }
    }
}

/// Delete a comment
///
/// This endpoint allows users to delete their own comments or admins to delete any comment.
#[utoipa::path(
    delete,
    path = "/api/comments/{id}",
    tag = "comments",
    params(
        ("id" = i64, Path, description = "The ID of the comment to delete")
    ),
    responses(
        (status = 204, description = "Comment deleted successfully"),
        (status = 401, description = "Unauthorized", body = CommentErrorResponse),
        (status = 404, description = "Comment not found", body = CommentErrorResponse),
        (status = 500, description = "Internal server error", body = CommentErrorResponse)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn delete_comment(
    Path(comment_id): Path<i64>,
    Extension(user): Extension<AuthUser>,
    Extension(comment_service): Extension<Arc<CommentService>>,
    _headers: HeaderMap,
) -> impl IntoResponse {
    info!(
        "Deleting comment: {}, requested by user: {}",
        comment_id, user.user_id
    );

    // Check if user is admin (in a real app, this would use a proper role system)
    let is_admin = user.role == crate::auth::jwt::Role::Admin;

    match comment_service
        .delete_comment(comment_id, user.user_id, is_admin)
        .await
    {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => comment_error_to_response(e).into_response(),
    }
}
