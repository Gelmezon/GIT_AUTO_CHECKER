use std::collections::HashMap;
use std::path::Path;

use axum::Json;
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::db::models::{
    GitRepo, NewGitRepo, NewTask, NewUser, Task, TaskStatus, TaskType, UpdateGitRepo, UpdateUser,
    User,
};
use crate::db::next_run_from_cron;
use crate::mcp::tools::git::{GitCloneArgs, GitPullArgs, git_clone, git_pull};
use crate::web::middleware::RequireAdmin;
use crate::web::{ApiError, AppState};

#[derive(Debug, Serialize)]
pub struct AdminDashboardResponse {
    pub repo_count: i64,
    pub task_count: i64,
    pub user_count: i64,
    pub today_executed_count: i64,
    pub recent_tasks: Vec<AdminTaskResponse>,
}

#[derive(Debug, Serialize)]
pub struct AdminTaskListResponse {
    pub total: i64,
    pub page: usize,
    pub page_size: usize,
    pub items: Vec<AdminTaskResponse>,
}

#[derive(Debug, Serialize)]
pub struct RepoSyncResponse {
    pub action: String,
    pub branch: String,
    pub updated: bool,
    pub head: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AdminRepoResponse {
    pub id: i64,
    pub name: String,
    pub repo_url: String,
    pub branch: String,
    pub local_path: String,
    pub review_cron: Option<String>,
    pub last_commit: Option<String>,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct AdminUserResponse {
    pub id: i64,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub activated_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct AdminTaskResponse {
    pub id: i64,
    pub name: String,
    pub task_type: String,
    pub repo_id: Option<i64>,
    pub repo_name: Option<String>,
    pub prompt: String,
    pub cron_expr: Option<String>,
    pub scheduled_at: String,
    pub started_at: Option<String>,
    pub status: String,
    pub result: Option<String>,
    pub retry_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateRepoRequest {
    pub name: String,
    pub repo_url: String,
    #[serde(default = "default_branch")]
    pub branch: String,
    pub local_path: String,
    pub review_cron: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRepoRequest {
    pub name: String,
    pub repo_url: String,
    #[serde(default = "default_branch")]
    pub branch: String,
    pub local_path: String,
    pub review_cron: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub display_name: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub email: String,
    pub display_name: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub name: String,
    pub task_type: String,
    pub repo_id: Option<i64>,
    pub prompt: String,
    pub cron_expr: Option<String>,
    pub scheduled_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListTasksQuery {
    pub status: Option<String>,
    #[serde(default = "default_page")]
    pub page: usize,
    #[serde(default = "default_page_size")]
    pub page_size: usize,
}

pub async fn dashboard(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
) -> Result<Json<AdminDashboardResponse>, ApiError> {
    let repo_names = repo_name_map(&state)?;
    let recent_tasks = state
        .database
        .recent_task_runs(10)?
        .into_iter()
        .map(|task| task_to_response(task, &repo_names))
        .collect();

    info!(actor = %admin.email, "admin dashboard fetched");
    Ok(Json(AdminDashboardResponse {
        repo_count: state.database.repo_count()?,
        task_count: state.database.task_count()?,
        user_count: state.database.user_count()?,
        today_executed_count: state.database.today_executed_task_count()?,
        recent_tasks,
    }))
}

pub async fn list_repos(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
) -> Result<Json<Vec<AdminRepoResponse>>, ApiError> {
    info!(actor = %admin.email, "admin repo list fetched");
    Ok(Json(
        state
            .database
            .list_repos()?
            .into_iter()
            .map(repo_to_response)
            .collect(),
    ))
}

pub async fn create_repo(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
    Json(request): Json<CreateRepoRequest>,
) -> Result<(StatusCode, Json<AdminRepoResponse>), ApiError> {
    let id = state.database.insert_repo(&NewGitRepo {
        name: request.name,
        repo_url: request.repo_url,
        branch: request.branch,
        local_path: request.local_path,
        review_cron: normalize_optional(request.review_cron),
        enabled: request.enabled,
    })?;
    let repo = state
        .database
        .get_repo(id)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "repository not found after create"))?;

    info!(actor = %admin.email, repo_id = id, "admin repo created");
    Ok((StatusCode::CREATED, Json(repo_to_response(repo))))
}

pub async fn update_repo(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
    AxumPath(repo_id): AxumPath<i64>,
    Json(request): Json<UpdateRepoRequest>,
) -> Result<Json<AdminRepoResponse>, ApiError> {
    let updated = state.database.update_repo(
        repo_id,
        &UpdateGitRepo {
            name: request.name,
            repo_url: request.repo_url,
            branch: request.branch,
            local_path: request.local_path,
            review_cron: normalize_optional(request.review_cron),
            enabled: request.enabled,
        },
    )?;
    if !updated {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "repository not found"));
    }

    let repo = state
        .database
        .get_repo(repo_id)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "repository not found"))?;
    info!(actor = %admin.email, repo_id, "admin repo updated");
    Ok(Json(repo_to_response(repo)))
}

