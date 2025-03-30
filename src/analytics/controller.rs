use crate::analytics::model::{
    AnalyticsError, EngagementParams, PostStats, PostStatsParams, UserEngagement,
};
use crate::analytics::service::AnalyticsService;
use crate::auth::jwt::Role;
use crate::auth::middleware::AuthUser;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    Extension,
};
use serde_json::json;
use std::sync::Arc;
use tracing::{error, info};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// Get user engagement metrics
#[utoipa::path(
    get,
    path = "/api/analytics/engagement",
    tag = "analytics",
    params(
        ("time_range" = Option<String>, Query, description = "Time range: day, week, month, year", example = "day"),
        ("start_date" = Option<String>, Query, description = "Start date for custom range (YYYY-MM-DD)", example = "2025-03-19"),
        ("end_date" = Option<String>, Query, description = "End date for custom range (YYYY-MM-DD)", example = "2025-03-26"),
        ("limit" = Option<i64>, Query, description = "Maximum number of results", example = "100"),
        ("offset" = Option<i64>, Query, description = "Offset for pagination", example = "0")
    ),
    responses(
        (status = 200, description = "User engagement metrics retrieved successfully", body = Vec<UserEngagement>),
        (status = 400, description = "Invalid parameters"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_user_engagement(
    Extension(user): Extension<AuthUser>,
    State(service): State<Arc<AnalyticsService>>,
    Query(params): Query<EngagementParams>,
) -> impl IntoResponse {
    match service.get_user_engagement(&params).await {
        Ok(engagement) => {
            info!("Retrieved user engagement for user: {}", user.user_id);
            (StatusCode::OK, Json(json!(engagement)))
        }
        Err(e) => {
            error!("Failed to get user engagement: {:?}", e);
            let status = match e {
                AnalyticsError::InvalidParameter(_) => StatusCode::BAD_REQUEST,
                AnalyticsError::NotFound => StatusCode::NOT_FOUND,
                AnalyticsError::Unauthorized => StatusCode::UNAUTHORIZED,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (
                status,
                Json(json!({
                    "error": format!("Failed to get user engagement: {}", e)
                })),
            )
        }
    }
}

/// Get engagement for a specific user
#[utoipa::path(
    get,
    path = "/api/analytics/engagement/user/{target_user_id}",
    tag = "analytics",
    params(
        ("target_user_id" = Uuid, Path, description = "User ID to get engagement for"),
        ("time_range" = Option<String>, Query, description = "Time range: day, week, month, year", example = "day"),
        ("start_date" = Option<String>, Query, description = "Start date for custom range (YYYY-MM-DD)", example = "2025-03-19"),
        ("end_date" = Option<String>, Query, description = "End date for custom range (YYYY-MM-DD)", example = "2025-03-26"),
        ("limit" = Option<i64>, Query, description = "Maximum number of results", example = "100"),
        ("offset" = Option<i64>, Query, description = "Offset for pagination", example = "0")
    ),
    responses(
        (status = 200, description = "User engagement metrics retrieved successfully", body = UserEngagement),
        (status = 400, description = "Invalid parameters"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_user_engagement_by_id(
    Extension(auth_user): Extension<AuthUser>,
    Path(target_user_id): Path<Uuid>,
    State(service): State<Arc<AnalyticsService>>,
    Query(params): Query<EngagementParams>,
) -> impl IntoResponse {
    // Check authorization - users can only see their own engagement
    // unless they're an admin/analyst
    if auth_user.user_id != target_user_id
        && auth_user.role != Role::Admin
        && auth_user.role != Role::Analyst
    {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "You are not authorized to view this user's engagement"
            })),
        );
    }

    match service
        .get_user_engagement_by_id(target_user_id, &params)
        .await
    {
        Ok(engagement) => {
            info!("Retrieved engagement for user: {}", target_user_id);
            (StatusCode::OK, Json(json!(engagement)))
        }
        Err(e) => {
            error!("Failed to get user engagement by ID: {:?}", e);
            let status = match e {
                AnalyticsError::InvalidParameter(_) => StatusCode::BAD_REQUEST,
                AnalyticsError::NotFound => StatusCode::NOT_FOUND,
                AnalyticsError::Unauthorized => StatusCode::UNAUTHORIZED,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (
                status,
                Json(json!({
                    "error": format!("Failed to get user engagement: {}", e)
                })),
            )
        }
    }
}

/// Get post statistics (public endpoint with optional auth)
#[utoipa::path(
    get,
    path = "/api/analytics/posts",
    tag = "analytics",
    params(
        ("post_id" = Option<i64>, Query, description = "Specific post ID to get stats for", example = "123"),
        ("time_range" = Option<String>, Query, description = "Time range: day, week, month, year", example = "week"),
        ("start_date" = Option<String>, Query, description = "Start date for custom range (YYYY-MM-DD)", example = "2025-03-19"),
        ("end_date" = Option<String>, Query, description = "End date for custom range (YYYY-MM-DD)", example = "2025-03-26"),
        ("limit" = Option<i64>, Query, description = "Maximum number of results", example = "100"),
        ("offset" = Option<i64>, Query, description = "Offset for pagination", example = "0")
    ),
    responses(
        (status = 200, description = "Post statistics retrieved successfully", body = Vec<PostStats>),
        (status = 400, description = "Invalid parameters"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_post_stats(
    _auth_user: Option<Extension<AuthUser>>,
    State(service): State<Arc<AnalyticsService>>,
    Query(params): Query<PostStatsParams>,
) -> impl IntoResponse {
    match service.get_post_stats(&params).await {
        Ok(stats) => {
            info!("Retrieved post statistics");
            (StatusCode::OK, Json(json!(stats)))
        }
        Err(e) => {
            error!("Failed to get post statistics: {:?}", e);
            let status = match e {
                AnalyticsError::InvalidParameter(_) => StatusCode::BAD_REQUEST,
                AnalyticsError::NotFound => StatusCode::NOT_FOUND,
                AnalyticsError::Unauthorized => StatusCode::UNAUTHORIZED,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (
                status,
                Json(json!({
                    "error": format!("Failed to get post statistics: {}", e)
                })),
            )
        }
    }
}

/// Get statistics for a specific post
#[utoipa::path(
    get,
    path = "/api/analytics/posts/{post_id}",
    tag = "analytics",
    params(
        ("post_id" = i64, Path, description = "Post ID to get statistics for"),
        ("time_range" = Option<String>, Query, description = "Time range: day, week, month, year", example = "week"),
        ("start_date" = Option<String>, Query, description = "Start date for custom range (YYYY-MM-DD)", example = "2025-03-19"),
        ("end_date" = Option<String>, Query, description = "End date for custom range (YYYY-MM-DD)", example = "2025-03-26"),
        ("limit" = Option<i64>, Query, description = "Maximum number of results", example = "100"),
        ("offset" = Option<i64>, Query, description = "Offset for pagination", example = "0")
    ),
    responses(
        (status = 200, description = "Post statistics retrieved successfully", body = PostStats),
        (status = 400, description = "Invalid parameters"),
        (status = 404, description = "Post not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_post_stats_by_id(
    _auth_user: Option<Extension<AuthUser>>,
    Path(post_id): Path<i64>,
    State(service): State<Arc<AnalyticsService>>,
    Query(params): Query<PostStatsParams>,
) -> impl IntoResponse {
    match service.get_post_stats_by_id(post_id, &params).await {
        Ok(stats) => {
            info!("Retrieved statistics for post: {}", post_id);
            (StatusCode::OK, Json(json!(stats)))
        }
        Err(e) => {
            error!("Failed to get statistics for post {}: {:?}", post_id, e);
            let status = match e {
                AnalyticsError::InvalidParameter(_) => StatusCode::BAD_REQUEST,
                AnalyticsError::NotFound => StatusCode::NOT_FOUND,
                AnalyticsError::Unauthorized => StatusCode::UNAUTHORIZED,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (
                status,
                Json(json!({
                    "error": format!("Failed to get post statistics: {}", e)
                })),
            )
        }
    }
}

/// Get time-based statistics for a post
#[utoipa::path(
    get,
    path = "/api/analytics/posts/{post_id}/time/{time_range}",
    tag = "analytics",
    params(
        ("post_id" = i64, Path, description = "Post ID to get statistics for"),
        ("time_range" = String, Path, description = "Time range (day, week, month, year)")
    ),
    responses(
        (status = 200, description = "Time-based statistics retrieved successfully", body = Vec<PostStats>),
        (status = 400, description = "Invalid parameters"),
        (status = 404, description = "Post not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_post_stats_by_time(
    _auth_user: Option<Extension<AuthUser>>,
    Path((post_id, time_range)): Path<(i64, String)>,
    State(service): State<Arc<AnalyticsService>>,
) -> impl IntoResponse {
    match service.get_post_stats_by_time(post_id, &time_range).await {
        Ok(stats) => {
            info!(
                "Retrieved time-based statistics for post {}: time range {}",
                post_id, time_range
            );
            (StatusCode::OK, Json(json!(stats)))
        }
        Err(e) => {
            error!(
                "Failed to get time-based statistics for post {}: {:?}",
                post_id, e
            );
            let status = match e {
                AnalyticsError::InvalidParameter(_) => StatusCode::BAD_REQUEST,
                AnalyticsError::NotFound => StatusCode::NOT_FOUND,
                AnalyticsError::Unauthorized => StatusCode::UNAUTHORIZED,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (
                status,
                Json(json!({
                    "error": format!("Failed to get time-based post statistics: {}", e)
                })),
            )
        }
    }
}

/// Refresh the analytics materialized views (admin only)
#[utoipa::path(
    post,
    path = "/api/analytics/refresh",
    tag = "analytics",
    responses(
        (status = 200, description = "Analytics views refreshed successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - admin access required"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn refresh_analytics_views(
    Extension(user): Extension<AuthUser>,
    State(service): State<Arc<AnalyticsService>>,
) -> impl IntoResponse {
    if user.role != Role::Admin {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "Only admins can refresh analytics views"
            })),
        );
    }

    match service.refresh_materialized_views().await {
        Ok(_) => {
            info!("Analytics materialized views refreshed successfully");
            (
                StatusCode::OK,
                Json(json!({
                    "message": "Analytics materialized views refreshed successfully"
                })),
            )
        }
        Err(e) => {
            error!("Failed to refresh analytics views: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": format!("Failed to refresh analytics views: {}", e)
                })),
            )
        }
    }
}
