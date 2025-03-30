use crate::auth::middleware::auth_middleware;
use crate::cache::redis::RedisCache;
use crate::recommendations::controller;
use crate::recommendations::service::RecommendationService;
use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use sqlx::PgPool;
use std::sync::Arc;

/// Set up recommendations routes
pub fn routes(pool: PgPool, redis_cache: Option<RedisCache>) -> Router {
    let recommendation_service = Arc::new(RecommendationService::new(pool.clone(), redis_cache));

    Router::new()
        .route(
            "/recommendations",
            get(controller::get_recommended_posts)
                .route_layer(middleware::from_fn(auth_middleware)),
        )
        .route(
            "/similar/:post_id",
            get(controller::get_similar_posts).route_layer(middleware::from_fn(auth_middleware)),
        )
        .route(
            "/model/refresh",
            post(controller::refresh_recommendation_model)
                .route_layer(middleware::from_fn(auth_middleware)),
        )
        .with_state(recommendation_service)
}
