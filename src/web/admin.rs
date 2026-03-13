use std::collections::HashMap;
use std::path::Path;

use axum::Json;
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::credentials::CredentialCipher;
use crate::db::models::{
    GitAuthType, GitCredential, GitPlatform, GitRepo, NewGitCredential, NewGitRepo, NewTask,
    NewUser, Task, TaskDefinition, TaskDefinitionStatus, TaskRun, TaskStatus, TaskType,
    UpdateGitCredential, UpdateGitRepo, UpdateTask, UpdateUser, User,
};
use crate::db::next_run_from_cron;
use crate::git_auth::{resolve_repo_auth_by_local_path, validate_git_credential_payload};
use crate::mcp::tools::git::{GitCloneArgs, GitPullArgs, git_clone, git_pull};
use crate::web::middleware::RequireAdmin;
use crate::web::{ApiError, AppState};

#[derive(Debug, Serialize)]
pub struct AdminDashboardResponse {
    pub repo_count: i64,
    pub task_count: i64,
    pub user_count: i64,
    pub today_executed_count: i64,
    pub recent_runs: Vec<AdminTaskRunResponse>,
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
    pub credential_id: Option<i64>,
    pub credential_name: Option<String>,
    pub last_commit: Option<String>,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct AdminCredentialResponse {
    pub id: i64,
    pub name: String,
    pub platform: String,
    pub auth_type: String,
    pub username: Option<String>,
    pub ssh_key_path: Option<String>,
    pub has_token: bool,
    pub has_password: bool,
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
    pub status: String,
    pub last_run_at: Option<String>,
    pub last_run_status: Option<String>,
    pub next_run_at: Option<String>,
    pub total_runs: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct AdminTaskRunResponse {
    pub id: i64,
    pub task_id: i64,
    pub task_name: String,
    pub repo_name: Option<String>,
    pub scheduled_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub status: String,
    pub result: Option<String>,
    pub log: Option<String>,
    pub retry_count: i64,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct AdminTaskRunListResponse {
    pub total: i64,
    pub page: usize,
    pub page_size: usize,
    pub items: Vec<AdminTaskRunResponse>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRepoRequest {
    pub name: String,
    pub repo_url: String,
    #[serde(default = "default_branch")]
    pub branch: String,
    pub local_path: String,
    pub review_cron: Option<String>,
    pub credential_id: Option<i64>,
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
    pub credential_id: Option<i64>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateCredentialRequest {
    pub name: String,
    pub platform: String,
    pub auth_type: String,
    pub token: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub ssh_key_path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCredentialRequest {
    pub name: String,
    pub platform: String,
    pub auth_type: String,
    pub token: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub ssh_key_path: Option<String>,
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
pub struct UpdateTaskRequest {
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
    pub task_type: Option<String>,
    #[serde(default = "default_page")]
    pub page: usize,
    #[serde(default = "default_page_size")]
    pub page_size: usize,
}

#[derive(Debug, Deserialize)]
pub struct ListTaskRunsQuery {
    #[serde(default = "default_page")]
    pub page: usize,
    #[serde(default = "default_page_size")]
    pub page_size: usize,
}

#[derive(Debug, Deserialize)]
pub struct ListRunsQuery {
    pub status: Option<String>,
    pub task_id: Option<i64>,
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
    let recent_runs = state
        .database
        .recent_task_runs(10)?
        .into_iter()
        .map(|task| execution_to_run_response(task, &repo_names))
        .collect();

    info!(actor = %admin.email, "admin dashboard fetched");
    Ok(Json(AdminDashboardResponse {
        repo_count: state.database.repo_count()?,
        task_count: state.database.task_count()?,
        user_count: state.database.user_count()?,
        today_executed_count: state.database.today_executed_task_count()?,
        recent_runs,
    }))
}

pub async fn list_repos(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
) -> Result<Json<Vec<AdminRepoResponse>>, ApiError> {
    let credential_names = credential_name_map(&state)?;
    info!(actor = %admin.email, "admin repo list fetched");
    Ok(Json(
        state
            .database
            .list_repos()?
            .into_iter()
            .map(|repo| repo_to_response(repo, &credential_names))
            .collect(),
    ))
}

pub async fn list_credentials(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
) -> Result<Json<Vec<AdminCredentialResponse>>, ApiError> {
    info!(actor = %admin.email, "admin credential list fetched");
    Ok(Json(
        state
            .database
            .list_git_credentials()?
            .into_iter()
            .map(credential_to_response)
            .collect(),
    ))
}

pub async fn get_credential(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
    AxumPath(credential_id): AxumPath<i64>,
) -> Result<Json<AdminCredentialResponse>, ApiError> {
    let credential = state
        .database
        .get_git_credential(credential_id)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "credential not found"))?;
    info!(actor = %admin.email, credential_id, "admin credential fetched");
    Ok(Json(credential_to_response(credential)))
}

pub async fn get_repo(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
    AxumPath(repo_id): AxumPath<i64>,
) -> Result<Json<AdminRepoResponse>, ApiError> {
    let credential_names = credential_name_map(&state)?;
    let repo = state
        .database
        .get_repo(repo_id)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "repository not found"))?;
    info!(actor = %admin.email, repo_id, "admin repo fetched");
    Ok(Json(repo_to_response(repo, &credential_names)))
}

pub async fn create_repo(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
    Json(request): Json<CreateRepoRequest>,
) -> Result<(StatusCode, Json<AdminRepoResponse>), ApiError> {
    let review_cron = normalize_optional(request.review_cron);
    validate_cron_field(review_cron.as_deref(), "review_cron")?;
    validate_repo_credential(request.credential_id, &state)?;

    let id = state.database.insert_repo(&NewGitRepo {
        name: request.name,
        repo_url: request.repo_url,
        branch: request.branch,
        local_path: request.local_path,
        review_cron,
        credential_id: request.credential_id,
        enabled: request.enabled,
    })?;
    let repo = state
        .database
        .get_repo(id)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "repository not found after create"))?;
    let credential_names = credential_name_map(&state)?;

    info!(actor = %admin.email, repo_id = id, "admin repo created");
    Ok((
        StatusCode::CREATED,
        Json(repo_to_response(repo, &credential_names)),
    ))
}

pub async fn update_repo(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
    AxumPath(repo_id): AxumPath<i64>,
    Json(request): Json<UpdateRepoRequest>,
) -> Result<Json<AdminRepoResponse>, ApiError> {
    let review_cron = normalize_optional(request.review_cron);
    validate_cron_field(review_cron.as_deref(), "review_cron")?;
    validate_repo_credential(request.credential_id, &state)?;

    let updated = state.database.update_repo(
        repo_id,
        &UpdateGitRepo {
            name: request.name,
            repo_url: request.repo_url,
            branch: request.branch,
            local_path: request.local_path,
            review_cron,
            credential_id: request.credential_id,
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
    let credential_names = credential_name_map(&state)?;
    info!(actor = %admin.email, repo_id, "admin repo updated");
    Ok(Json(repo_to_response(repo, &credential_names)))
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
    let auth = resolve_repo_auth_by_local_path(&state.database, &state.config, &repo.local_path)?;
    let response = if git_dir.exists() {
        let output = git_pull(
            &GitPullArgs {
                path: repo.local_path.clone(),
            },
            auth.as_ref(),
        )?;
        RepoSyncResponse {
            action: "pulled".to_string(),
            branch: output.branch,
            updated: output.updated,
            head: output.head,
        }
    } else {
        let output = git_clone(
            &GitCloneArgs {
                url: repo.repo_url.clone(),
                path: repo.local_path.clone(),
                branch: Some(repo.branch.clone()),
            },
            auth.as_ref(),
        )?;
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

pub async fn create_credential(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
    Json(request): Json<CreateCredentialRequest>,
) -> Result<(StatusCode, Json<AdminCredentialResponse>), ApiError> {
    let payload = build_new_credential_payload(&state, request)?;
    let id = state.database.insert_git_credential(&payload)?;
    let credential = state
        .database
        .get_git_credential(id)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "credential not found after create"))?;
    info!(actor = %admin.email, credential_id = id, "admin credential created");
    Ok((
        StatusCode::CREATED,
        Json(credential_to_response(credential)),
    ))
}

pub async fn update_credential(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
    AxumPath(credential_id): AxumPath<i64>,
    Json(request): Json<UpdateCredentialRequest>,
) -> Result<Json<AdminCredentialResponse>, ApiError> {
    let existing = state
        .database
        .get_git_credential(credential_id)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "credential not found"))?;
    let payload = build_updated_credential_payload(&state, request, &existing)?;
    if !state
        .database
        .update_git_credential(credential_id, &payload)?
    {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "credential not found"));
    }

    let credential = state
        .database
        .get_git_credential(credential_id)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "credential not found"))?;
    info!(actor = %admin.email, credential_id, "admin credential updated");
    Ok(Json(credential_to_response(credential)))
}