pub async fn delete_repo(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
    AxumPath(repo_id): AxumPath<i64>,
) -> Result<StatusCode, ApiError> {
    if !state.database.delete_repo(repo_id)? {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "repository not found"));
    }
    info!(actor = %admin.email, repo_id, "admin repo deleted");
    Ok(StatusCode::NO_CONTENT)
}

pub async fn sync_repo(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
    AxumPath(repo_id): AxumPath<i64>,
) -> Result<Json<RepoSyncResponse>, ApiError> {
    let repo = state
        .database
        .get_repo(repo_id)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "repository not found"))?;

    let git_dir = Path::new(&repo.local_path).join(".git");
    let response = if git_dir.exists() {
        let output = git_pull(&GitPullArgs {
            path: repo.local_path.clone(),
        })?;
        RepoSyncResponse {
            action: "pulled".to_string(),
            branch: output.branch,
            updated: output.updated,
            head: output.head,
        }
    } else {
        let output = git_clone(&GitCloneArgs {
            url: repo.repo_url.clone(),
            path: repo.local_path.clone(),
            branch: Some(repo.branch.clone()),
        })?;
        RepoSyncResponse {
            action: "cloned".to_string(),
            branch: output.branch,
            updated: true,
            head: output.head,
        }
    };

    info!(actor = %admin.email, repo_id, action = %response.action, "admin repo synced");
    Ok(Json(response))
}

pub async fn list_users(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
) -> Result<Json<Vec<AdminUserResponse>>, ApiError> {
    info!(actor = %admin.email, "admin user list fetched");
    Ok(Json(
        state
            .database
            .list_users()?
            .into_iter()
            .map(user_to_response)
            .collect(),
    ))
}

pub async fn create_user(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
    Json(request): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<AdminUserResponse>), ApiError> {
    let id = state.database.insert_user(&NewUser {
        email: request.email,
        display_name: request.display_name,
        password_hash: None,
        avatar_url: None,
    })?;
    let user = state
        .database
        .get_user(id)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "user not found after create"))?;

    info!(actor = %admin.email, user_id = id, "admin user created");
    Ok((StatusCode::CREATED, Json(user_to_response(user))))
}

pub async fn update_user(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
    AxumPath(user_id): AxumPath<i64>,
    Json(request): Json<UpdateUserRequest>,
) -> Result<Json<AdminUserResponse>, ApiError> {
    if !state.database.update_user(
        user_id,
        &UpdateUser {
            email: request.email,
            display_name: request.display_name,
        },
    )? {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "user not found"));
    }

    let user = state
        .database
        .get_user(user_id)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "user not found"))?;
    info!(actor = %admin.email, user_id, "admin user updated");
    Ok(Json(user_to_response(user)))
}

pub async fn delete_user(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
    AxumPath(user_id): AxumPath<i64>,
) -> Result<StatusCode, ApiError> {
    if !state.database.delete_user(user_id)? {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "user not found"));
    }
    info!(actor = %admin.email, user_id, "admin user deleted");
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_tasks(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
    Query(query): Query<ListTasksQuery>,
) -> Result<Json<AdminTaskListResponse>, ApiError> {
    let status = query
        .status
        .as_deref()
        .map(TaskStatus::from_db)
        .transpose()
        .map_err(|error| ApiError::new(StatusCode::BAD_REQUEST, error.to_string()))?;
    let page = query.page.max(1);
    let page_size = query.page_size.clamp(1, 100);
    let repo_names = repo_name_map(&state)?;
    let (tasks, total) = state
        .database
        .list_tasks_filtered(status, page, page_size)?;

    info!(actor = %admin.email, total, "admin task list fetched");
    Ok(Json(AdminTaskListResponse {
        total,
        page,
        page_size,
        items: tasks
            .into_iter()
            .map(|task| task_to_response(task, &repo_names))
            .collect(),
    }))
}

