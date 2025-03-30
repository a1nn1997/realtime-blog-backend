use axum::{
    extract::State, http::StatusCode, middleware::from_fn, response::IntoResponse, routing::get,
    Json, Router,
};
use serde::Serialize;
use sqlx::PgPool;
#[allow(unused_imports)]
use utoipa::{OpenApi, ToSchema};

use crate::auth::middleware::{auth_middleware, AuthUser};

#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    status: String,
    message: String,
}

/// Public health check endpoint
///
/// Returns status "ok" if the service is running
#[utoipa::path(
    get,
    path = "/api/health",
    responses(
        (status = 200, description = "Server is healthy"),
    ),
    tag = "health"
)]
pub async fn health_check() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(HealthResponse {
            status: "ok".to_string(),
            message: "Server is running".to_string(),
        }),
    )
}

/// Protected health check endpoint
///
/// Returns status "ok" along with user information if authenticated
#[utoipa::path(
    get,
    path = "/api/health/protected",
    responses(
        (status = 200, description = "Server is healthy and user is authenticated"),
        (status = 401, description = "Unauthorized - Invalid or missing token")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "health"
)]
pub async fn protected_health_check(
    user: AuthUser,
    State(pool): State<PgPool>,
) -> impl IntoResponse {
    // Attempt a simple database query to check DB health
    let db_status = match sqlx::query("SELECT 1").fetch_one(&pool).await {
        Ok(_) => "ok",
        Err(_) => "error",
    };

    (
        StatusCode::OK,
        Json(HealthResponse {
            status: "ok".to_string(),
            message: format!(
                "Server is running. Authenticated as user: {} with role: {:?}. Database status: {}",
                user.user_id, user.role, db_status
            ),
        }),
    )
}

pub fn routes(pool: PgPool) -> Router {
    Router::new().route("/api/health", get(health_check)).route(
        "/api/health/protected",
        get(protected_health_check)
            .route_layer(from_fn(auth_middleware))
            .with_state(pool),
    )
}