pub async fn delete_credential(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
    AxumPath(credential_id): AxumPath<i64>,
) -> Result<StatusCode, ApiError> {
    if !state.database.delete_git_credential(credential_id)? {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "credential not found"));
    }
    info!(actor = %admin.email, credential_id, "admin credential deleted");
    Ok(StatusCode::NO_CONTENT)
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

pub async fn get_user(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
    AxumPath(user_id): AxumPath<i64>,
) -> Result<Json<AdminUserResponse>, ApiError> {
    let user = state
        .database
        .get_user(user_id)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "user not found"))?;
    info!(actor = %admin.email, user_id, "admin user fetched");
    Ok(Json(user_to_response(user)))
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
        .map(TaskDefinitionStatus::from_db)
        .transpose()
        .map_err(|error| ApiError::new(StatusCode::BAD_REQUEST, error.to_string()))?;
    let task_type = query
        .task_type
        .as_deref()
        .map(TaskType::from_cli)
        .transpose()
        .map_err(|error| ApiError::new(StatusCode::BAD_REQUEST, error.to_string()))?;
    let page = query.page.max(1);
    let page_size = query.page_size.clamp(1, 100);
    let repo_names = repo_name_map(&state)?;
    let (tasks, total) = state
        .database
        .list_tasks_filtered(status, task_type, page, page_size)?;

    info!(actor = %admin.email, total, "admin task list fetched");
    Ok(Json(AdminTaskListResponse {
        total,
        page,
        page_size,
        items: tasks
            .into_iter()
            .map(|task| task_to_response(&state, task, &repo_names))
            .collect::<Result<Vec<_>, _>>()?,
    }))
}