pub async fn create_task(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
    Json(request): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<AdminTaskResponse>), ApiError> {
    let task_type = TaskType::from_cli(&request.task_type)
        .map_err(|error| ApiError::new(StatusCode::BAD_REQUEST, error.to_string()))?;
    validate_task_repo(task_type, request.repo_id, &state)?;

    let scheduled_at = resolve_task_schedule(
        normalize_optional(request.cron_expr.clone()),
        request.scheduled_at.as_deref(),
    )?;
    let task_id = state.database.insert_task(&NewTask {
        name: request.name,
        task_type,
        repo_id: request.repo_id,
        prompt: request.prompt,
        cron_expr: normalize_optional(request.cron_expr),
        scheduled_at,
    })?;

    let repo_names = repo_name_map(&state)?;
    let task = state
        .database
        .list_tasks()?
        .into_iter()
        .find(|task| task.id == task_id)
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "task not found after create"))?;
    info!(actor = %admin.email, task_id, "admin task created");
    Ok((
        StatusCode::CREATED,
        Json(task_to_response(task, &repo_names)),
    ))
}

pub async fn delete_task(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
    AxumPath(task_id): AxumPath<i64>,
) -> Result<StatusCode, ApiError> {
    if !state.database.delete_task(task_id)? {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "task not found"));
    }
    info!(actor = %admin.email, task_id, "admin task deleted");
    Ok(StatusCode::NO_CONTENT)
}

fn validate_task_repo(
    task_type: TaskType,
    repo_id: Option<i64>,
    state: &AppState,
) -> Result<(), ApiError> {
    if matches!(task_type, TaskType::GitReview | TaskType::TestGen) {
        let repo_id = repo_id.ok_or_else(|| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "repo_id is required for repository tasks",
            )
        })?;
        if state.database.get_repo(repo_id)?.is_none() {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "repository not found",
            ));
        }
    }
    Ok(())
}

fn resolve_task_schedule(
    cron_expr: Option<String>,
    scheduled_at: Option<&str>,
) -> Result<DateTime<Utc>, ApiError> {
    if let Some(raw) = scheduled_at {
        return parse_datetime(raw);
    }
    if let Some(expr) = cron_expr.as_deref() {
        return next_run_from_cron(expr, Utc::now())
            .map_err(|error| ApiError::new(StatusCode::BAD_REQUEST, error.to_string()));
    }
    Ok(Utc::now())
}

fn parse_datetime(input: &str) -> Result<DateTime<Utc>, ApiError> {
    DateTime::parse_from_rfc3339(input)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid scheduled_at datetime"))
}

fn repo_name_map(state: &AppState) -> Result<HashMap<i64, String>, ApiError> {
    Ok(state
        .database
        .list_repos()?
        .into_iter()
        .map(|repo| (repo.id, repo.name))
        .collect())
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn repo_to_response(repo: GitRepo) -> AdminRepoResponse {
    AdminRepoResponse {
        id: repo.id,
        name: repo.name,
        repo_url: repo.repo_url,
        branch: repo.branch,
        local_path: repo.local_path,
        review_cron: repo.review_cron,
        last_commit: repo.last_commit,
        enabled: repo.enabled,
        created_at: repo.created_at.to_rfc3339(),
        updated_at: repo.updated_at.to_rfc3339(),
    }
}

fn user_to_response(user: User) -> AdminUserResponse {
    AdminUserResponse {
        id: user.id,
        email: user.email,
        display_name: user.display_name,
        avatar_url: user.avatar_url,
        activated_at: user.activated_at.map(|value| value.to_rfc3339()),
        created_at: user.created_at.to_rfc3339(),
        updated_at: user.updated_at.to_rfc3339(),
    }
}

fn task_to_response(task: Task, repo_names: &HashMap<i64, String>) -> AdminTaskResponse {
    AdminTaskResponse {
        id: task.id,
        name: task.name,
        task_type: task.task_type.as_str().to_string(),
        repo_id: task.repo_id,
        repo_name: task
            .repo_id
            .and_then(|repo_id| repo_names.get(&repo_id).cloned()),
        prompt: task.prompt,
        cron_expr: task.cron_expr,
        scheduled_at: task.scheduled_at.to_rfc3339(),
        started_at: task.started_at.map(|value| value.to_rfc3339()),
        status: task.status.as_str().to_string(),
        result: task.result,
        retry_count: task.retry_count,
        created_at: task.created_at.to_rfc3339(),
        updated_at: task.updated_at.to_rfc3339(),
    }
}

fn default_page() -> usize {
    1
}

fn default_page_size() -> usize {
    20
}

fn default_true() -> bool {
    true
}

fn default_branch() -> String {
    "main".to_string()
}
