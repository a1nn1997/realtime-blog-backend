use crate::auth::jwt::Role;
use crate::auth::middleware::AuthUser;
use crate::recommendations::model::{
    PostRecommendation, RecommendationError, RecommendationParams,
};
use crate::recommendations::service::RecommendationService;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    Extension,
};
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, error};
use utoipa::{IntoParams, ToSchema};

/// Get personalized post recommendations for the current user
#[utoipa::path(
    get,
    path = "/api/recommendations",
    tag = "recommendations",
    params(
        ("limit" = Option<i64>, Query, description = "Maximum number of recommendations", example = "10"),
        ("offset" = Option<i64>, Query, description = "Offset for pagination", example = "0"),
        ("algorithm" = Option<String>, Query, description = "Algorithm to use: collaborative, content_based, hybrid, popular", example = "hybrid"),
        ("include_tags" = Option<Vec<String>>, Query, description = "Tags to include in recommendations (comma-separated)", example = "rust,programming,webdev"),
        ("exclude_tags" = Option<Vec<String>>, Query, description = "Tags to exclude from recommendations (comma-separated)", example = "deprecated,outdated"),
        ("min_score" = Option<f64>, Query, description = "Minimum score threshold", example = "0.5")
    ),
    responses(
        (status = 200, description = "Recommendations retrieved successfully", body = Vec<PostRecommendation>),
        (status = 400, description = "Invalid parameters"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_recommended_posts(
    Extension(user): Extension<AuthUser>,
    State(service): State<Arc<RecommendationService>>,
    Query(params): Query<RecommendationParams>,
) -> impl IntoResponse {
    match service
        .get_recommendations_for_user(user.user_id, &params)
        .await
    {
        Ok(recommendations) => {
            let recommendations_count = recommendations.len();
            debug!(
                "Retrieved {} recommendations for user {}",
                recommendations_count, user.user_id
            );
            (StatusCode::OK, Json(json!(recommendations)))
        }
        Err(err) => {
            let status = match err {
                RecommendationError::InvalidParameter(_) => StatusCode::BAD_REQUEST,
                RecommendationError::NotFound => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            error!("Failed to get recommendations: {}", err);
            (
                status,
                Json(json!({
                    "error": format!("Failed to get recommendations: {}", err),
                })),
            )
        }
    }
}

/// Get similar posts to a specific post
#[utoipa::path(
    get,
    path = "/api/recommendations/similar/{post_id}",
    tag = "recommendations",
    params(
        ("post_id" = i64, Path, description = "Post ID to find similar posts for"),
        ("limit" = Option<i64>, Query, description = "Maximum number of recommendations", example = "10"),
        ("offset" = Option<i64>, Query, description = "Offset for pagination", example = "0"),
        ("algorithm" = Option<String>, Query, description = "Algorithm to use: collaborative, content_based, hybrid, popular", example = "hybrid"),
        ("include_tags" = Option<Vec<String>>, Query, description = "Tags to include in recommendations (comma-separated)", example = "rust,programming,webdev"),
        ("exclude_tags" = Option<Vec<String>>, Query, description = "Tags to exclude from recommendations (comma-separated)", example = "deprecated,outdated"),
        ("min_score" = Option<f64>, Query, description = "Minimum score threshold", example = "0.5")
    ),
    responses(
        (status = 200, description = "Similar posts retrieved successfully", body = Vec<PostRecommendation>),
        (status = 400, description = "Invalid parameters"),
        (status = 404, description = "Post not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_similar_posts(
    Path(post_id): Path<i64>,
    State(service): State<Arc<RecommendationService>>,
    Query(params): Query<RecommendationParams>,
) -> impl IntoResponse {
    let user_id = None; // Optional user ID, not required for similar posts

    match service.get_similar_posts(post_id, user_id, &params).await {
        Ok(similar_posts) => {
            debug!(
                "Retrieved {} similar posts for post {}",
                similar_posts.len(),
                post_id
            );
            (StatusCode::OK, Json(json!(similar_posts)))
        }
        Err(err) => {
            let status = match err {
                RecommendationError::InvalidParameter(_) => StatusCode::BAD_REQUEST,
                RecommendationError::NotFound => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            error!("Failed to get similar posts: {}", err);
            (
                status,
                Json(json!({
                    "error": format!("Failed to get similar posts: {}", err),
                })),
            )
        }
    }
}

/// Refresh recommendation model (admin only)
#[utoipa::path(
    post,
    path = "/api/recommendations/refresh",
    tag = "recommendations",
    responses(
        (status = 200, description = "Recommendation model refreshed successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - admin access required"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn refresh_recommendation_model(
    Extension(user): Extension<AuthUser>,
    State(service): State<Arc<RecommendationService>>,
) -> impl IntoResponse {
    if user.role != Role::Admin {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "Admin access required",
            })),
        );
    }

    match service.refresh_recommendation_model().await {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({
                "message": "Recommendation model refreshed successfully",
            })),
        ),
        Err(err) => {
            error!("Failed to refresh recommendation model: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": format!("Failed to refresh recommendation model: {}", err),
                })),
            )
        }
    }
}