pub async fn get_task(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
    AxumPath(task_id): AxumPath<i64>,
) -> Result<Json<AdminTaskResponse>, ApiError> {
    let repo_names = repo_name_map(&state)?;
    let task = state
        .database
        .get_task(task_id)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "task not found"))?;
    info!(actor = %admin.email, task_id, "admin task fetched");
    Ok(Json(task_to_response(&state, task, &repo_names)?))
}

pub async fn list_task_runs(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
    AxumPath(task_id): AxumPath<i64>,
    Query(query): Query<ListTaskRunsQuery>,
) -> Result<Json<AdminTaskRunListResponse>, ApiError> {
    if state.database.get_task(task_id)?.is_none() {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "task not found"));
    }
    let page = query.page.max(1);
    let page_size = query.page_size.clamp(1, 100);
    let repo_names = repo_name_map(&state)?;
    let task_map = task_definition_map(&state)?;
    let (runs, total) = state.database.list_task_runs(task_id, page, page_size)?;

    info!(actor = %admin.email, task_id, total, "admin task runs fetched");
    Ok(Json(AdminTaskRunListResponse {
        total,
        page,
        page_size,
        items: runs
            .into_iter()
            .map(|run| task_run_to_response(run, &task_map, &repo_names))
            .collect::<Result<Vec<_>, _>>()?,
    }))
}

