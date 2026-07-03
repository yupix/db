use axum::{
    Json, Router,
    extract::State,
    http::header,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use validator::Validate;

use crate::auth::{jwt, password};
use crate::db::models::User;
use crate::error::AppError;
use crate::state::AppState;

const ACCESS_TOKEN_COOKIE: &str = "access_token";
const REFRESH_TOKEN_COOKIE: &str = "refresh_token";

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/me", get(me))
        .route("/refresh", post(refresh))
        .route("/logout", post(logout))
}

#[derive(Deserialize, Validate)]
struct RegisterRequest {
    #[validate(email)]
    email: String,
    #[validate(length(min = 8))]
    password: String,
    #[validate(length(min = 1, max = 100))]
    name: String,
}

#[derive(Serialize)]
struct AuthResponse {
    user: UserResponse,
}

#[derive(Serialize)]
struct UserResponse {
    id: String,
    email: String,
    name: String,
}

fn build_auth_response(access_token: &str, refresh_token: &str, user: UserResponse) -> Response {
    let access_cookie = format!(
        "{}={}; HttpOnly; SameSite=Lax; Path=/; Max-Age=900",
        ACCESS_TOKEN_COOKIE, access_token
    );
    let refresh_cookie = format!(
        "{}={}; HttpOnly; SameSite=Lax; Path=/api/auth/refresh; Max-Age=604800",
        REFRESH_TOKEN_COOKIE, refresh_token
    );

    let mut response = Json(AuthResponse { user }).into_response();
    let headers = response.headers_mut();
    headers.append(header::SET_COOKIE, access_cookie.parse().unwrap());
    headers.append(header::SET_COOKIE, refresh_cookie.parse().unwrap());
    response
}

async fn register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterRequest>,
) -> Result<Response, AppError> {
    req.validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let existing = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE email = $1")
        .bind(&req.email)
        .fetch_one(&state.db)
        .await?;

    if existing > 0 {
        return Err(AppError::Conflict("Email already registered".into()));
    }

    let hashed = password::hash(&req.password)?;
    let user = sqlx::query_as::<_, User>(
        "INSERT INTO users (email, password, name) VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(&req.email)
    .bind(&hashed)
    .bind(&req.name)
    .fetch_one(&state.db)
    .await?;

    let (access_token, refresh_token) =
        jwt::generate(&user.id.to_string(), &state.config.jwt_secret)?;

    Ok(build_auth_response(
        &access_token,
        &refresh_token,
        UserResponse {
            id: user.id.to_string(),
            email: user.email,
            name: user.name,
        },
    ))
}

#[derive(Deserialize, Validate)]
struct LoginRequest {
    #[validate(email)]
    email: String,
    password: String,
}

async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Result<Response, AppError> {
    req.validate()
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
        .bind(&req.email)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;

    if !password::verify(&req.password, &user.password)? {
        return Err(AppError::Unauthorized);
    }

    let (access_token, refresh_token) =
        jwt::generate(&user.id.to_string(), &state.config.jwt_secret)?;

    Ok(build_auth_response(
        &access_token,
        &refresh_token,
        UserResponse {
            id: user.id.to_string(),
            email: user.email,
            name: user.name,
        },
    ))
}

async fn me(
    State(_state): State<Arc<AppState>>,
    claims: jwt::Claims,
) -> Result<Json<UserResponse>, AppError> {
    let user_id: uuid::Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&_state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(Json(UserResponse {
        id: user.id.to_string(),
        email: user.email,
        name: user.name,
    }))
}

async fn refresh(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Response, AppError> {
    let refresh_token = extract_cookie(&headers, REFRESH_TOKEN_COOKIE)
        .ok_or(AppError::BadRequest("Missing refresh_token cookie".into()))?;

    let claims = jwt::verify(&refresh_token, &state.config.jwt_secret)?;

    if claims.token_type != jwt::TokenType::Refresh {
        return Err(AppError::BadRequest("Invalid token type".into()));
    }

    let user_id: uuid::Uuid = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let (access_token, refresh_token) =
        jwt::generate(&user.id.to_string(), &state.config.jwt_secret)?;

    Ok(build_auth_response(
        &access_token,
        &refresh_token,
        UserResponse {
            id: user.id.to_string(),
            email: user.email,
            name: user.name,
        },
    ))
}

fn extract_cookie(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|c| {
                let c = c.trim();
                c.strip_prefix(&format!("{}=", name)).map(|s| s.to_string())
            })
        })
}

async fn logout() -> Response {
    let access_cookie = format!(
        "{}=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0",
        ACCESS_TOKEN_COOKIE
    );
    let refresh_cookie = format!(
        "{}=; HttpOnly; SameSite=Lax; Path=/api/auth/refresh; Max-Age=0",
        REFRESH_TOKEN_COOKIE
    );

    let mut response = Json(serde_json::json!({ "logged_out": true })).into_response();
    let headers = response.headers_mut();
    headers.append(header::SET_COOKIE, access_cookie.parse().unwrap());
    headers.append(header::SET_COOKIE, refresh_cookie.parse().unwrap());
    response
}
