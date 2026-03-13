use axum::extract::FromRef;
use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::http::request::Parts;
use jsonwebtoken::{DecodingKey, Validation, decode};
use serde::{Deserialize, Serialize};

use crate::db::models::UserRole;
use crate::web::{ApiError, AppState};

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Option<i64>,
    pub email: String,
    pub role: UserRole,
}

#[derive(Debug, Clone)]
pub struct RequireAdmin(pub AuthUser);

impl AuthUser {
    pub fn require_user_id(&self) -> Result<i64, ApiError> {
        self.user_id.ok_or_else(|| {
            ApiError::new(
                StatusCode::FORBIDDEN,
                "super admin account does not have message access",
            )
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Option<i64>,
    pub email: String,
    pub role: UserRole,
    pub exp: usize,
}

impl<S> FromRequestParts<S> for AuthUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);
        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| {
                ApiError::new(StatusCode::UNAUTHORIZED, "missing authorization header")
            })?;
        let token = header.strip_prefix("Bearer ").ok_or_else(|| {
            ApiError::new(StatusCode::UNAUTHORIZED, "invalid authorization scheme")
        })?;

        let decoded = decode::<Claims>(
            token,
            &DecodingKey::from_secret(app_state.config.web.jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|_| ApiError::new(StatusCode::UNAUTHORIZED, "invalid token"))?;

        Ok(AuthUser {
            user_id: decoded.claims.sub,
            email: decoded.claims.email,
            role: decoded.claims.role,
        })
    }
}

impl<S> FromRequestParts<S> for RequireAdmin
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let auth_user = AuthUser::from_request_parts(parts, state).await?;
        if auth_user.role != UserRole::SuperAdmin {
            return Err(ApiError::new(
                StatusCode::FORBIDDEN,
                "super admin role required",
            ));
        }
        Ok(Self(auth_user))
    }
}
