pub mod models;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use cron::Schedule;
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::db::models::{GitRepo, NewGitRepo, NewTask, Task, TaskStatus};

#[derive(Debug, Clone)]
pub struct Database {
    path: PathBuf,
}

impl Database {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    pub fn init(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        let conn = self.open_connection()?;
        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS git_repos (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                repo_url TEXT NOT NULL,
                branch TEXT NOT NULL DEFAULT 'main',
                local_path TEXT NOT NULL,
                review_cron TEXT,
                last_commit TEXT,
                enabled INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS tasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                task_type TEXT NOT NULL,
                repo_id INTEGER,
                prompt TEXT NOT NULL,
                cron_expr TEXT,
                scheduled_at TEXT NOT NULL,
                started_at TEXT,
                status TEXT NOT NULL DEFAULT 'pending',
                result TEXT,
                retry_count INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY(repo_id) REFERENCES git_repos(id)
            );

            CREATE INDEX IF NOT EXISTS idx_tasks_schedule ON tasks(status, scheduled_at);
            CREATE INDEX IF NOT EXISTS idx_tasks_repo_id ON tasks(repo_id);
            "#,
        )
        .context("failed to initialize schema")?;
        Ok(())
    }

    pub fn insert_repo(&self, repo: &NewGitRepo) -> Result<i64> {
        let conn = self.open_connection()?;
        conn.execute(
            r#"
            INSERT INTO git_repos (name, repo_url, branch, local_path, review_cron, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
            "#,
            params![
                repo.name,
                repo.repo_url,
                repo.branch,
                repo.local_path,
                repo.review_cron,
                now_string(),
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_repos(&self) -> Result<Vec<GitRepo>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, repo_url, branch, local_path, review_cron, last_commit, enabled, created_at, updated_at FROM git_repos ORDER BY id",
        )?;
        let rows = stmt.query_map([], GitRepo::from_row)?;
        collect_rows(rows)
    }

    pub fn get_repo(&self, repo_id: i64) -> Result<Option<GitRepo>> {
        let conn = self.open_connection()?;
        conn.query_row(
            "SELECT id, name, repo_url, branch, local_path, review_cron, last_commit, enabled, created_at, updated_at FROM git_repos WHERE id = ?1",
            [repo_id],
            GitRepo::from_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn update_repo_last_commit(&self, repo_id: i64, last_commit: Option<&str>) -> Result<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "UPDATE git_repos SET last_commit = ?1, updated_at = ?2 WHERE id = ?3",
            params![last_commit, now_string(), repo_id],
        )?;
        Ok(())
    }

    pub fn insert_task(&self, task: &NewTask) -> Result<i64> {
        let conn = self.open_connection()?;
        conn.execute(
            r#"
            INSERT INTO tasks (
                name, task_type, repo_id, prompt, cron_expr, scheduled_at, status, retry_count, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', 0, ?7, ?7)
            "#,
            params![
                task.name,
                task.task_type.as_str(),
                task.repo_id,
                task.prompt,
                task.cron_expr,
                encode_datetime(task.scheduled_at),
                now_string(),
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_tasks(&self) -> Result<Vec<Task>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, task_type, repo_id, prompt, cron_expr, scheduled_at, started_at, status, result, retry_count, created_at, updated_at FROM tasks ORDER BY id",
        )?;
        let rows = stmt.query_map([], Task::from_row)?;
        collect_rows(rows)
    }

    pub fn claim_due_tasks(&self, limit: usize) -> Result<Vec<Task>> {
        let mut conn = self.open_connection()?;
        let tx = conn.transaction()?;
        let ids = Self::claim_due_task_ids(&tx, limit)?;
        let tasks = Self::load_tasks_by_ids(&tx, &ids)?;
        tx.commit()?;
        Ok(tasks)
    }

    pub fn recover_stalled_tasks(&self, timeout: Duration) -> Result<usize> {
        let conn = self.open_connection()?;
        let threshold = Utc::now() - chrono::Duration::from_std(timeout)?;
        let count = conn.execute(
            r#"
            UPDATE tasks
            SET status = 'pending', started_at = NULL, updated_at = ?1
            WHERE status = 'running' AND started_at IS NOT NULL AND started_at < ?2
            "#,
            params![now_string(), encode_datetime(threshold)],
        )?;
        Ok(count)
    }

    pub fn finish_task(&self, task: &Task, status: TaskStatus, result: Option<&str>) -> Result<()> {
        let mut conn = self.open_connection()?;
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE tasks SET status = ?1, result = ?2, updated_at = ?3 WHERE id = ?4",
            params![status.as_str(), result, now_string(), task.id],
        )?;
        if let Some(next_run) = task.next_scheduled_at()? {
            tx.execute(
                r#"
                INSERT INTO tasks (
                    name, task_type, repo_id, prompt, cron_expr, scheduled_at, status, retry_count, created_at, updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', 0, ?7, ?7)
                "#,
                params![
                    task.name,
                    task.task_type.as_str(),
                    task.repo_id,
                    task.prompt,
                    task.cron_expr,
                    encode_datetime(next_run),
                    now_string(),
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    fn open_connection(&self) -> Result<Connection> {
        let conn = Connection::open(&self.path)
            .with_context(|| format!("failed to open {}", self.path.display()))?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        Ok(conn)
    }

    fn claim_due_task_ids(tx: &Transaction<'_>, limit: usize) -> Result<Vec<i64>> {
        let now = now_string();
        let mut stmt = tx.prepare(
            "SELECT id FROM tasks WHERE status = 'pending' AND scheduled_at <= ?1 ORDER BY scheduled_at, id LIMIT ?2",
        )?;
        let ids = stmt
            .query_map(params![now, limit as i64], |row| row.get::<_, i64>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(stmt);

        let mut claimed = Vec::new();
        for id in ids {
            let updated = tx.execute(
                r#"
                UPDATE tasks
                SET status = 'running', started_at = ?1, updated_at = ?1
                WHERE id = ?2 AND status = 'pending'
                "#,
                params![now_string(), id],
            )?;
            if updated == 1 {
                claimed.push(id);
            }
        }

        Ok(claimed)
    }

    fn load_tasks_by_ids(tx: &Transaction<'_>, ids: &[i64]) -> Result<Vec<Task>> {
        let mut tasks = Vec::new();
        for id in ids {
            let task = tx.query_row(
                "SELECT id, name, task_type, repo_id, prompt, cron_expr, scheduled_at, started_at, status, result, retry_count, created_at, updated_at FROM tasks WHERE id = ?1",
                [id],
                Task::from_row,
            )?;
            tasks.push(task);
        }
        Ok(tasks)
    }
}

fn now_string() -> String {
    encode_datetime(Utc::now())
}

pub fn encode_datetime(value: DateTime<Utc>) -> String {
    value.to_rfc3339()
}

pub fn decode_datetime(value: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(value)?.with_timezone(&Utc))
}

fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>>,
) -> Result<Vec<T>> {
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn next_run_from_cron(expr: &str, after: DateTime<Utc>) -> Result<DateTime<Utc>> {
    let schedule = expr.parse::<Schedule>()?;
    schedule
        .upcoming(Utc)
        .find(|candidate| *candidate > after)
        .context("cron expression has no future schedule")
}

#[cfg(test)]
mod tests {
    use chrono::Duration;
    use tempfile::tempdir;

    use super::*;
    use crate::db::models::{NewTask, TaskType};

    #[test]
    fn claim_due_task_transitions_to_running() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("test.db"));
        db.init().unwrap();

        db.insert_task(&NewTask {
            name: "sample".to_string(),
            task_type: TaskType::Custom,
            repo_id: None,
            prompt: "echo".to_string(),
            cron_expr: None,
            scheduled_at: Utc::now() - Duration::seconds(1),
        })
        .unwrap();

        let tasks = db.claim_due_tasks(8).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].status, TaskStatus::Running);
    }
}
