use axum::{
    async_trait,
    extract::FromRequestParts,
    headers::{authorization::Bearer, Authorization},
    http::{request::Parts, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Json, Response},
    RequestPartsExt, TypedHeader,
};
use serde::Serialize;
use tracing::{error, info};
use uuid::Uuid;

use super::jwt::{validate_token, Role};

/// Authenticated user information
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub role: Role,
}

#[derive(Debug, Serialize)]
struct AuthErrorResponse {
    error: String,
}

/// Authentication middleware to protect routes
pub async fn auth_middleware<B>(req: Request<B>, next: Next<B>) -> Result<Response, Response> {
    let (mut parts, body) = req.into_parts();

    // Extract the token from the Authorization header
    let bearer_result = parts.extract::<TypedHeader<Authorization<Bearer>>>().await;

    if let Err(e) = bearer_result {
        error!("Authorization header extraction failed: {:?}", e);
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(AuthErrorResponse {
                error: "Missing or invalid Authorization header. Please provide a Bearer token"
                    .to_string(),
            }),
        )
            .into_response());
    }

    let TypedHeader(Authorization(bearer)) = bearer_result.unwrap();

    // Validate the token
    let claims_result = validate_token(bearer.token());
    if let Err(e) = claims_result {
        error!("Token validation failed: {:?}", e);
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(AuthErrorResponse {
                error: "Invalid token. Please login again".to_string(),
            }),
        )
            .into_response());
    }

    let claims = claims_result.unwrap();

    // Parse the user ID
    let user_id_result = Uuid::parse_str(&claims.sub);
    if let Err(e) = user_id_result {
        error!("User ID parsing failed: {:?}", e);
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(AuthErrorResponse {
                error: "Invalid user identifier in token".to_string(),
            }),
        )
            .into_response());
    }

    let user_id = user_id_result.unwrap();
    info!(
        "User authenticated: {} with role {:?}",
        user_id, claims.role
    );

    // Create AuthUser and insert into request extensions
    let auth_user = AuthUser {
        user_id,
        role: claims.role,
    };

    parts.extensions.insert(auth_user);

    // Continue with the request
    let req = Request::from_parts(parts, body);
    Ok(next.run(req).await)
}

/// Role-based authorization middleware
pub async fn require_role<B>(
    role: Role,
    req: Request<B>,
    next: Next<B>,
) -> Result<Response, Response> {
    let auth_user = match req.extensions().get::<AuthUser>() {
        Some(user) => user.clone(),
        None => {
            error!("AuthUser not found in request extensions");
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(AuthErrorResponse {
                    error: "Authentication required".to_string(),
                }),
            )
                .into_response());
        }
    };

    // Check if user has required role
    match auth_user.role {
        Role::Admin => {
            info!("Admin access granted to user: {}", auth_user.user_id);
        } // Admin has access to everything
        r if r == role => {
            info!(
                "Role-based access granted to user: {} with role {:?}",
                auth_user.user_id, r
            );
        } // User has the specific required role
        _ => {
            error!(
                "Insufficient permissions for user: {} with role {:?}, required role: {:?}",
                auth_user.user_id, auth_user.role, role
            );
            return Err((
                StatusCode::FORBIDDEN,
                Json(AuthErrorResponse {
                    error: format!("Insufficient permissions. Required role: {:?}", role),
                }),
            )
                .into_response());
        }
    }

    Ok(next.run(req).await)
}

/// Extractor for authenticated user
#[async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts.extensions.get::<AuthUser>().cloned().ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(AuthErrorResponse {
                    error: "Authentication required".to_string(),
                }),
            )
                .into_response()
        })
    }
}

/// Optional authentication middleware for public routes that need auth info
pub async fn optional_auth_middleware<B>(req: Request<B>, next: Next<B>) -> Response {
    let (mut parts, body) = req.into_parts();

    // Extract the token from the Authorization header if present
    let bearer_result = parts.extract::<TypedHeader<Authorization<Bearer>>>().await;

    if let Ok(TypedHeader(Authorization(bearer))) = bearer_result {
        // If token is present, try to validate it
        if let Ok(claims) = validate_token(bearer.token()) {
            // Parse the user ID
            if let Ok(user_id) = Uuid::parse_str(&claims.sub) {
                // Create AuthUser and insert into request extensions
                let auth_user = AuthUser {
                    user_id,
                    role: claims.role,
                };

                // Insert as Option<AuthUser>
                parts.extensions.insert(Some(auth_user));
            }
        }
    } else {
        // No valid token, insert None as Option<AuthUser>
        parts.extensions.insert(Option::<AuthUser>::None);
    }

    // Continue with the request
    let req = Request::from_parts(parts, body);
    next.run(req).await
}
