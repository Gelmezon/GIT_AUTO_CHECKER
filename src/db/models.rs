use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use rusqlite::Row;
use serde::{Deserialize, Serialize};

use crate::db::{decode_datetime, next_run_from_cron};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskType {
    GitReview,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Running,
    Done,
    Failed,
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
pub struct Task {
    pub id: i64,
    pub name: String,
    pub task_type: TaskType,
    pub repo_id: Option<i64>,
    pub prompt: String,
    pub cron_expr: Option<String>,
    pub scheduled_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub status: TaskStatus,
    pub result: Option<String>,
    pub retry_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewGitRepo {
    pub name: String,
    pub repo_url: String,
    pub branch: String,
    pub local_path: String,
    pub review_cron: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GitRepo {
    pub id: i64,
    pub name: String,
    pub repo_url: String,
    pub branch: String,
    pub local_path: String,
    pub review_cron: Option<String>,
    pub last_commit: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TaskType {
    pub fn as_str(self) -> &'static str {
        match self {
            TaskType::GitReview => "git_review",
            TaskType::Custom => "custom",
        }
    }

    pub fn from_cli(input: &str) -> Result<Self> {
        Self::from_db(input)
    }

    pub fn from_db(input: &str) -> Result<Self> {
        match input {
            "git_review" => Ok(Self::GitReview),
            "custom" => Ok(Self::Custom),
            other => Err(anyhow!("unsupported task type: {other}")),
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
        }
    }

    pub fn from_db(input: &str) -> Result<Self> {
        match input {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "done" => Ok(Self::Done),
            "failed" => Ok(Self::Failed),
            other => Err(anyhow!("unsupported task status: {other}")),
        }
    }
}

impl Task {
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            name: row.get(1)?,
            task_type: TaskType::from_db(row.get_ref(2)?.as_str().map_err(to_sql_err)?)
                .map_err(to_sql_err)?,
            repo_id: row.get(3)?,
            prompt: row.get(4)?,
            cron_expr: row.get(5)?,
            scheduled_at: decode_datetime(row.get_ref(6)?.as_str().map_err(to_sql_err)?)
                .map_err(to_sql_err)?,
            started_at: row
                .get::<_, Option<String>>(7)?
                .map(|value| decode_datetime(&value))
                .transpose()
                .map_err(to_sql_err)?,
            status: TaskStatus::from_db(row.get_ref(8)?.as_str().map_err(to_sql_err)?)
                .map_err(to_sql_err)?,
            result: row.get(9)?,
            retry_count: row.get(10)?,
            created_at: decode_datetime(row.get_ref(11)?.as_str().map_err(to_sql_err)?)
                .map_err(to_sql_err)?,
            updated_at: decode_datetime(row.get_ref(12)?.as_str().map_err(to_sql_err)?)
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
            last_commit: row.get(6)?,
            enabled: row.get::<_, i64>(7)? != 0,
            created_at: decode_datetime(row.get_ref(8)?.as_str().map_err(to_sql_err)?)
                .map_err(to_sql_err)?,
            updated_at: decode_datetime(row.get_ref(9)?.as_str().map_err(to_sql_err)?)
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
