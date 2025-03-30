use axum::{middleware::from_fn, routing::post, Router};
use sqlx::PgPool;

// Import the controller functions directly
// These functions have #[utoipa::path] attributes and will appear in Swagger
use crate::auth::controller::{login, register};
use crate::auth::middleware::auth_middleware;

// Fix route definitions
pub fn routes(pool: PgPool) -> Router {
    Router::new()
        // This route will appear in Swagger because register() has #[utoipa::path] attribute
        .route("/api/auth/register", post(register))
        // This route will appear in Swagger because login() has #[utoipa::path] attribute
        .route("/api/auth/login", post(login))
        .with_state(pool)
}

// Simplify protected routes to avoid middleware nesting issues
pub fn protected_routes(pool: PgPool) -> Router {
    Router::new()
        // Admin routes
        .route("/api/admin/example", post(|| async { "Admin only" }))
        // Author routes
        .route("/api/author/example", post(|| async { "Author only" }))
        // Add basic auth middleware
        .layer(from_fn(auth_middleware))
        .with_state(pool)
}
