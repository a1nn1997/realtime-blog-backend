use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{error, info};
use utoipa::ToSchema;

use super::service::{self, AuthError, AuthResult, LoginData, RegisterData};

// Request DTOs
#[derive(Debug, Deserialize, ToSchema)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    pub role: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

// Response DTOs
#[derive(Debug, Serialize, ToSchema)]
pub struct AuthResponse {
    pub user_id: String,
    pub username: String,
    pub email: String,
    pub role: String,
    pub token: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

// Convert AuthResult to AuthResponse
fn to_response(result: AuthResult) -> AuthResponse {
    AuthResponse {
        user_id: result.user_id.to_string(),
        username: result.username,
        email: result.email,
        role: result.role,
        token: result.token,
    }
}

// Convert AuthError to Response
fn handle_error(error: AuthError) -> Response {
    let status = error.status_code();
    let message = error.message();

    // Log the error
    if status == StatusCode::INTERNAL_SERVER_ERROR {
        error!("Internal server error: {}", message);
    } else {
        info!("Auth error: {} ({})", message, status);
    }

    // Create a more detailed error response
    let details = match &error {
        AuthError::DatabaseError(details) => {
            Some(format!("Database operation failed: {}", details))
        }
        _ => None,
    };

    (
        status,
        Json(ErrorResponse {
            error: message,
            details,
        }),
    )
        .into_response()
}

// Controller for user registration
#[utoipa::path(
    post,
    path = "/api/auth/register",
    request_body = RegisterRequest,
    responses(
        (status = 201, description = "User registered successfully", body = AuthResponse),
        (status = 400, description = "Bad request", body = ErrorResponse)
    ),
    tag = "authentication"
)]
pub async fn register(State(pool): State<PgPool>, Json(req): Json<RegisterRequest>) -> Response {
    info!("Registration request received for email: {}", req.email);

    let data = RegisterData {
        username: req.username,
        email: req.email,
        password: req.password,
        role: req.role,
    };

    match service::register(&pool, data).await {
        Ok(result) => {
            let response = to_response(result);
            info!("User registered successfully: {}", response.user_id);
            (StatusCode::CREATED, Json(response)).into_response()
        }
        Err(error) => handle_error(error),
    }
}

// Controller for user login
#[utoipa::path(
    post,
    path = "/api/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = AuthResponse),
        (status = 401, description = "Invalid credentials", body = ErrorResponse)
    ),
    tag = "authentication"
)]
pub async fn login(State(pool): State<PgPool>, Json(req): Json<LoginRequest>) -> Response {
    info!("Login request received for email: {}", req.email);

    let data = LoginData {
        email: req.email,
        password: req.password,
    };

    match service::login(&pool, data).await {
        Ok(result) => {
            let response = to_response(result);
            info!("User login successful: {}", response.user_id);
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(error) => handle_error(error),
    }
}
