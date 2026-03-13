pub mod admin;
pub mod auth;
pub mod messages;
pub mod middleware;

use std::sync::Arc;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Json, Router, routing::get, routing::post, routing::put};
use serde_json::json;

use crate::config::AppConfig;
use crate::db::Database;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub database: Database,
}

pub fn api_router() -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(auth::login))
        .route("/auth/activate", post(auth::activate))
        .route("/me", get(auth::me))
        .route("/messages", get(messages::list_messages))
        .route("/messages/{id}", get(messages::get_message))
        .route("/messages/{id}/read", put(messages::mark_message_read))
        .route("/messages/read-all", put(messages::mark_all_messages_read))
        .route("/messages/unread-count", get(messages::unread_count))
        .route("/admin/dashboard", get(admin::dashboard))
        .route(
            "/admin/repos",
            get(admin::list_repos).post(admin::create_repo),
        )
        .route(
            "/admin/repos/{id}",
            put(admin::update_repo).delete(admin::delete_repo),
        )
        .route("/admin/repos/{id}/sync", post(admin::sync_repo))
        .route(
            "/admin/users",
            get(admin::list_users).post(admin::create_user),
        )
        .route(
            "/admin/users/{id}",
            put(admin::update_user).delete(admin::delete_user),
        )
        .route(
            "/admin/tasks",
            get(admin::list_tasks).post(admin::create_task),
        )
        .route(
            "/admin/tasks/{id}",
            axum::routing::delete(admin::delete_task),
        )
}

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
    }
}