/// Get personalized post recommendations for the current user - boxed version
pub async fn get_recommended_posts_boxed(
    Extension(user): Extension<AuthUser>,
    State(service): State<Arc<RecommendationService>>,
    Query(params): Query<RecommendationParams>,
) -> Box<dyn IntoResponse> {
    match service
        .get_recommendations_for_user(user.user_id, &params)
        .await
    {
        Ok(recommendations) => {
            debug!(
                "Retrieved {} recommendations for user {}",
                recommendations.len(),
                user.user_id
            );
            Box::new((StatusCode::OK, Json(json!(recommendations))))
        }
        Err(err) => {
            let status = match err {
                RecommendationError::InvalidParameter(_) => StatusCode::BAD_REQUEST,
                RecommendationError::NotFound => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            error!("Failed to get recommendations: {}", err);
            Box::new((
                status,
                Json(json!({
                    "error": format!("Failed to get recommendations: {}", err),
                })),
            ))
        }
    }
}

/// Get similar posts to a specific post - boxed version
pub async fn get_similar_posts_boxed(
    Path(post_id): Path<i64>,
    State(service): State<Arc<RecommendationService>>,
    Query(params): Query<RecommendationParams>,
) -> Box<dyn IntoResponse> {
    // Pass None for user_id as it should be optional for similar posts
    let user_id = None;

    match service.get_similar_posts(post_id, user_id, &params).await {
        Ok(similar_posts) => {
            debug!(
                "Retrieved {} similar posts for post {}",
                similar_posts.len(),
                post_id
            );
            Box::new((StatusCode::OK, Json(json!(similar_posts))))
        }
        Err(err) => {
            let status = match err {
                RecommendationError::InvalidParameter(_) => StatusCode::BAD_REQUEST,
                RecommendationError::NotFound => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            error!("Failed to get similar posts: {}", err);
            Box::new((
                status,
                Json(json!({
                    "error": format!("Failed to get similar posts: {}", err),
                })),
            ))
        }
    }
}

/// Refresh recommendation model (admin only) - boxed version
pub async fn refresh_recommendation_model_boxed(
    Extension(user): Extension<AuthUser>,
    State(service): State<Arc<RecommendationService>>,
) -> Box<dyn IntoResponse> {
    if user.role != Role::Admin {
        return Box::new((
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "Admin access required",
            })),
        ));
    }

    match service.refresh_recommendation_model().await {
        Ok(_) => Box::new((
            StatusCode::OK,
            Json(json!({
                "message": "Recommendation model refreshed successfully",
            })),
        )),
        Err(err) => {
            error!("Failed to refresh recommendation model: {}", err);
            Box::new((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": format!("Failed to refresh recommendation model: {}", err),
                })),
            ))
        }
    }
}
