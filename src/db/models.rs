use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use rusqlite::Row;
use serde::{Deserialize, Serialize};

use crate::db::{decode_datetime, next_run_from_cron};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskType {
    GitReview,
    TestGen,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskDefinitionStatus {
    Active,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Running,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserRole {
    #[serde(rename = "superAdmin")]
    SuperAdmin,
    #[serde(rename = "user")]
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GitPlatform {
    Github,
    Gitee,
    Gitlab,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GitAuthType {
    Token,
    Ssh,
    Basic,
}

#[derive(Debug, Clone)]
pub struct NewTask {
    pub name: String,
    pub task_type: TaskType,
    pub repo_id: Option<i64>,
    pub prompt: String,
    pub cron_expr: Option<String>,
    pub scheduled_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct UpdateTask {
    pub name: String,
    pub task_type: TaskType,
    pub repo_id: Option<i64>,
    pub prompt: String,
    pub cron_expr: Option<String>,
    pub scheduled_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TaskDefinition {
    pub id: i64,
    pub name: String,
    pub task_type: TaskType,
    pub repo_id: Option<i64>,
    pub prompt: String,
    pub cron_expr: Option<String>,
    pub status: TaskDefinitionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TaskRun {
    pub id: i64,
    pub task_id: i64,
    pub scheduled_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: TaskStatus,
    pub result: Option<String>,
    pub log: Option<String>,
    pub retry_count: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TaskRunStats {
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_run_status: Option<TaskStatus>,
    pub next_run_at: Option<DateTime<Utc>>,
    pub total_runs: i64,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub id: i64,
    pub task_id: i64,
    pub name: String,
    pub task_type: TaskType,
    pub repo_id: Option<i64>,
    pub prompt: String,
    pub cron_expr: Option<String>,
    pub scheduled_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: TaskStatus,
    pub result: Option<String>,
    pub log: Option<String>,
    pub retry_count: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewGitRepo {
    pub name: String,
    pub repo_url: String,
    pub branch: String,
    pub local_path: String,
    pub review_cron: Option<String>,
    pub credential_id: Option<i64>,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct UpdateGitRepo {
    pub name: String,
    pub repo_url: String,
    pub branch: String,
    pub local_path: String,
    pub review_cron: Option<String>,
    pub credential_id: Option<i64>,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct GitRepo {
    pub id: i64,
    pub name: String,
    pub repo_url: String,
    pub branch: String,
    pub local_path: String,
    pub review_cron: Option<String>,
    pub credential_id: Option<i64>,
    pub last_commit: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewGitCredential {
    pub name: String,
    pub platform: GitPlatform,
    pub auth_type: GitAuthType,
    pub token: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub ssh_key_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateGitCredential {
    pub name: String,
    pub platform: GitPlatform,
    pub auth_type: GitAuthType,
    pub token: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub ssh_key_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GitCredential {
    pub id: i64,
    pub name: String,
    pub platform: GitPlatform,
    pub auth_type: GitAuthType,
    pub token: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub ssh_key_path: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewUser {
    pub email: String,
    pub display_name: String,
    pub password_hash: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateUser {
    pub email: String,
    pub display_name: String,
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub display_name: String,
    pub password_hash: Option<String>,
    pub avatar_url: Option<String>,
    pub activated_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewMessage {
    pub user_id: i64,
    pub title: String,
    pub repo_name: Option<String>,
    pub content: String,
    pub summary: String,
    pub report_path: Option<String>,
    pub commit_range: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub id: i64,
    pub user_id: i64,
    pub title: String,
    pub repo_name: Option<String>,
    pub content: String,
    pub summary: String,
    pub report_path: Option<String>,
    pub commit_range: Option<String>,
    pub is_read: bool,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TaskType {
    pub fn as_str(self) -> &'static str {
        match self {
            TaskType::GitReview => "git_review",
            TaskType::TestGen => "test_gen",
            TaskType::Custom => "custom",
        }
    }

    pub fn from_cli(input: &str) -> Result<Self> {
        Self::from_db(input)
    }

    pub fn from_db(input: &str) -> Result<Self> {
        match input {
            "git_review" => Ok(Self::GitReview),
            "test_gen" => Ok(Self::TestGen),
            "custom" => Ok(Self::Custom),
            other => Err(anyhow!("unsupported task type: {other}")),
        }
    }
}

impl TaskDefinitionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TaskDefinitionStatus::Active => "active",
            TaskDefinitionStatus::Paused => "paused",
        }
    }

    pub fn from_db(input: &str) -> Result<Self> {
        match input {
            "active" => Ok(Self::Active),
            "paused" => Ok(Self::Paused),
            other => Err(anyhow!("unsupported task definition status: {other}")),
        }
    }
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::Running => "running",
            TaskStatus::Done => "done",
            TaskStatus::Failed => "failed",
            TaskStatus::Cancelled => "cancelled",
        }
    }

    pub fn from_db(input: &str) -> Result<Self> {
        match input {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "done" => Ok(Self::Done),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(anyhow!("unsupported task status: {other}")),
        }
    }
}

impl UserRole {
    pub fn as_str(self) -> &'static str {
        match self {
            UserRole::SuperAdmin => "superAdmin",
            UserRole::User => "user",
        }
    }
}

impl GitPlatform {
    pub fn as_str(self) -> &'static str {
        match self {
            GitPlatform::Github => "github",
            GitPlatform::Gitee => "gitee",
            GitPlatform::Gitlab => "gitlab",
            GitPlatform::Other => "other",
        }
    }

    pub fn from_db(input: &str) -> Result<Self> {
        match input {
            "github" => Ok(Self::Github),
            "gitee" => Ok(Self::Gitee),
            "gitlab" => Ok(Self::Gitlab),
            "other" => Ok(Self::Other),
            other => Err(anyhow!("unsupported git platform: {other}")),
        }
    }
}

impl GitAuthType {
    pub fn as_str(self) -> &'static str {
        match self {
            GitAuthType::Token => "token",
            GitAuthType::Ssh => "ssh",
            GitAuthType::Basic => "basic",
        }
    }

    pub fn from_db(input: &str) -> Result<Self> {
        match input {
            "token" => Ok(Self::Token),
            "ssh" => Ok(Self::Ssh),
            "basic" => Ok(Self::Basic),
            other => Err(anyhow!("unsupported git auth type: {other}")),
        }
    }
}

impl TaskDefinition {
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            name: row.get(1)?,
            task_type: TaskType::from_db(row.get_ref(2)?.as_str().map_err(to_sql_err)?)
                .map_err(to_sql_err)?,
            repo_id: row.get(3)?,
            prompt: row.get(4)?,
            cron_expr: row.get(5)?,
            status: TaskDefinitionStatus::from_db(row.get_ref(6)?.as_str().map_err(to_sql_err)?)
                .map_err(to_sql_err)?,
            created_at: decode_datetime(row.get_ref(7)?.as_str().map_err(to_sql_err)?)
                .map_err(to_sql_err)?,
            updated_at: decode_datetime(row.get_ref(8)?.as_str().map_err(to_sql_err)?)
                .map_err(to_sql_err)?,
        })
    }
}

impl TaskRun {
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            task_id: row.get(1)?,
            scheduled_at: decode_datetime(row.get_ref(2)?.as_str().map_err(to_sql_err)?)
                .map_err(to_sql_err)?,
            started_at: row
                .get::<_, Option<String>>(3)?
                .map(|value| decode_datetime(&value))
                .transpose()
                .map_err(to_sql_err)?,
            finished_at: row
                .get::<_, Option<String>>(4)?
                .map(|value| decode_datetime(&value))
                .transpose()
                .map_err(to_sql_err)?,
            status: TaskStatus::from_db(row.get_ref(5)?.as_str().map_err(to_sql_err)?)
                .map_err(to_sql_err)?,
            result: row.get(6)?,
            log: row.get(7)?,
            retry_count: row.get(8)?,
            created_at: decode_datetime(row.get_ref(9)?.as_str().map_err(to_sql_err)?)
                .map_err(to_sql_err)?,
        })
    }
}

impl Task {
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            task_id: row.get(1)?,
            name: row.get(2)?,
            task_type: TaskType::from_db(row.get_ref(3)?.as_str().map_err(to_sql_err)?)
                .map_err(to_sql_err)?,
            repo_id: row.get(4)?,
            prompt: row.get(5)?,
            cron_expr: row.get(6)?,
            scheduled_at: decode_datetime(row.get_ref(7)?.as_str().map_err(to_sql_err)?)
                .map_err(to_sql_err)?,
            started_at: row
                .get::<_, Option<String>>(8)?
                .map(|value| decode_datetime(&value))
                .transpose()
                .map_err(to_sql_err)?,
            finished_at: row
                .get::<_, Option<String>>(9)?
                .map(|value| decode_datetime(&value))
                .transpose()
                .map_err(to_sql_err)?,
            status: TaskStatus::from_db(row.get_ref(10)?.as_str().map_err(to_sql_err)?)
                .map_err(to_sql_err)?,
            result: row.get(11)?,
            log: row.get(12)?,
            retry_count: row.get(13)?,
            created_at: decode_datetime(row.get_ref(14)?.as_str().map_err(to_sql_err)?)
                .map_err(to_sql_err)?,
        })
    }

    pub fn next_scheduled_at(&self) -> Result<Option<DateTime<Utc>>> {
        self.cron_expr
            .as_deref()
            .map(|expr| next_run_from_cron(expr, self.scheduled_at))
            .transpose()
    }
}

