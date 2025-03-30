use argon2::{
    password_hash::PasswordVerifier,
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use axum::http::StatusCode;
use sqlx::PgPool;
use tracing::{error, info};
use uuid::Uuid;

use super::jwt::{generate_token, Role};

// Input data structures
pub struct RegisterData {
    pub username: String,
    pub email: String,
    pub password: String,
    pub role: Option<String>,
}

pub struct LoginData {
    pub email: String,
    pub password: String,
}

// Result data structure
pub struct AuthResult {
    pub user_id: Uuid,
    pub username: String,
    pub email: String,
    pub role: String,
    pub token: String,
}

// Service errors
pub enum AuthError {
    InvalidInput(String),
    AlreadyExists(String),
    InvalidCredentials,
    DatabaseError(String),
    TokenError,
    InternalError(String),
}

impl AuthError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::InvalidInput(_) => StatusCode::BAD_REQUEST,
            Self::AlreadyExists(_) => StatusCode::CONFLICT,
            Self::InvalidCredentials => StatusCode::UNAUTHORIZED,
            Self::DatabaseError(_) | Self::TokenError | Self::InternalError(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::InvalidInput(msg) => msg.clone(),
            Self::AlreadyExists(msg) => msg.clone(),
            Self::InvalidCredentials => "Invalid email or password".to_string(),
            Self::DatabaseError(msg) => format!("Database error: {}", msg),
            Self::TokenError => "Failed to generate auth token".to_string(),
            Self::InternalError(msg) => msg.clone(),
        }
    }
}

// User registration service
pub async fn register(pool: &PgPool, data: RegisterData) -> Result<AuthResult, AuthError> {
    // Validate input
    if data.username.is_empty() || data.email.is_empty() || data.password.is_empty() {
        return Err(AuthError::InvalidInput(
            "Username, email, and password are required".to_string(),
        ));
    }

    info!("Checking if user with email {} already exists", data.email);

    // Check if user with email already exists
    let existing_user =
        sqlx::query_as::<_, (uuid::Uuid,)>("SELECT id FROM global.users WHERE email = $1")
            .bind(&data.email)
            .fetch_optional(pool)
            .await
            .map_err(|e| {
                error!("Database error while checking existing user: {}", e);
                AuthError::DatabaseError(e.to_string())
            })?;

    if existing_user.is_some() {
        info!("User with email {} already exists", data.email);
        return Err(AuthError::AlreadyExists("Email already in use".to_string()));
    }

    info!("Creating new user with email {}", data.email);

    // Hash password
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(data.password.as_bytes(), &salt)
        .map_err(|e| {
            error!("Password hashing failed: {}", e);
            AuthError::InternalError(format!("Password hashing failed: {}", e))
        })?
        .to_string();

    // Determine role
    let role_str = data.role.unwrap_or_else(|| "user".to_string());
    let role = Role::from_str(&role_str).map_err(|e| AuthError::InvalidInput(e))?;

    // Create new user
    let user_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO global.users (id, username, email, password_hash, role) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind(&data.username)
    .bind(&data.email)
    .bind(&password_hash)
    .bind(&role_str)
    .execute(pool)
    .await
    .map_err(|e| {
        error!("Failed to insert new user: {}", e);
        AuthError::DatabaseError(e.to_string())
    })?;

    info!("User created successfully with ID: {}", user_id);

    // Generate token
    let token = generate_token(&user_id, role).map_err(|e| {
        error!("Token generation failed: {:?}", e);
        AuthError::TokenError
    })?;

    // Return result
    Ok(AuthResult {
        user_id,
        username: data.username,
        email: data.email,
        role: role_str,
        token,
    })
}

// User login service
pub async fn login(pool: &PgPool, data: LoginData) -> Result<AuthResult, AuthError> {
    info!("Attempting login for user with email: {}", data.email);

    // Find user by email (without role column)
    let user = sqlx::query_as::<_, (Uuid, String, String, String)>(
        "SELECT id, username, email, password_hash FROM global.users WHERE email = $1",
    )
    .bind(&data.email)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        error!("Database error while fetching user: {}", e);
        AuthError::DatabaseError(e.to_string())
    })?;

    let user = match user {
        Some(user) => user,
        None => {
            info!("No user found with email: {}", data.email);
            return Err(AuthError::InvalidCredentials);
        }
    };

    info!("User found, verifying password");

    // Verify password
    let parsed_hash = argon2::password_hash::PasswordHash::new(&user.3).map_err(|e| {
        error!("Failed to parse password hash: {}", e);
        AuthError::InvalidCredentials
    })?;

    let argon2 = Argon2::default();
    argon2
        .verify_password(data.password.as_bytes(), &parsed_hash)
        .map_err(|e| {
            info!("Password verification failed: {}", e);
            AuthError::InvalidCredentials
        })?;

    info!("Password verified successfully");

    // Use default role (User) since the role column doesn't exist in the database
    let role = Role::User;
    let role_str = "user".to_string();

    // Generate token
    let token = generate_token(&user.0, role).map_err(|e| {
        error!("Token generation failed: {:?}", e);
        AuthError::TokenError
    })?;

    info!("Login successful for user ID: {}", user.0);

    // Return result
    Ok(AuthResult {
        user_id: user.0,
        username: user.1,
        email: user.2,
        role: role_str,
        token,
    })
}