pub async fn list_runs(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
    Query(query): Query<ListRunsQuery>,
) -> Result<Json<AdminTaskRunListResponse>, ApiError> {
    let status = query
        .status
        .as_deref()
        .map(TaskStatus::from_db)
        .transpose()
        .map_err(|error| ApiError::new(StatusCode::BAD_REQUEST, error.to_string()))?;
    let page = query.page.max(1);
    let page_size = query.page_size.clamp(1, 100);
    let repo_names = repo_name_map(&state)?;
    let task_map = task_definition_map(&state)?;
    let (runs, total) = state
        .database
        .list_all_task_runs(status, query.task_id, page, page_size)?;

    info!(actor = %admin.email, total, "admin run list fetched");
    Ok(Json(AdminTaskRunListResponse {
        total,
        page,
        page_size,
        items: runs
            .into_iter()
            .map(|run| task_run_to_response(run, &task_map, &repo_names))
            .collect::<Result<Vec<_>, _>>()?,
    }))
}

pub async fn get_run(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
    AxumPath(run_id): AxumPath<i64>,
) -> Result<Json<AdminTaskRunResponse>, ApiError> {
    let repo_names = repo_name_map(&state)?;
    let task_map = task_definition_map(&state)?;
    let run = state
        .database
        .get_task_run(run_id)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "task run not found"))?;
    info!(actor = %admin.email, run_id, "admin task run fetched");
    Ok(Json(task_run_to_response(run, &task_map, &repo_names)?))
}

pub async fn cancel_run(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
    AxumPath(run_id): AxumPath<i64>,
) -> Result<Json<AdminTaskRunResponse>, ApiError> {
    if !state.database.cancel_task_run(run_id)? {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "task run is not pending or does not exist",
        ));
    }
    let repo_names = repo_name_map(&state)?;
    let task_map = task_definition_map(&state)?;
    let run = state
        .database
        .get_task_run(run_id)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "task run not found"))?;
    info!(actor = %admin.email, run_id, "admin task run cancelled");
    Ok(Json(task_run_to_response(run, &task_map, &repo_names)?))
}

pub async fn create_task(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
    Json(request): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<AdminTaskResponse>), ApiError> {
    let task_type = TaskType::from_cli(&request.task_type)
        .map_err(|error| ApiError::new(StatusCode::BAD_REQUEST, error.to_string()))?;
    validate_task_repo(task_type, request.repo_id, &state)?;
    let cron_expr = normalize_optional(request.cron_expr);
    validate_cron_field(cron_expr.as_deref(), "cron_expr")?;

    let scheduled_at = resolve_task_schedule(cron_expr.clone(), request.scheduled_at.as_deref())?;
    let task_id = state.database.insert_task(&NewTask {
        name: request.name,
        task_type,
        repo_id: request.repo_id,
        prompt: request.prompt,
        cron_expr,
        scheduled_at,
    })?;

    let repo_names = repo_name_map(&state)?;
    let task = state
        .database
        .get_task(task_id)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "task not found after create"))?;
    info!(actor = %admin.email, task_id, "admin task created");
    Ok((
        StatusCode::CREATED,
        Json(task_to_response(&state, task, &repo_names)?),
    ))
}

pub async fn update_task(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
    AxumPath(task_id): AxumPath<i64>,
    Json(request): Json<UpdateTaskRequest>,
) -> Result<Json<AdminTaskResponse>, ApiError> {
    let task_type = TaskType::from_cli(&request.task_type)
        .map_err(|error| ApiError::new(StatusCode::BAD_REQUEST, error.to_string()))?;
    validate_task_repo(task_type, request.repo_id, &state)?;
    let cron_expr = normalize_optional(request.cron_expr);
    validate_cron_field(cron_expr.as_deref(), "cron_expr")?;
    let scheduled_at = resolve_task_schedule(cron_expr.clone(), request.scheduled_at.as_deref())?;

    if !state.database.update_task(
        task_id,
        &UpdateTask {
            name: request.name,
            task_type,
            repo_id: request.repo_id,
            prompt: request.prompt,
            cron_expr,
            scheduled_at,
        },
    )? {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "task not found"));
    }

    let repo_names = repo_name_map(&state)?;
    let task = state
        .database
        .get_task(task_id)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "task not found"))?;
    info!(actor = %admin.email, task_id, "admin task updated");
    Ok(Json(task_to_response(&state, task, &repo_names)?))
}

