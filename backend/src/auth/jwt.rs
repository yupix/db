use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
}

impl FromRequestParts<Arc<AppState>> for Claims {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or(AppError::Unauthorized)?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .or_else(|| auth_header.strip_prefix("bearer "))
            .ok_or(AppError::Unauthorized)?;

        verify(token, &state.config.jwt_secret)
    }
}

pub fn generate(user_id: &str, secret: &str) -> Result<String, AppError> {
    let now = Utc::now();
    let expires = now + Duration::hours(24);

    let claims = Claims {
        sub: user_id.to_string(),
        exp: expires.timestamp() as usize,
        iat: now.timestamp() as usize,
    };

    jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("JWT encode error: {}", e)))
}

pub fn verify(token: &str, secret: &str) -> Result<Claims, AppError> {
    let token_data = jsonwebtoken::decode::<Claims>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(secret.as_bytes()),
        &jsonwebtoken::Validation::default(),
    )
    .map_err(|_| AppError::Unauthorized)?;

    Ok(token_data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_generate_and_verify() {
        let secret = "test-secret-key";
        let user_id = "test-user-123";

        let token = generate(user_id, secret).unwrap();
        let claims = verify(&token, secret).unwrap();

        assert_eq!(claims.sub, user_id);
    }

    #[test]
    fn test_jwt_invalid_token() {
        let result = verify("invalid-token", "secret");
        assert!(result.is_err());
    }

    #[test]
    fn test_jwt_wrong_secret() {
        let token = generate("user1", "secret1").unwrap();
        let result = verify(&token, "secret2");
        assert!(result.is_err());
    }
}