impl GitRepo {
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            name: row.get(1)?,
            repo_url: row.get(2)?,
            branch: row.get(3)?,
            local_path: row.get(4)?,
            review_cron: row.get(5)?,
            credential_id: row.get(6)?,
            last_commit: row.get(7)?,
            enabled: row.get::<_, i64>(8)? != 0,
            created_at: decode_datetime(row.get_ref(9)?.as_str().map_err(to_sql_err)?)
                .map_err(to_sql_err)?,
            updated_at: decode_datetime(row.get_ref(10)?.as_str().map_err(to_sql_err)?)
                .map_err(to_sql_err)?,
        })
    }
}

impl GitCredential {
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            name: row.get(1)?,
            platform: GitPlatform::from_db(row.get_ref(2)?.as_str().map_err(to_sql_err)?)
                .map_err(to_sql_err)?,
            auth_type: GitAuthType::from_db(row.get_ref(3)?.as_str().map_err(to_sql_err)?)
                .map_err(to_sql_err)?,
            token: row.get(4)?,
            username: row.get(5)?,
            password: row.get(6)?,
            ssh_key_path: row.get(7)?,
            created_at: decode_datetime(row.get_ref(8)?.as_str().map_err(to_sql_err)?)
                .map_err(to_sql_err)?,
            updated_at: decode_datetime(row.get_ref(9)?.as_str().map_err(to_sql_err)?)
                .map_err(to_sql_err)?,
        })
    }
}

