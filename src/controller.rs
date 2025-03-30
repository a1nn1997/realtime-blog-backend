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
}
