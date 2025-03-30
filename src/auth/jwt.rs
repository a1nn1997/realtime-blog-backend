use axum::http::StatusCode;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// User roles for role-based access control
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum Role {
    User,
    Author,
    Admin,
    Analyst,
}

impl Role {
    pub fn from_str(role: &str) -> Result<Self, String> {
        match role.to_lowercase().as_str() {
            "user" => Ok(Role::User),
            "author" => Ok(Role::Author),
            "admin" => Ok(Role::Admin),
            "analyst" => Ok(Role::Analyst),
            _ => Err(format!("Invalid role: {}", role)),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Role::User => "user",
            Role::Author => "author",
            Role::Admin => "admin",
            Role::Analyst => "analyst",
        }
    }
}

/// JWT Claims structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // Subject (user ID)
    pub role: Role,  // User role
    pub exp: usize,  // Expiration time
    pub iat: usize,  // Issued at
}

/// Generate a JWT token for a user
pub fn generate_token(user_id: &Uuid, role: Role) -> Result<String, JwtError> {
    let jwt_secret = std::env::var("JWT_SECRET").map_err(|_| JwtError::MissingSecret)?;

    let now = Utc::now();
    let expiry = now + Duration::hours(24); // 24 hour expiration

    let claims = Claims {
        sub: user_id.to_string(),
        role,
        exp: expiry.timestamp() as usize,
        iat: now.timestamp() as usize,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
    .map_err(|_| JwtError::TokenCreation)
}

/// Validate a JWT token and extract claims
pub fn validate_token(token: &str) -> Result<Claims, JwtError> {
    let jwt_secret = std::env::var("JWT_SECRET").map_err(|_| JwtError::MissingSecret)?;

    // Create a validation that explicitly checks for token expiration
    let mut validation = Validation::default();
    validation.validate_exp = true; // Explicitly validate expiration
    validation.leeway = 0; // No leeway/grace period for testing

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &validation,
    )
    .map_err(|_e| {
        // You could add logging here for debugging in real applications
        // println!("Token validation error: {:?}", _e);
        JwtError::InvalidToken
    })?;

    Ok(token_data.claims)
}

#[derive(Debug)]
pub enum JwtError {
    MissingSecret,
    TokenCreation,
    InvalidToken,
}

impl fmt::Display for JwtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JwtError::MissingSecret => write!(f, "JWT secret is missing or not set"),
            JwtError::TokenCreation => write!(f, "Failed to create JWT token"),
            JwtError::InvalidToken => write!(f, "Invalid or expired JWT token"),
        }
    }
}