pub async fn pause_task(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
    AxumPath(task_id): AxumPath<i64>,
) -> Result<Json<AdminTaskResponse>, ApiError> {
    if !state.database.pause_task(task_id)? {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "task not found"));
    }
    let repo_names = repo_name_map(&state)?;
    let task = state
        .database
        .get_task(task_id)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "task not found"))?;
    info!(actor = %admin.email, task_id, "admin task paused");
    Ok(Json(task_to_response(&state, task, &repo_names)?))
}

pub async fn resume_task(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
    AxumPath(task_id): AxumPath<i64>,
) -> Result<Json<AdminTaskResponse>, ApiError> {
    if !state.database.resume_task(task_id)? {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "task not found"));
    }
    let repo_names = repo_name_map(&state)?;
    let task = state
        .database
        .get_task(task_id)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "task not found"))?;
    info!(actor = %admin.email, task_id, "admin task resumed");
    Ok(Json(task_to_response(&state, task, &repo_names)?))
}

pub async fn trigger_task(
    State(state): State<AppState>,
    RequireAdmin(admin): RequireAdmin,
    AxumPath(task_id): AxumPath<i64>,
) -> Result<Json<AdminTaskRunResponse>, ApiError> {
    let run_id = state
        .database
        .trigger_task(task_id)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "task not found"))?;
    let repo_names = repo_name_map(&state)?;
    let task_map = task_definition_map(&state)?;
    let run = state
        .database
        .get_task_run(run_id)?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "task run not found"))?;
    info!(actor = %admin.email, task_id, run_id, "admin task triggered");
    Ok(Json(task_run_to_response(run, &task_map, &repo_names)?))
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

