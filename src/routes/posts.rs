use crate::auth::middleware::{auth_middleware, optional_auth_middleware};
use crate::cache::redis::RedisCache;
use crate::post::controller;
use axum::{
    middleware,
    routing::{delete, get, post, put},
    Router,
};
use sqlx::PgPool;

pub fn routes(pool: PgPool, redis_cache: Option<RedisCache>) -> Router {
    // Create routers with their state once
    let app_state = (pool, redis_cache);

    let public_routes = Router::new()
        // Order matters here - more specific routes first
        .route("/api/posts/popular", get(controller::get_popular_posts))
        .route("/api/posts/view/:id_or_slug", get(controller::get_post))
        .route_layer(middleware::from_fn(optional_auth_middleware))
        .with_state(app_state.clone());

    let private_routes = Router::new()
        .route("/api/posts", post(controller::create_post))
        .route("/api/posts/edit/:id", put(controller::update_post))
        .route("/api/posts/delete/:id", delete(controller::delete_post))
        .route_layer(middleware::from_fn(auth_middleware))
        .with_state(app_state);

    public_routes.merge(private_routes)
}