impl User {
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            email: row.get(1)?,
            display_name: row.get(2)?,
            password_hash: row.get(3)?,
            avatar_url: row.get(4)?,
            activated_at: row
                .get::<_, Option<String>>(5)?
                .map(|value| decode_datetime(&value))
                .transpose()
                .map_err(to_sql_err)?,
            created_at: decode_datetime(row.get_ref(6)?.as_str().map_err(to_sql_err)?)
                .map_err(to_sql_err)?,
            updated_at: decode_datetime(row.get_ref(7)?.as_str().map_err(to_sql_err)?)
                .map_err(to_sql_err)?,
        })
    }
}

impl Message {
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            user_id: row.get(1)?,
            title: row.get(2)?,
            repo_name: row.get(3)?,
            content: row.get(4)?,
            summary: row.get(5)?,
            report_path: row.get(6)?,
            commit_range: row.get(7)?,
            is_read: row.get::<_, i64>(8)? != 0,
            read_at: row
                .get::<_, Option<String>>(9)?
                .map(|value| decode_datetime(&value))
                .transpose()
                .map_err(to_sql_err)?,
            created_at: decode_datetime(row.get_ref(10)?.as_str().map_err(to_sql_err)?)
                .map_err(to_sql_err)?,
            updated_at: decode_datetime(row.get_ref(11)?.as_str().map_err(to_sql_err)?)
                .map_err(to_sql_err)?,
        })
    }
}

fn to_sql_err(error: impl std::fmt::Display) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            error.to_string(),
        )),
    )
}