fn validate_repo_credential(credential_id: Option<i64>, state: &AppState) -> Result<(), ApiError> {
    if let Some(credential_id) = credential_id {
        if state.database.get_git_credential(credential_id)?.is_none() {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "credential not found",
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

fn validate_cron_field(expr: Option<&str>, field: &str) -> Result<(), ApiError> {
    if let Some(expr) = expr {
        next_run_from_cron(expr, Utc::now()).map_err(|error| {
            ApiError::new(StatusCode::BAD_REQUEST, format!("invalid {field}: {error}"))
        })?;
    }
    Ok(())
}

fn parse_git_platform(input: &str) -> Result<GitPlatform, ApiError> {
    GitPlatform::from_db(input.trim())
        .map_err(|error| ApiError::new(StatusCode::BAD_REQUEST, error.to_string()))
}

fn parse_git_auth_type(input: &str) -> Result<GitAuthType, ApiError> {
    GitAuthType::from_db(input.trim())
        .map_err(|error| ApiError::new(StatusCode::BAD_REQUEST, error.to_string()))
}

fn build_new_credential_payload(
    state: &AppState,
    request: CreateCredentialRequest,
) -> Result<NewGitCredential, ApiError> {
    let platform = parse_git_platform(&request.platform)?;
    let auth_type = parse_git_auth_type(&request.auth_type)?;
    let cipher = CredentialCipher::from_hex_key(&state.config.credentials.encryption_key)
        .map_err(ApiError::from)?;

    let normalized_username = normalize_optional(request.username);
    let normalized_token = normalize_optional(request.token);
    let normalized_password = normalize_optional(request.password);
    let normalized_ssh_key_path = normalize_optional(request.ssh_key_path);

    validate_git_credential_payload(
        auth_type,
        platform,
        normalized_username.as_deref(),
        normalized_token.as_deref(),
        normalized_password.as_deref(),
        normalized_ssh_key_path.as_deref(),
    )
    .map_err(|error| ApiError::new(StatusCode::BAD_REQUEST, error.to_string()))?;

    let (token, password, ssh_key_path) = match auth_type {
        GitAuthType::Token => (
            normalized_token
                .as_deref()
                .map(|value| encrypt_secret(&cipher, value))
                .transpose()?,
            None,
            None,
        ),
        GitAuthType::Basic => (
            None,
            normalized_password
                .as_deref()
                .map(|value| encrypt_secret(&cipher, value))
                .transpose()?,
            None,
        ),
        GitAuthType::Ssh => (None, None, normalized_ssh_key_path),
    };

    Ok(NewGitCredential {
        name: request.name,
        platform,
        auth_type,
        token,
        username: normalized_username,
        password,
        ssh_key_path,
    })
}

fn build_updated_credential_payload(
    state: &AppState,
    request: UpdateCredentialRequest,
    existing: &GitCredential,
) -> Result<UpdateGitCredential, ApiError> {
    let platform = parse_git_platform(&request.platform)?;
    let auth_type = parse_git_auth_type(&request.auth_type)?;
    let cipher = CredentialCipher::from_hex_key(&state.config.credentials.encryption_key)
        .map_err(ApiError::from)?;

    let username = normalize_optional(request.username);
    let ssh_key_path = normalize_optional(request.ssh_key_path);
    let token_input = normalize_optional(request.token);
    let password_input = normalize_optional(request.password);

    let (token, password, ssh_key_path) = match auth_type {
        GitAuthType::Token => {
            let token = match token_input {
                Some(token) => Some(encrypt_secret(&cipher, &token)?),
                None => existing.token.clone(),
            };
            validate_git_credential_payload(
                auth_type,
                platform,
                username.as_deref(),
                token.as_deref(),
                None,
                None,
            )
            .map_err(|error| ApiError::new(StatusCode::BAD_REQUEST, error.to_string()))?;
            (token, None, None)
        }
        GitAuthType::Basic => {
            let password = match password_input {
                Some(password) => Some(encrypt_secret(&cipher, &password)?),
                None => existing.password.clone(),
            };
            validate_git_credential_payload(
                auth_type,
                platform,
                username.as_deref(),
                None,
                password.as_deref(),
                None,
            )
            .map_err(|error| ApiError::new(StatusCode::BAD_REQUEST, error.to_string()))?;
            (None, password, None)
        }
        GitAuthType::Ssh => {
            validate_git_credential_payload(
                auth_type,
                platform,
                username.as_deref(),
                None,
                None,
                ssh_key_path.as_deref(),
            )
            .map_err(|error| ApiError::new(StatusCode::BAD_REQUEST, error.to_string()))?;
            (None, None, ssh_key_path)
        }
    };

    Ok(UpdateGitCredential {
        name: request.name,
        platform,
        auth_type,
        token,
        username,
        password,
        ssh_key_path,
    })
}

fn encrypt_secret(cipher: &CredentialCipher, plaintext: &str) -> Result<String, ApiError> {
    cipher.encrypt(plaintext).map_err(ApiError::from)
}

fn repo_name_map(state: &AppState) -> Result<HashMap<i64, String>, ApiError> {
    Ok(state
        .database
        .list_repos()?
        .into_iter()
        .map(|repo| (repo.id, repo.name))
        .collect())
}

fn credential_name_map(state: &AppState) -> Result<HashMap<i64, String>, ApiError> {
    Ok(state
        .database
        .list_git_credentials()?
        .into_iter()
        .map(|credential| (credential.id, credential.name))
        .collect())
}

fn task_definition_map(state: &AppState) -> Result<HashMap<i64, TaskDefinition>, ApiError> {
    Ok(state
        .database
        .list_tasks()?
        .into_iter()
        .map(|task| (task.id, task))
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

fn repo_to_response(repo: GitRepo, credential_names: &HashMap<i64, String>) -> AdminRepoResponse {
    AdminRepoResponse {
        id: repo.id,
        name: repo.name,
        repo_url: repo.repo_url,
        branch: repo.branch,
        local_path: repo.local_path,
        review_cron: repo.review_cron,
        credential_id: repo.credential_id,
        credential_name: repo
            .credential_id
            .and_then(|credential_id| credential_names.get(&credential_id).cloned()),
        last_commit: repo.last_commit,
        enabled: repo.enabled,
        created_at: repo.created_at.to_rfc3339(),
        updated_at: repo.updated_at.to_rfc3339(),
    }
}

fn credential_to_response(credential: GitCredential) -> AdminCredentialResponse {
    AdminCredentialResponse {
        id: credential.id,
        name: credential.name,
        platform: credential.platform.as_str().to_string(),
        auth_type: credential.auth_type.as_str().to_string(),
        username: credential.username,
        ssh_key_path: credential.ssh_key_path,
        has_token: credential.token.is_some(),
        has_password: credential.password.is_some(),
        created_at: credential.created_at.to_rfc3339(),
        updated_at: credential.updated_at.to_rfc3339(),
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

fn task_to_response(
    state: &AppState,
    task: TaskDefinition,
    repo_names: &HashMap<i64, String>,
) -> Result<AdminTaskResponse, ApiError> {
    let stats = state.database.task_run_stats(task.id)?;
    Ok(AdminTaskResponse {
        id: task.id,
        name: task.name,
        task_type: task.task_type.as_str().to_string(),
        repo_id: task.repo_id,
        repo_name: task
            .repo_id
            .and_then(|repo_id| repo_names.get(&repo_id).cloned()),
        prompt: task.prompt,
        cron_expr: task.cron_expr,
        status: task.status.as_str().to_string(),
        last_run_at: stats.last_run_at.map(|value| value.to_rfc3339()),
        last_run_status: stats.last_run_status.map(|value| value.as_str().to_string()),
        next_run_at: stats.next_run_at.map(|value| value.to_rfc3339()),
        total_runs: stats.total_runs,
        created_at: task.created_at.to_rfc3339(),
        updated_at: task.updated_at.to_rfc3339(),
    })
}

fn task_run_to_response(
    run: TaskRun,
    task_map: &HashMap<i64, TaskDefinition>,
    repo_names: &HashMap<i64, String>,
) -> Result<AdminTaskRunResponse, ApiError> {
    let task = task_map
        .get(&run.task_id)
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "task definition not found"))?;
    Ok(AdminTaskRunResponse {
        id: run.id,
        task_id: run.task_id,
        task_name: task.name.clone(),
        repo_name: task
            .repo_id
            .and_then(|repo_id| repo_names.get(&repo_id).cloned()),
        scheduled_at: run.scheduled_at.to_rfc3339(),
        started_at: run.started_at.map(|value| value.to_rfc3339()),
        finished_at: run.finished_at.map(|value| value.to_rfc3339()),
        status: run.status.as_str().to_string(),
        result: run.result,
        log: run.log,
        retry_count: run.retry_count,
        created_at: run.created_at.to_rfc3339(),
    })
}

fn execution_to_run_response(task: Task, repo_names: &HashMap<i64, String>) -> AdminTaskRunResponse {
    AdminTaskRunResponse {
        id: task.id,
        task_id: task.task_id,
        task_name: task.name,
        repo_name: task
            .repo_id
            .and_then(|repo_id| repo_names.get(&repo_id).cloned()),
        scheduled_at: task.scheduled_at.to_rfc3339(),
        started_at: task.started_at.map(|value| value.to_rfc3339()),
        finished_at: task.finished_at.map(|value| value.to_rfc3339()),
        status: task.status.as_str().to_string(),
        result: task.result,
        log: task.log,
        retry_count: task.retry_count,
        created_at: task.created_at.to_rfc3339(),
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
