use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

use crate::web::middleware::AuthUser;
use crate::web::{ApiError, AppState};

#[derive(Debug, Deserialize)]
pub struct ListMessagesQuery {
    #[serde(default)]
    pub unread: bool,
    #[serde(default = "default_page")]
    pub page: usize,
    #[serde(default = "default_page_size")]
    pub page_size: usize,
}

#[derive(Debug, Serialize)]
pub struct MessagesResponse {
    pub total: i64,
    pub unread_count: i64,
    pub page: usize,
    pub page_size: usize,
    pub items: Vec<MessageListItem>,
}

#[derive(Debug, Serialize)]
pub struct MessageListItem {
    pub id: i64,
    pub title: String,
    pub repo_name: Option<String>,
    pub summary: String,
    pub commit_range: Option<String>,
    pub is_read: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct MessageDetailResponse {
    pub id: i64,
    pub title: String,
    pub repo_name: Option<String>,
    pub content: String,
    pub report_path: Option<String>,
    pub commit_range: Option<String>,
    pub is_read: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct ReadAllResponse {
    pub updated: usize,
}

#[derive(Debug, Serialize)]
pub struct UnreadCountResponse {
    pub unread_count: i64,
}

pub async fn list_messages(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<ListMessagesQuery>,
) -> Result<Json<MessagesResponse>, ApiError> {
    let user_id = auth_user.require_user_id()?;
    let page_size = query.page_size.clamp(1, 100);
    let (messages, total, unread_count) =
        state
            .database
            .list_messages(user_id, query.unread, query.page.max(1), page_size)?;

    Ok(Json(MessagesResponse {
        total,
        unread_count,
        page: query.page.max(1),
        page_size,
        items: messages
            .into_iter()
            .map(|message| MessageListItem {
                id: message.id,
                title: message.title,
                repo_name: message.repo_name,
                summary: message.summary,
                commit_range: message.commit_range,
                is_read: message.is_read,
                created_at: message.created_at.to_rfc3339(),
            })
            .collect(),
    }))
}

pub async fn get_message(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(message_id): Path<i64>,
) -> Result<Json<MessageDetailResponse>, ApiError> {
    let user_id = auth_user.require_user_id()?;
    state.database.mark_message_read(user_id, message_id)?;
    let message = state
        .database
        .get_message(user_id, message_id)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "message not found"))?;

    Ok(Json(MessageDetailResponse {
        id: message.id,
        title: message.title,
        repo_name: message.repo_name,
        content: message.content,
        report_path: message.report_path,
        commit_range: message.commit_range,
        is_read: message.is_read || message.read_at.is_some(),
        created_at: message.created_at.to_rfc3339(),
    }))
}

pub async fn mark_message_read(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(message_id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let user_id = auth_user.require_user_id()?;
    let updated = state.database.mark_message_read(user_id, message_id)?;
    if !updated {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "message not found"));
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn mark_all_messages_read(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Json<ReadAllResponse>, ApiError> {
    let updated = state
        .database
        .mark_all_messages_read(auth_user.require_user_id()?)?;
    Ok(Json(ReadAllResponse { updated }))
}

pub async fn unread_count(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Json<UnreadCountResponse>, ApiError> {
    let unread_count = state
        .database
        .unread_message_count(auth_user.require_user_id()?)?;
    Ok(Json(UnreadCountResponse { unread_count }))
}

fn default_page() -> usize {
    1
}

fn default_page_size() -> usize {
    20
}