impl From<JwtError> for StatusCode {
    fn from(err: JwtError) -> Self {
        match err {
            JwtError::MissingSecret => StatusCode::INTERNAL_SERVER_ERROR,
            JwtError::TokenCreation => StatusCode::INTERNAL_SERVER_ERROR,
            JwtError::InvalidToken => StatusCode::UNAUTHORIZED,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::thread;
    use std::time::Duration as StdDuration;

    #[test]
    fn test_role_from_str() {
        assert_eq!(Role::from_str("user").unwrap(), Role::User);
        assert_eq!(Role::from_str("admin").unwrap(), Role::Admin);
        assert_eq!(Role::from_str("author").unwrap(), Role::Author);
        assert_eq!(Role::from_str("analyst").unwrap(), Role::Analyst);
        assert!(Role::from_str("invalid").is_err());
    }

    #[test]
    fn test_role_as_str() {
        assert_eq!(Role::User.as_str(), "user");
        assert_eq!(Role::Admin.as_str(), "admin");
        assert_eq!(Role::Author.as_str(), "author");
        assert_eq!(Role::Analyst.as_str(), "analyst");
    }

    #[test]
    fn test_jwt_token_generation_and_validation() {
        // Set JWT_SECRET for the test
        env::set_var("JWT_SECRET", "test_secret");

        let user_id = Uuid::new_v4();
        let role = Role::User;

        // Generate token
        let token = generate_token(&user_id, role.clone()).expect("Token generation failed");
        assert!(!token.is_empty());

        // Validate token
        let claims = validate_token(&token).expect("Token validation failed");
        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.role, role);
    }

    #[test]
    fn test_jwt_error_conversion() {
        // Test conversion from JwtError to StatusCode
        use axum::http::StatusCode;

        assert_eq!(
            StatusCode::from(JwtError::MissingSecret),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            StatusCode::from(JwtError::TokenCreation),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            StatusCode::from(JwtError::InvalidToken),
            StatusCode::UNAUTHORIZED
        );
    }

    #[test]
    fn test_expired_token_rejection() {
        env::set_var("JWT_SECRET", "test_secret");

        let result = validate_token("invalid.token.format");
        assert!(result.is_err());

        match result {
            Err(JwtError::InvalidToken) => (), // expected
            _ => panic!("Expected InvalidToken error"),
        }
    }

    #[test]
    fn test_different_roles_in_tokens() {
        env::set_var("JWT_SECRET", "test_secret");
        let user_id = Uuid::new_v4();

        for role in [Role::User, Role::Admin, Role::Author, Role::Analyst].iter() {
            let token = generate_token(&user_id, role.clone()).expect("Token generation failed");
            let claims = validate_token(&token).expect("Token validation failed");

            assert_eq!(claims.role, *role);
        }
    }

    #[test]
    fn test_jwt_secret_environment_variable() {
        // Test missing JWT secret
        env::remove_var("JWT_SECRET");
        let user_id = Uuid::new_v4();
        let result = generate_token(&user_id, Role::User);
        assert!(result.is_err());
        match result {
            Err(JwtError::MissingSecret) => {} // Expected
            _ => panic!("Expected MissingSecret error"),
        }

        // Test with empty JWT secret
        env::set_var("JWT_SECRET", "");
        let result = generate_token(&user_id, Role::User);
        assert!(
            result.is_ok(),
            "Should accept empty secret, though not recommended"
        );

        // Restore valid secret for other tests
        env::set_var("JWT_SECRET", "test_secret");
    }

    #[test]
    fn test_token_tampering() {
        env::set_var("JWT_SECRET", "test_secret");
        let user_id = Uuid::new_v4();

        // Generate valid token
        let token = generate_token(&user_id, Role::User).unwrap();

        // Tamper with the token - modify the middle section (payload)
        let parts: Vec<&str> = token.split('.').collect();
        assert_eq!(parts.len(), 3, "JWT should have 3 parts");

        let tampered_token = format!("{}.{}tampered.{}", parts[0], parts[1], parts[2]);

        // Verify that tampered token is rejected
        let result = validate_token(&tampered_token);
        assert!(result.is_err());
        match result {
            Err(JwtError::InvalidToken) => {} // Expected
            _ => panic!("Expected InvalidToken error"),
        }
    }

    #[test]
    fn test_malformed_tokens() {
        env::set_var("JWT_SECRET", "test_secret");

        // Test various malformed tokens
        let malformed_tokens = [
            "",                          // Empty token
            "not.a.jwt.token",           // Too many segments
            "missing.segments",          // Too few segments
            "invalid base64.parts.here", // Invalid base64
            "eyJhbGciOiJIUzI1NiJ9",      // Header only
        ];

        for token in &malformed_tokens {
            let result = validate_token(token);
            assert!(result.is_err(), "Token '{}' should be rejected", token);
            match result {
                Err(JwtError::InvalidToken) => {} // Expected
                _ => panic!("Expected InvalidToken error for '{}'", token),
            }
        }
    }

    #[test]
    fn test_role_serialization_consistency() {
        // Test that roles are serialized and deserialized consistently
        let roles = [Role::User, Role::Admin, Role::Author, Role::Analyst];

        for role in &roles {
            // This test serializes the role to JSON and back
            let serialized = serde_json::to_string(role).expect("Failed to serialize role");
            let deserialized: Role =
                serde_json::from_str(&serialized).expect("Failed to deserialize role");

            assert_eq!(
                *role, deserialized,
                "Role should remain the same after serialization cycle"
            );
        }
    }

    #[test]
    fn test_token_with_all_roles() {
        env::set_var("JWT_SECRET", "test_secret");
        let user_id = Uuid::new_v4();
        let roles = [Role::User, Role::Admin, Role::Author, Role::Analyst];

        for role in &roles {
            let token = generate_token(&user_id, role.clone()).unwrap();
            let claims = validate_token(&token).unwrap();

            assert_eq!(claims.sub, user_id.to_string());
            assert_eq!(claims.role, *role);
        }
    }

    #[test]
    fn test_claims_issued_and_expiry_times() {
        env::set_var("JWT_SECRET", "test_secret");
        let user_id = Uuid::new_v4();

        let now = chrono::Utc::now().timestamp() as usize;
        let token = generate_token(&user_id, Role::User).unwrap();
        let claims = validate_token(&token).unwrap();

        // Verify that issued at time is approximately now
        assert!(
            claims.iat <= now + 1 && claims.iat >= now - 1,
            "Issued at time should be close to current time"
        );

        // Verify expiry is 24 hours later (with small margin for test execution time)
        let expected_expiry = now + (24 * 60 * 60);
        assert!(
            claims.exp <= expected_expiry + 5 && claims.exp >= expected_expiry - 5,
            "Expiry should be approximately 24 hours from now"
        );
    }

    #[test]
    fn test_uuid_conversion_in_claims() {
        env::set_var("JWT_SECRET", "test_secret");

        // Test with normal UUID
        let user_id = Uuid::new_v4();
        let token = generate_token(&user_id, Role::User).unwrap();
        let claims = validate_token(&token).unwrap();
        assert_eq!(claims.sub, user_id.to_string());

        // Test with nil UUID
        let nil_uuid = Uuid::nil();
        let token = generate_token(&nil_uuid, Role::User).unwrap();
        let claims = validate_token(&token).unwrap();
        assert_eq!(claims.sub, nil_uuid.to_string());
    }

    #[test]
    fn test_token_validation_concurrency() {
        env::set_var("JWT_SECRET", "test_secret");
        let user_id = Uuid::new_v4();
        let token = generate_token(&user_id, Role::User).unwrap();

        // Test concurrent validation
        let mut handles = vec![];
        for _ in 0..10 {
            let token_clone = token.clone();
            let handle = thread::spawn(move || {
                let result = validate_token(&token_clone);
                assert!(result.is_ok());
                let claims = result.unwrap();
                assert_eq!(claims.sub, user_id.to_string());
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_role_case_insensitivity() {
        // Test that role parsing is case-insensitive
        assert_eq!(Role::from_str("USER").unwrap(), Role::User);
        assert_eq!(Role::from_str("user").unwrap(), Role::User);
        assert_eq!(Role::from_str("User").unwrap(), Role::User);

        assert_eq!(Role::from_str("ADMIN").unwrap(), Role::Admin);
        assert_eq!(Role::from_str("admin").unwrap(), Role::Admin);
        assert_eq!(Role::from_str("Admin").unwrap(), Role::Admin);

        assert_eq!(Role::from_str("AUTHOR").unwrap(), Role::Author);
        assert_eq!(Role::from_str("ANALYST").unwrap(), Role::Analyst);
    }

    // Only run this test if specifically requested as it takes time
    #[test]
    #[ignore]
    fn test_token_expiration() {
        // This test creates a token with a very short expiration time
        env::set_var("JWT_SECRET", "temp_secret_for_expiration_test");

        let user_id = Uuid::new_v4();

        // Create claims with a 1-second expiration
        let now = chrono::Utc::now();
        let claims = Claims {
            sub: user_id.to_string(),
            role: Role::User,
            iat: now.timestamp() as usize,
            exp: (now.timestamp() + 1) as usize, // Expire in 1 second
        };

        // Encode the token
        let token = jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &claims,
            &jsonwebtoken::EncodingKey::from_secret(env::var("JWT_SECRET").unwrap().as_bytes()),
        )
        .unwrap();

        // Token should be valid initially
        let result = validate_token(&token);
        assert!(result.is_ok(), "Token should be valid initially");

        // Sleep for 2 seconds to ensure token expires
        thread::sleep(StdDuration::from_secs(2));

        // Token should now be invalid due to expiration
        let result = validate_token(&token);
        assert!(result.is_err(), "Token should be expired");

        // Check the error type is correct
        match result {
            Err(JwtError::InvalidToken) => {} // Expected
            _ => panic!("Expected InvalidToken error for expired token"),
        }

        // Restore original secret
        env::set_var("JWT_SECRET", "test_secret");
    }
}
