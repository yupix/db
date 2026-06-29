use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::error::AppError;
use crate::state::AppState;

const ACCESS_TOKEN_COOKIE: &str = "access_token";

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TokenType {
    Access,
    Refresh,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
    #[serde(default = "default_token_type")]
    pub token_type: TokenType,
}

fn default_token_type() -> TokenType {
    TokenType::Access
}

impl FromRequestParts<Arc<AppState>> for Claims {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let token = extract_token_from_header(parts)
            .or_else(|| extract_token_from_cookie(parts));

        let token = token.ok_or(AppError::Unauthorized)?;
        let claims = verify(&token, &state.config.jwt_secret)?;
        if claims.token_type != TokenType::Access {
            return Err(AppError::Unauthorized);
        }
        Ok(claims)
    }
}

fn extract_token_from_header(parts: &Parts) -> Option<String> {
    parts
        .headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.strip_prefix("Bearer ")
                .or_else(|| v.strip_prefix("bearer "))
                .map(|s| s.to_string())
        })
}

fn extract_token_from_cookie(parts: &Parts) -> Option<String> {
    parts
        .headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|c| {
                let c = c.trim();
                c.strip_prefix(&format!("{}=", ACCESS_TOKEN_COOKIE))
                    .map(|s| s.to_string())
            })
        })
}

pub fn generate_access_token(user_id: &str, secret: &str) -> Result<String, AppError> {
    let now = Utc::now();
    let expires = now + Duration::minutes(15);

    let claims = Claims {
        sub: user_id.to_string(),
        exp: expires.timestamp() as usize,
        iat: now.timestamp() as usize,
        token_type: TokenType::Access,
    };

    jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("JWT encode error: {}", e)))
}

pub fn generate_refresh_token(user_id: &str, secret: &str) -> Result<String, AppError> {
    let now = Utc::now();
    let expires = now + Duration::days(7);

    let claims = Claims {
        sub: user_id.to_string(),
        exp: expires.timestamp() as usize,
        iat: now.timestamp() as usize,
        token_type: TokenType::Refresh,
    };

    jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("JWT encode error: {}", e)))
}

pub fn generate(user_id: &str, secret: &str) -> Result<(String, String), AppError> {
    Ok((
        generate_access_token(user_id, secret)?,
        generate_refresh_token(user_id, secret)?,
    ))
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

        let (access, refresh) = generate(user_id, secret).unwrap();
        let access_claims = verify(&access, secret).unwrap();
        let refresh_claims = verify(&refresh, secret).unwrap();

        assert_eq!(access_claims.sub, user_id);
        assert_eq!(access_claims.token_type, TokenType::Access);
        assert_eq!(refresh_claims.sub, user_id);
        assert_eq!(refresh_claims.token_type, TokenType::Refresh);
    }

    #[test]
    fn test_jwt_invalid_token() {
        let result = verify("invalid-token", "secret");
        assert!(result.is_err());
    }

    #[test]
    fn test_jwt_wrong_secret() {
        let (access, _) = generate("user1", "secret1").unwrap();
        let result = verify(&access, "secret2");
        assert!(result.is_err());
    }

    #[test]
    fn test_access_token_cannot_be_used_as_refresh() {
        let secret = "test-secret";
        let (access, _) = generate("user1", secret).unwrap();
        let claims = verify(&access, secret).unwrap();
        assert_eq!(claims.token_type, TokenType::Access);
    }

    #[test]
    fn test_refresh_token_has_longer_expiry() {
        let secret = "test-secret";
        let (access, refresh) = generate("user1", secret).unwrap();
        let access_claims = verify(&access, secret).unwrap();
        let refresh_claims = verify(&refresh, secret).unwrap();
        assert!(refresh_claims.exp > access_claims.exp);
    }
}
