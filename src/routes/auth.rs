use crate::auth::controller;
use axum::{routing::post, Router};
use sqlx::PgPool;

/// Authentication routes for login and registration
pub fn routes(pool: PgPool) -> Router {
    Router::new()
        .route("/api/auth/login", post(controller::login))
        .route("/api/auth/register", post(controller::register))
        .with_state(pool)
}
