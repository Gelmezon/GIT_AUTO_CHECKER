use axum::extract::FromRef;
use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::http::request::Parts;
use jsonwebtoken::{DecodingKey, Validation, decode};
use serde::{Deserialize, Serialize};

use crate::web::{ApiError, AppState};

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: i64,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: i64,
    pub email: String,
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
        })
    }
}
