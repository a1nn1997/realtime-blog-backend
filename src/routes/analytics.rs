use crate::analytics::{controller, service::AnalyticsService};
use crate::auth::middleware::auth_middleware;
use crate::cache::redis::RedisCache;
use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use sqlx::PgPool;
use std::sync::Arc;

/// Set up analytics routes
pub fn routes(pool: PgPool, redis_cache: Option<RedisCache>) -> Router {
    let analytics_service = Arc::new(AnalyticsService::new(pool.clone(), redis_cache));

    Router::new()
        .route(
            "/api/analytics/engagement",
            get(controller::get_user_engagement).route_layer(middleware::from_fn(auth_middleware)),
        )
        .route(
            "/api/analytics/engagement/user/:target_user_id",
            get(controller::get_user_engagement_by_id)
                .route_layer(middleware::from_fn(auth_middleware)),
        )
        .route("/api/analytics/posts", get(controller::get_post_stats))
        .route(
            "/api/analytics/posts/:post_id",
            get(controller::get_post_stats_by_id),
        )
        .route(
            "/api/analytics/posts/:post_id/time/:time_range",
            get(controller::get_post_stats_by_time),
        )
        .route(
            "/api/analytics/refresh",
            post(controller::refresh_analytics_views)
                .route_layer(middleware::from_fn(auth_middleware)),
        )
        .with_state(analytics_service)
}
