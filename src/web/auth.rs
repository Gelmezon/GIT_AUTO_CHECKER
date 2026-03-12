use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use bcrypt::{DEFAULT_COST, hash, verify};
use chrono::{Duration, Utc};
use jsonwebtoken::{EncodingKey, Header, encode};
use serde::{Deserialize, Serialize};

use crate::db::models::User;
use crate::web::middleware::{AuthUser, Claims};
use crate::web::{ApiError, AppState};

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct ActivateRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserResponse,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: i64,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
}

pub async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let user = state
        .database
        .get_user_by_email(&request.email)?
        .ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "invalid credentials"))?;
    let password_hash = user
        .password_hash
        .as_deref()
        .ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "invalid credentials"))?;
    let password_ok = verify(&request.password, password_hash)
        .map_err(|_| ApiError::new(StatusCode::UNAUTHORIZED, "invalid credentials"))?;
    if !password_ok {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid credentials",
        ));
    }

    Ok(Json(AuthResponse {
        token: issue_token(&state, &user)?,
        user: user.into(),
    }))
}

pub async fn activate(
    State(state): State<AppState>,
    Json(request): Json<ActivateRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let existing = state
        .database
        .get_user_by_email(&request.email)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "user not found"))?;
    if existing.password_hash.is_some() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "account already activated",
        ));
    }

    let password_hash = hash(&request.password, DEFAULT_COST)
        .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    let user = state
        .database
        .activate_user(&request.email, &password_hash)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "user not found"))?;

    Ok(Json(AuthResponse {
        token: issue_token(&state, &user)?,
        user: user.into(),
    }))
}

pub async fn me(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Json<UserResponse>, ApiError> {
    let user = state
        .database
        .get_user(auth_user.user_id)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "user not found"))?;
    Ok(Json(user.into()))
}

fn issue_token(state: &AppState, user: &User) -> Result<String, ApiError> {
    let exp =
        (Utc::now() + Duration::hours(state.config.web.token_expire_hours as i64)).timestamp();
    encode(
        &Header::default(),
        &Claims {
            sub: user.id,
            email: user.email.clone(),
            exp: exp as usize,
        },
        &EncodingKey::from_secret(state.config.web.jwt_secret.as_bytes()),
    )
    .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            email: user.email,
            display_name: user.display_name,
            avatar_url: user.avatar_url,
        }
    }
}
