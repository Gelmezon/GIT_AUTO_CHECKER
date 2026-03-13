pub mod models;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use cron::Schedule;
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::db::models::{
    GitCredential, GitRepo, Message, NewGitCredential, NewGitRepo, NewMessage, NewTask, NewUser,
    Task, TaskDefinition, TaskDefinitionStatus, TaskRun, TaskRunStats, TaskStatus, TaskType,
    UpdateGitCredential, UpdateGitRepo, UpdateTask, UpdateUser, User,
};

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

        let mut conn = self.open_connection()?;
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
                credential_id INTEGER,
                last_commit TEXT,
                enabled INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY(credential_id) REFERENCES git_credentials(id) ON DELETE SET NULL
            );

            CREATE TABLE IF NOT EXISTS git_credentials (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                platform TEXT NOT NULL,
                auth_type TEXT NOT NULL,
                token TEXT,
                username TEXT,
                password TEXT,
                ssh_key_path TEXT,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS task_definitions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                task_type TEXT NOT NULL,
                repo_id INTEGER,
                prompt TEXT NOT NULL,
                cron_expr TEXT,
                status TEXT NOT NULL DEFAULT 'active',
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY(repo_id) REFERENCES git_repos(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS task_runs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id INTEGER NOT NULL,
                scheduled_at TEXT NOT NULL,
                started_at TEXT,
                finished_at TEXT,
                status TEXT NOT NULL DEFAULT 'pending',
                result TEXT,
                log TEXT,
                retry_count INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY(task_id) REFERENCES task_definitions(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                email TEXT NOT NULL UNIQUE,
                display_name TEXT NOT NULL,
                password_hash TEXT,
                avatar_url TEXT,
                activated_at TEXT,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                title TEXT NOT NULL,
                repo_name TEXT,
                content TEXT NOT NULL,
                summary TEXT NOT NULL,
                report_path TEXT,
                commit_range TEXT,
                is_read INTEGER NOT NULL DEFAULT 0,
                read_at TEXT,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_task_definitions_status ON task_definitions(status);
            CREATE INDEX IF NOT EXISTS idx_task_definitions_repo_id ON task_definitions(repo_id);
            CREATE INDEX IF NOT EXISTS idx_task_runs_task_id ON task_runs(task_id);
            CREATE INDEX IF NOT EXISTS idx_task_runs_status_scheduled ON task_runs(status, scheduled_at);
            CREATE INDEX IF NOT EXISTS idx_task_runs_task_status ON task_runs(task_id, status);
            "#,
        )
        .context("failed to initialize schema")?;
        ensure_column_exists(
            &conn,
            "git_repos",
            "credential_id",
            "ALTER TABLE git_repos ADD COLUMN credential_id INTEGER",
        )?;
        conn.execute_batch(
            r#"
            CREATE INDEX IF NOT EXISTS idx_repos_credential_id ON git_repos(credential_id);
            CREATE INDEX IF NOT EXISTS idx_git_credentials_name ON git_credentials(name);
            CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
            CREATE INDEX IF NOT EXISTS idx_messages_user_created ON messages(user_id, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_messages_user_unread ON messages(user_id, is_read);
            "#,
        )
        .context("failed to initialize indexes")?;
        migrate_legacy_tasks_if_needed(&mut conn)?;
        Ok(())
    }

    pub fn insert_repo(&self, repo: &NewGitRepo) -> Result<i64> {
        let conn = self.open_connection()?;
        conn.execute(
            r#"
            INSERT INTO git_repos (name, repo_url, branch, local_path, review_cron, credential_id, enabled, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
            "#,
            params![
                repo.name,
                repo.repo_url,
                repo.branch,
                repo.local_path,
                repo.review_cron,
                repo.credential_id,
                repo.enabled as i64,
                now_string(),
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_repos(&self) -> Result<Vec<GitRepo>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, repo_url, branch, local_path, review_cron, credential_id, last_commit, enabled, created_at, updated_at FROM git_repos ORDER BY id",
        )?;
        let rows = stmt.query_map([], GitRepo::from_row)?;
        collect_rows(rows)
    }

    pub fn get_repo(&self, repo_id: i64) -> Result<Option<GitRepo>> {
        let conn = self.open_connection()?;
        conn.query_row(
            "SELECT id, name, repo_url, branch, local_path, review_cron, credential_id, last_commit, enabled, created_at, updated_at FROM git_repos WHERE id = ?1",
            [repo_id],
            GitRepo::from_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn get_repo_by_local_path(&self, local_path: &str) -> Result<Option<GitRepo>> {
        let conn = self.open_connection()?;
        conn.query_row(
            "SELECT id, name, repo_url, branch, local_path, review_cron, credential_id, last_commit, enabled, created_at, updated_at FROM git_repos WHERE local_path = ?1",
            [local_path],
            GitRepo::from_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn get_repo_by_repo_url(&self, repo_url: &str) -> Result<Option<GitRepo>> {
        let conn = self.open_connection()?;
        conn.query_row(
            "SELECT id, name, repo_url, branch, local_path, review_cron, credential_id, last_commit, enabled, created_at, updated_at FROM git_repos WHERE repo_url = ?1",
            [repo_url],
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

    pub fn update_repo(&self, repo_id: i64, repo: &UpdateGitRepo) -> Result<bool> {
        let conn = self.open_connection()?;
        let updated = conn.execute(
            r#"
            UPDATE git_repos
            SET name = ?1,
                repo_url = ?2,
                branch = ?3,
                local_path = ?4,
                review_cron = ?5,
                credential_id = ?6,
                enabled = ?7,
                updated_at = ?8
            WHERE id = ?9
            "#,
            params![
                repo.name,
                repo.repo_url,
                repo.branch,
                repo.local_path,
                repo.review_cron,
                repo.credential_id,
                repo.enabled as i64,
                now_string(),
                repo_id,
            ],
        )?;
        Ok(updated > 0)
    }

    pub fn delete_repo(&self, repo_id: i64) -> Result<bool> {
        let conn = self.open_connection()?;
        let deleted = conn.execute("DELETE FROM git_repos WHERE id = ?1", [repo_id])?;
        Ok(deleted > 0)
    }

    pub fn insert_git_credential(&self, credential: &NewGitCredential) -> Result<i64> {
        let conn = self.open_connection()?;
        conn.execute(
            r#"
            INSERT INTO git_credentials (
                name, platform, auth_type, token, username, password, ssh_key_path, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
            "#,
            params![
                credential.name,
                credential.platform.as_str(),
                credential.auth_type.as_str(),
                credential.token,
                credential.username,
                credential.password,
                credential.ssh_key_path,
                now_string(),
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_git_credentials(&self) -> Result<Vec<GitCredential>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, platform, auth_type, token, username, password, ssh_key_path, created_at, updated_at FROM git_credentials ORDER BY id",
        )?;
        let rows = stmt.query_map([], GitCredential::from_row)?;
        collect_rows(rows)
    }

    pub fn get_git_credential(&self, credential_id: i64) -> Result<Option<GitCredential>> {
        let conn = self.open_connection()?;
        conn.query_row(
            "SELECT id, name, platform, auth_type, token, username, password, ssh_key_path, created_at, updated_at FROM git_credentials WHERE id = ?1",
            [credential_id],
            GitCredential::from_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn update_git_credential(
        &self,
        credential_id: i64,
        credential: &UpdateGitCredential,
    ) -> Result<bool> {
        let conn = self.open_connection()?;
        let updated = conn.execute(
            r#"
            UPDATE git_credentials
            SET name = ?1,
                platform = ?2,
                auth_type = ?3,
                token = ?4,
                username = ?5,
                password = ?6,
                ssh_key_path = ?7,
                updated_at = ?8
            WHERE id = ?9
            "#,
            params![
                credential.name,
                credential.platform.as_str(),
                credential.auth_type.as_str(),
                credential.token,
                credential.username,
                credential.password,
                credential.ssh_key_path,
                now_string(),
                credential_id,
            ],
        )?;
        Ok(updated > 0)
    }

    pub fn delete_git_credential(&self, credential_id: i64) -> Result<bool> {
        let mut conn = self.open_connection()?;
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE git_repos SET credential_id = NULL, updated_at = ?1 WHERE credential_id = ?2",
            params![now_string(), credential_id],
        )?;
        let deleted = tx.execute("DELETE FROM git_credentials WHERE id = ?1", [credential_id])?;
        tx.commit()?;
        Ok(deleted > 0)
    }

    pub fn insert_task(&self, task: &NewTask) -> Result<i64> {
        let mut conn = self.open_connection()?;
        let tx = conn.transaction()?;
        let task_id = Self::insert_task_definition_tx(&tx, task, TaskDefinitionStatus::Active)?;
        Self::insert_task_run_tx(
            &tx,
            task_id,
            task.scheduled_at,
            TaskStatus::Pending,
            None,
            None,
            0,
            Some(now_string()),
        )?;
        tx.commit()?;
        Ok(task_id)
    }

    pub fn list_tasks(&self) -> Result<Vec<TaskDefinition>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, task_type, repo_id, prompt, cron_expr, status, created_at, updated_at FROM task_definitions ORDER BY updated_at DESC, id DESC",
        )?;
        let rows = stmt.query_map([], TaskDefinition::from_row)?;
        collect_rows(rows)
    }

    pub fn get_task(&self, task_id: i64) -> Result<Option<TaskDefinition>> {
        let conn = self.open_connection()?;
        conn.query_row(
            "SELECT id, name, task_type, repo_id, prompt, cron_expr, status, created_at, updated_at FROM task_definitions WHERE id = ?1",
            [task_id],
            TaskDefinition::from_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn list_tasks_filtered(
        &self,
        status: Option<TaskDefinitionStatus>,
        task_type: Option<TaskType>,
        page: usize,
        page_size: usize,
    ) -> Result<(Vec<TaskDefinition>, i64)> {
        let conn = self.open_connection()?;
        let offset = (page.saturating_sub(1) * page_size) as i64;

        let (total, query) = match (status, task_type) {
            (Some(status), Some(task_type)) => {
                let total = conn.query_row(
                    "SELECT COUNT(*) FROM task_definitions WHERE status = ?1 AND task_type = ?2",
                    params![status.as_str(), task_type.as_str()],
                    |row| row.get(0),
                )?;
                let query = conn.prepare(
                    "SELECT id, name, task_type, repo_id, prompt, cron_expr, status, created_at, updated_at FROM task_definitions WHERE status = ?1 AND task_type = ?2 ORDER BY updated_at DESC, id DESC LIMIT ?3 OFFSET ?4",
                )?;
                (total, (query, Some(status.as_str().to_string()), Some(task_type.as_str().to_string())))
            }
            (Some(status), None) => {
                let total = conn.query_row(
                    "SELECT COUNT(*) FROM task_definitions WHERE status = ?1",
                    [status.as_str()],
                    |row| row.get(0),
                )?;
                let query = conn.prepare(
                    "SELECT id, name, task_type, repo_id, prompt, cron_expr, status, created_at, updated_at FROM task_definitions WHERE status = ?1 ORDER BY updated_at DESC, id DESC LIMIT ?2 OFFSET ?3",
                )?;
                (total, (query, Some(status.as_str().to_string()), None))
            }
            (None, Some(task_type)) => {
                let total = conn.query_row(
                    "SELECT COUNT(*) FROM task_definitions WHERE task_type = ?1",
                    [task_type.as_str()],
                    |row| row.get(0),
                )?;
                let query = conn.prepare(
                    "SELECT id, name, task_type, repo_id, prompt, cron_expr, status, created_at, updated_at FROM task_definitions WHERE task_type = ?1 ORDER BY updated_at DESC, id DESC LIMIT ?2 OFFSET ?3",
                )?;
                (total, (query, None, Some(task_type.as_str().to_string())))
            }
            (None, None) => {
                let total =
                    conn.query_row("SELECT COUNT(*) FROM task_definitions", [], |row| row.get(0))?;
                let query = conn.prepare(
                    "SELECT id, name, task_type, repo_id, prompt, cron_expr, status, created_at, updated_at FROM task_definitions ORDER BY updated_at DESC, id DESC LIMIT ?1 OFFSET ?2",
                )?;
                (total, (query, None, None))
            }
        };

        let (mut stmt, status_value, task_type_value) = query;
        let rows = match (status_value, task_type_value) {
            (Some(status), Some(task_type)) => stmt.query_map(
                params![status, task_type, page_size as i64, offset],
                TaskDefinition::from_row,
            )?,
            (Some(status), None) => stmt.query_map(
                params![status, page_size as i64, offset],
                TaskDefinition::from_row,
            )?,
            (None, Some(task_type)) => stmt.query_map(
                params![task_type, page_size as i64, offset],
                TaskDefinition::from_row,
            )?,
            (None, None) => stmt.query_map(params![page_size as i64, offset], TaskDefinition::from_row)?,
        };

        Ok((collect_rows(rows)?, total))
    }

    pub fn update_task(&self, task_id: i64, task: &UpdateTask) -> Result<bool> {
        let mut conn = self.open_connection()?;
        let tx = conn.transaction()?;
        let definition = match Self::load_task_definition_tx(&tx, task_id)? {
            Some(definition) => definition,
            None => return Ok(false),
        };

        tx.execute(
            r#"
            UPDATE task_definitions
            SET name = ?1,
                task_type = ?2,
                repo_id = ?3,
                prompt = ?4,
                cron_expr = ?5,
                updated_at = ?6
            WHERE id = ?7
            "#,
            params![
                task.name,
                task.task_type.as_str(),
                task.repo_id,
                task.prompt,
                task.cron_expr,
                now_string(),
                task_id,
            ],
        )?;

        Self::cancel_pending_runs_tx(&tx, task_id, "task definition updated")?;
        if definition.status == TaskDefinitionStatus::Active && !Self::has_open_runs_tx(&tx, task_id)? {
            Self::insert_task_run_tx(
                &tx,
                task_id,
                task.scheduled_at,
                TaskStatus::Pending,
                None,
                None,
                0,
                Some(now_string()),
            )?;
        }

        tx.commit()?;
        Ok(true)
    }

    pub fn delete_task(&self, task_id: i64) -> Result<bool> {
        let conn = self.open_connection()?;
        let deleted = conn.execute("DELETE FROM task_definitions WHERE id = ?1", [task_id])?;
        Ok(deleted > 0)
    }

    pub fn pause_task(&self, task_id: i64) -> Result<bool> {
        let mut conn = self.open_connection()?;
        let tx = conn.transaction()?;
        let updated = tx.execute(
            "UPDATE task_definitions SET status = 'paused', updated_at = ?1 WHERE id = ?2",
            params![now_string(), task_id],
        )?;
        if updated == 0 {
            return Ok(false);
        }
        Self::cancel_pending_runs_tx(&tx, task_id, "task paused")?;
        tx.commit()?;
        Ok(true)
    }

    pub fn resume_task(&self, task_id: i64) -> Result<bool> {
        let mut conn = self.open_connection()?;
        let tx = conn.transaction()?;
        let definition = match Self::load_task_definition_tx(&tx, task_id)? {
            Some(definition) => definition,
            None => return Ok(false),
        };

        tx.execute(
            "UPDATE task_definitions SET status = 'active', updated_at = ?1 WHERE id = ?2",
            params![now_string(), task_id],
        )?;

        if !Self::has_open_runs_tx(&tx, task_id)? {
            let scheduled_at = scheduled_at_for_definition(&definition, Utc::now())?;
            Self::insert_task_run_tx(
                &tx,
                task_id,
                scheduled_at,
                TaskStatus::Pending,
                None,
                None,
                0,
                Some(now_string()),
            )?;
        }

        tx.commit()?;
        Ok(true)
    }

    pub fn trigger_task(&self, task_id: i64) -> Result<Option<i64>> {
        let mut conn = self.open_connection()?;
        let tx = conn.transaction()?;
        if Self::load_task_definition_tx(&tx, task_id)?.is_none() {
            return Ok(None);
        }
        let run_id = Self::insert_task_run_tx(
            &tx,
            task_id,
            Utc::now(),
            TaskStatus::Pending,
            None,
            None,
            0,
            Some(now_string()),
        )?;
        tx.commit()?;
        Ok(Some(run_id))
    }

    pub fn task_run_stats(&self, task_id: i64) -> Result<TaskRunStats> {
        let conn = self.open_connection()?;
        let last_run = conn
            .query_row(
                r#"
                SELECT COALESCE(finished_at, started_at, scheduled_at), status
                FROM task_runs
                WHERE task_id = ?1 AND status != 'pending'
                ORDER BY COALESCE(finished_at, started_at, scheduled_at) DESC, id DESC
                LIMIT 1
                "#,
                [task_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let next_run_at = conn
            .query_row(
                "SELECT scheduled_at FROM task_runs WHERE task_id = ?1 AND status = 'pending' ORDER BY scheduled_at, id LIMIT 1",
                [task_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|value| decode_datetime(&value))
            .transpose()?;
        let total_runs = conn.query_row(
            "SELECT COUNT(*) FROM task_runs WHERE task_id = ?1",
            [task_id],
            |row| row.get(0),
        )?;

        Ok(TaskRunStats {
            last_run_at: last_run
                .as_ref()
                .map(|(value, _)| decode_datetime(value))
                .transpose()?,
            last_run_status: last_run
                .map(|(_, status)| TaskStatus::from_db(&status))
                .transpose()?,
            next_run_at,
            total_runs,
        })
    }

    pub fn list_task_runs(&self, task_id: i64, page: usize, page_size: usize) -> Result<(Vec<TaskRun>, i64)> {
        let conn = self.open_connection()?;
        let total = conn.query_row(
            "SELECT COUNT(*) FROM task_runs WHERE task_id = ?1",
            [task_id],
            |row| row.get(0),
        )?;
        let offset = (page.saturating_sub(1) * page_size) as i64;
        let mut stmt = conn.prepare(
            "SELECT id, task_id, scheduled_at, started_at, finished_at, status, result, log, retry_count, created_at FROM task_runs WHERE task_id = ?1 ORDER BY scheduled_at DESC, id DESC LIMIT ?2 OFFSET ?3",
        )?;
        let rows = stmt.query_map(params![task_id, page_size as i64, offset], TaskRun::from_row)?;
        Ok((collect_rows(rows)?, total))
    }

    pub fn list_all_task_runs(
        &self,
        status: Option<TaskStatus>,
        task_id: Option<i64>,
        page: usize,
        page_size: usize,
    ) -> Result<(Vec<TaskRun>, i64)> {
        let conn = self.open_connection()?;
        let offset = (page.saturating_sub(1) * page_size) as i64;
        match (status, task_id) {
            (Some(status), Some(task_id)) => {
                let total = conn.query_row(
                    "SELECT COUNT(*) FROM task_runs WHERE status = ?1 AND task_id = ?2",
                    params![status.as_str(), task_id],
                    |row| row.get(0),
                )?;
                let mut stmt = conn.prepare(
                    "SELECT id, task_id, scheduled_at, started_at, finished_at, status, result, log, retry_count, created_at FROM task_runs WHERE status = ?1 AND task_id = ?2 ORDER BY scheduled_at DESC, id DESC LIMIT ?3 OFFSET ?4",
                )?;
                let rows = stmt.query_map(
                    params![status.as_str(), task_id, page_size as i64, offset],
                    TaskRun::from_row,
                )?;
                Ok((collect_rows(rows)?, total))
            }
            (Some(status), None) => {
                let total = conn.query_row(
                    "SELECT COUNT(*) FROM task_runs WHERE status = ?1",
                    [status.as_str()],
                    |row| row.get(0),
                )?;
                let mut stmt = conn.prepare(
                    "SELECT id, task_id, scheduled_at, started_at, finished_at, status, result, log, retry_count, created_at FROM task_runs WHERE status = ?1 ORDER BY scheduled_at DESC, id DESC LIMIT ?2 OFFSET ?3",
                )?;
                let rows = stmt.query_map(
                    params![status.as_str(), page_size as i64, offset],
                    TaskRun::from_row,
                )?;
                Ok((collect_rows(rows)?, total))
            }
            (None, Some(task_id)) => {
                let total = conn.query_row(
                    "SELECT COUNT(*) FROM task_runs WHERE task_id = ?1",
                    [task_id],
                    |row| row.get(0),
                )?;
                let mut stmt = conn.prepare(
                    "SELECT id, task_id, scheduled_at, started_at, finished_at, status, result, log, retry_count, created_at FROM task_runs WHERE task_id = ?1 ORDER BY scheduled_at DESC, id DESC LIMIT ?2 OFFSET ?3",
                )?;
                let rows = stmt.query_map(
                    params![task_id, page_size as i64, offset],
                    TaskRun::from_row,
                )?;
                Ok((collect_rows(rows)?, total))
            }
            (None, None) => {
                let total =
                    conn.query_row("SELECT COUNT(*) FROM task_runs", [], |row| row.get(0))?;
                let mut stmt = conn.prepare(
                    "SELECT id, task_id, scheduled_at, started_at, finished_at, status, result, log, retry_count, created_at FROM task_runs ORDER BY scheduled_at DESC, id DESC LIMIT ?1 OFFSET ?2",
                )?;
                let rows =
                    stmt.query_map(params![page_size as i64, offset], TaskRun::from_row)?;
                Ok((collect_rows(rows)?, total))
            }
        }
    }

    pub fn get_task_run(&self, run_id: i64) -> Result<Option<TaskRun>> {
        let conn = self.open_connection()?;
        conn.query_row(
            "SELECT id, task_id, scheduled_at, started_at, finished_at, status, result, log, retry_count, created_at FROM task_runs WHERE id = ?1",
            [run_id],
            TaskRun::from_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn cancel_task_run(&self, run_id: i64) -> Result<bool> {
        let conn = self.open_connection()?;
        let updated = conn.execute(
            "UPDATE task_runs SET status = 'cancelled', finished_at = ?1, result = COALESCE(result, 'task run cancelled'), log = COALESCE(log, 'task run cancelled') WHERE id = ?2 AND status = 'pending'",
            params![now_string(), run_id],
        )?;
        Ok(updated > 0)
    }

    pub fn insert_user(&self, user: &NewUser) -> Result<i64> {
        let conn = self.open_connection()?;
        conn.execute(
            r#"
            INSERT INTO users (email, display_name, password_hash, avatar_url, activated_at, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
            "#,
            params![
                user.email,
                user.display_name,
                user.password_hash,
                user.avatar_url,
                user.password_hash.as_ref().map(|_| now_string()),
                now_string(),
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_users(&self) -> Result<Vec<User>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, email, display_name, password_hash, avatar_url, activated_at, created_at, updated_at FROM users ORDER BY id",
        )?;
        let rows = stmt.query_map([], User::from_row)?;
        collect_rows(rows)
    }

    pub fn update_user(&self, user_id: i64, user: &UpdateUser) -> Result<bool> {
        let conn = self.open_connection()?;
        let updated = conn.execute(
            "UPDATE users SET email = ?1, display_name = ?2, updated_at = ?3 WHERE id = ?4",
            params![user.email, user.display_name, now_string(), user_id],
        )?;
        Ok(updated > 0)
    }

    pub fn delete_user(&self, user_id: i64) -> Result<bool> {
        let conn = self.open_connection()?;
        let deleted = conn.execute("DELETE FROM users WHERE id = ?1", [user_id])?;
        Ok(deleted > 0)
    }

    pub fn get_user(&self, user_id: i64) -> Result<Option<User>> {
        let conn = self.open_connection()?;
        conn.query_row(
            "SELECT id, email, display_name, password_hash, avatar_url, activated_at, created_at, updated_at FROM users WHERE id = ?1",
            [user_id],
            User::from_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn get_user_by_email(&self, email: &str) -> Result<Option<User>> {
        let conn = self.open_connection()?;
        conn.query_row(
            "SELECT id, email, display_name, password_hash, avatar_url, activated_at, created_at, updated_at FROM users WHERE email = ?1",
            [email],
            User::from_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn activate_user(&self, email: &str, password_hash: &str) -> Result<Option<User>> {
        let conn = self.open_connection()?;
        let updated = conn.execute(
            "UPDATE users SET password_hash = ?1, activated_at = ?2, updated_at = ?2 WHERE email = ?3 AND password_hash IS NULL",
            params![password_hash, now_string(), email],
        )?;
        if updated == 0 {
            return self.get_user_by_email(email);
        }
        self.get_user_by_email(email)
    }

    pub fn insert_message(&self, message: &NewMessage) -> Result<i64> {
        let conn = self.open_connection()?;
        conn.execute(
            r#"
            INSERT INTO messages (
                user_id, title, repo_name, content, summary, report_path, commit_range, is_read, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?8)
            "#,
            params![
                message.user_id,
                message.title,
                message.repo_name,
                message.content,
                message.summary,
                message.report_path,
                message.commit_range,
                now_string(),
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_messages(
        &self,
        user_id: i64,
        unread_only: bool,
        page: usize,
        page_size: usize,
    ) -> Result<(Vec<Message>, i64, i64)> {
        let conn = self.open_connection()?;
        let filter_sql = if unread_only { " AND is_read = 0" } else { "" };
        let total_sql = format!("SELECT COUNT(*) FROM messages WHERE user_id = ?1{filter_sql}");
        let total: i64 = conn.query_row(&total_sql, [user_id], |row| row.get(0))?;
        let unread_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE user_id = ?1 AND is_read = 0",
            [user_id],
            |row| row.get(0),
        )?;

        let offset = (page.saturating_sub(1) * page_size) as i64;
        let query = format!(
            "SELECT id, user_id, title, repo_name, content, summary, report_path, commit_range, is_read, read_at, created_at, updated_at FROM messages WHERE user_id = ?1{filter_sql} ORDER BY created_at DESC, id DESC LIMIT ?2 OFFSET ?3"
        );
        let mut stmt = conn.prepare(&query)?;
        let rows = stmt.query_map(
            params![user_id, page_size as i64, offset],
            Message::from_row,
        )?;
        Ok((collect_rows(rows)?, total, unread_count))
    }

    pub fn get_message(&self, user_id: i64, message_id: i64) -> Result<Option<Message>> {
        let conn = self.open_connection()?;
        conn.query_row(
            "SELECT id, user_id, title, repo_name, content, summary, report_path, commit_range, is_read, read_at, created_at, updated_at FROM messages WHERE user_id = ?1 AND id = ?2",
            params![user_id, message_id],
            Message::from_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn mark_message_read(&self, user_id: i64, message_id: i64) -> Result<bool> {
        let conn = self.open_connection()?;
        let updated = conn.execute(
            "UPDATE messages SET is_read = 1, read_at = COALESCE(read_at, ?1), updated_at = ?1 WHERE user_id = ?2 AND id = ?3",
            params![now_string(), user_id, message_id],
        )?;
        Ok(updated > 0)
    }

    pub fn mark_all_messages_read(&self, user_id: i64) -> Result<usize> {
        let conn = self.open_connection()?;
        let updated = conn.execute(
            "UPDATE messages SET is_read = 1, read_at = COALESCE(read_at, ?1), updated_at = ?1 WHERE user_id = ?2 AND is_read = 0",
            params![now_string(), user_id],
        )?;
        Ok(updated)
    }

    pub fn unread_message_count(&self, user_id: i64) -> Result<i64> {
        let conn = self.open_connection()?;
        conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE user_id = ?1 AND is_read = 0",
            [user_id],
            |row| row.get(0),
        )
        .map_err(Into::into)
    }

    pub fn repo_count(&self) -> Result<i64> {
        let conn = self.open_connection()?;
        conn.query_row("SELECT COUNT(*) FROM git_repos", [], |row| row.get(0))
            .map_err(Into::into)
    }

    pub fn task_count(&self) -> Result<i64> {
        let conn = self.open_connection()?;
        conn.query_row("SELECT COUNT(*) FROM task_definitions", [], |row| row.get(0))
            .map_err(Into::into)
    }

    pub fn user_count(&self) -> Result<i64> {
        let conn = self.open_connection()?;
        conn.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))
            .map_err(Into::into)
    }

    pub fn today_executed_task_count(&self) -> Result<i64> {
        let conn = self.open_connection()?;
        let today_start = Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .context("failed to build start of day")?;
        conn.query_row(
            "SELECT COUNT(*) FROM task_runs WHERE status IN ('done', 'failed') AND finished_at IS NOT NULL AND finished_at >= ?1",
            [encode_datetime(today_start.and_utc())],
            |row| row.get(0),
        )
        .map_err(Into::into)
    }

    pub fn recent_task_runs(&self, limit: usize) -> Result<Vec<Task>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT
                r.id,
                r.task_id,
                d.name,
                d.task_type,
                d.repo_id,
                d.prompt,
                d.cron_expr,
                r.scheduled_at,
                r.started_at,
                r.finished_at,
                r.status,
                r.result,
                r.log,
                r.retry_count,
                r.created_at
            FROM task_runs r
            INNER JOIN task_definitions d ON d.id = r.task_id
            WHERE r.status != 'pending'
            ORDER BY COALESCE(r.finished_at, r.started_at, r.created_at) DESC, r.id DESC
            LIMIT ?1
            "#,
        )?;
        let rows = stmt.query_map([limit as i64], Task::from_row)?;
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
            UPDATE task_runs
            SET status = 'failed',
                finished_at = ?1,
                result = COALESCE(result, 'execution timeout on recovery'),
                log = CASE
                    WHEN log IS NULL OR log = '' THEN 'execution timeout on recovery'
                    ELSE log || CHAR(10) || 'execution timeout on recovery'
                END
            WHERE status = 'running' AND started_at IS NOT NULL AND started_at < ?2
            "#,
            params![now_string(), encode_datetime(threshold)],
        )?;
        Ok(count)
    }

    pub fn finish_task(
        &self,
        task: &Task,
        status: TaskStatus,
        result: Option<&str>,
        log: Option<&str>,
    ) -> Result<()> {
        let mut conn = self.open_connection()?;
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE task_runs SET status = ?1, result = ?2, log = ?3, finished_at = ?4 WHERE id = ?5",
            params![status.as_str(), result, log, now_string(), task.id],
        )?;
        let definition = Self::load_task_definition_tx(&tx, task.task_id)?
            .context("task definition not found while finishing run")?;
        if definition.status == TaskDefinitionStatus::Active {
            if let Some(expr) = definition.cron_expr.as_deref() {
                let next_run = next_run_from_cron(expr, task.scheduled_at)?;
                if !Self::scheduled_run_exists_tx(&tx, definition.id, next_run)? {
                    Self::insert_task_run_tx(
                        &tx,
                        definition.id,
                        next_run,
                        TaskStatus::Pending,
                        None,
                        None,
                        0,
                        Some(now_string()),
                    )?;
                }
            } else {
                tx.execute(
                    "UPDATE task_definitions SET status = 'paused', updated_at = ?1 WHERE id = ?2",
                    params![now_string(), definition.id],
                )?;
            }
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
            "SELECT id FROM task_runs WHERE status = 'pending' AND scheduled_at <= ?1 ORDER BY scheduled_at, id LIMIT ?2",
        )?;
        let ids = stmt
            .query_map(params![now, limit as i64], |row| row.get::<_, i64>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(stmt);

        let mut claimed = Vec::new();
        for id in ids {
            let updated = tx.execute(
                r#"
                UPDATE task_runs
                SET status = 'running', started_at = ?1
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
                r#"
                SELECT
                    r.id,
                    r.task_id,
                    d.name,
                    d.task_type,
                    d.repo_id,
                    d.prompt,
                    d.cron_expr,
                    r.scheduled_at,
                    r.started_at,
                    r.finished_at,
                    r.status,
                    r.result,
                    r.log,
                    r.retry_count,
                    r.created_at
                FROM task_runs r
                INNER JOIN task_definitions d ON d.id = r.task_id
                WHERE r.id = ?1
                "#,
                [id],
                Task::from_row,
            )?;
            tasks.push(task);
        }
        Ok(tasks)
    }

    fn load_task_definition_tx(
        tx: &Transaction<'_>,
        task_id: i64,
    ) -> Result<Option<TaskDefinition>> {
        tx.query_row(
            "SELECT id, name, task_type, repo_id, prompt, cron_expr, status, created_at, updated_at FROM task_definitions WHERE id = ?1",
            [task_id],
            TaskDefinition::from_row,
        )
        .optional()
        .map_err(Into::into)
    }

    fn insert_task_definition_tx(
        tx: &Transaction<'_>,
        task: &NewTask,
        status: TaskDefinitionStatus,
    ) -> Result<i64> {
        let now = now_string();
        tx.execute(
            r#"
            INSERT INTO task_definitions (
                name, task_type, repo_id, prompt, cron_expr, status, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
            "#,
            params![
                task.name,
                task.task_type.as_str(),
                task.repo_id,
                task.prompt,
                task.cron_expr,
                status.as_str(),
                now,
            ],
        )?;
        Ok(tx.last_insert_rowid())
    }

    fn insert_task_run_tx(
        tx: &Transaction<'_>,
        task_id: i64,
        scheduled_at: DateTime<Utc>,
        status: TaskStatus,
        result: Option<&str>,
        log: Option<&str>,
        retry_count: i64,
        created_at: Option<String>,
    ) -> Result<i64> {
        let created_at = created_at.unwrap_or_else(now_string);
        let started_at = matches!(status, TaskStatus::Running).then(|| encode_datetime(Utc::now()));
        let finished_at = matches!(
            status,
            TaskStatus::Done | TaskStatus::Failed | TaskStatus::Cancelled
        )
        .then(|| now_string());

        tx.execute(
            r#"
            INSERT INTO task_runs (
                task_id, scheduled_at, started_at, finished_at, status, result, log, retry_count, created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            params![
                task_id,
                encode_datetime(scheduled_at),
                started_at,
                finished_at,
                status.as_str(),
                result,
                log,
                retry_count,
                created_at,
            ],
        )?;
        Ok(tx.last_insert_rowid())
    }

    fn has_open_runs_tx(tx: &Transaction<'_>, task_id: i64) -> Result<bool> {
        let count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM task_runs WHERE task_id = ?1 AND status IN ('pending', 'running')",
            [task_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn cancel_pending_runs_tx(tx: &Transaction<'_>, task_id: i64, reason: &str) -> Result<usize> {
        tx.execute(
            "UPDATE task_runs SET status = 'cancelled', finished_at = ?1, result = COALESCE(result, ?2), log = COALESCE(log, ?2) WHERE task_id = ?3 AND status = 'pending'",
            params![now_string(), reason, task_id],
        )
        .map_err(Into::into)
    }

    fn scheduled_run_exists_tx(
        tx: &Transaction<'_>,
        task_id: i64,
        scheduled_at: DateTime<Utc>,
    ) -> Result<bool> {
        let count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM task_runs WHERE task_id = ?1 AND scheduled_at = ?2",
            params![task_id, encode_datetime(scheduled_at)],
            |row| row.get(0),
        )?;
        Ok(count > 0)
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

fn sql_data_error(error: impl std::fmt::Display) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            error.to_string(),
        )),
    )
}

fn ensure_column_exists(
    conn: &Connection,
    table: &str,
    column: &str,
    alter_sql: &str,
) -> Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    let columns = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    if !columns.iter().any(|existing| existing == column) {
        conn.execute(alter_sql, [])
            .with_context(|| format!("failed to add {column} to {table}"))?;
    }
    Ok(())
}

fn scheduled_at_for_definition(
    definition: &TaskDefinition,
    after: DateTime<Utc>,
) -> Result<DateTime<Utc>> {
    match definition.cron_expr.as_deref() {
        Some(expr) => next_run_from_cron(expr, after),
        None => Ok(after),
    }
}

fn migrate_legacy_tasks_if_needed(conn: &mut Connection) -> Result<()> {
    if !table_exists(conn, "tasks")? {
        return Ok(());
    }

    let definition_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM task_definitions", [], |row| row.get(0))?;
    if definition_count > 0 {
        return Ok(());
    }

    let legacy_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get(0))
        .unwrap_or(0);
    if legacy_count == 0 {
        return Ok(());
    }

    #[derive(Debug)]
    struct LegacyTaskRow {
        name: String,
        task_type: TaskType,
        repo_id: Option<i64>,
        prompt: String,
        cron_expr: Option<String>,
        scheduled_at: DateTime<Utc>,
        started_at: Option<DateTime<Utc>>,
        status: TaskStatus,
        result: Option<String>,
        retry_count: i64,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    }

    let tx = conn.transaction()?;
    let mut stmt = tx.prepare(
        "SELECT name, task_type, repo_id, prompt, cron_expr, scheduled_at, started_at, status, result, retry_count, created_at, updated_at FROM tasks ORDER BY id",
    )?;
    let rows = stmt.query_map([], |row| {
        let task_type = TaskType::from_db(&row.get::<_, String>(1)?).map_err(sql_data_error)?;
        let status = TaskStatus::from_db(&row.get::<_, String>(7)?).map_err(sql_data_error)?;
        Ok(LegacyTaskRow {
            name: row.get(0)?,
            task_type,
            repo_id: row.get(2)?,
            prompt: row.get(3)?,
            cron_expr: row.get(4)?,
            scheduled_at: decode_datetime(&row.get::<_, String>(5)?).map_err(sql_data_error)?,
            started_at: row
                .get::<_, Option<String>>(6)?
                .map(|value| decode_datetime(&value))
                .transpose()
                .map_err(sql_data_error)?,
            status,
            result: row.get(8)?,
            retry_count: row.get(9)?,
            created_at: decode_datetime(&row.get::<_, String>(10)?).map_err(sql_data_error)?,
            updated_at: decode_datetime(&row.get::<_, String>(11)?).map_err(sql_data_error)?,
        })
    })?;
    let legacy_rows = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);

    let mut definition_map: HashMap<String, (i64, DateTime<Utc>)> = HashMap::new();
    for row in legacy_rows {
        let cron_expr = row.cron_expr.clone();
        let created_at = row.created_at;
        let updated_at = row.updated_at;
        let key = format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
            row.name,
            row.task_type.as_str(),
            row.repo_id
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            row.prompt,
            row.cron_expr.clone().unwrap_or_default(),
        );

        let (task_id, last_updated) = if let Some((task_id, last_updated)) = definition_map.get(&key) {
            (*task_id, *last_updated)
        } else {
            let initial_status = if row.cron_expr.is_some()
                || matches!(row.status, TaskStatus::Pending | TaskStatus::Running)
            {
                TaskDefinitionStatus::Active
            } else {
                TaskDefinitionStatus::Paused
            };
            tx.execute(
                r#"
                INSERT INTO task_definitions (
                    name, task_type, repo_id, prompt, cron_expr, status, created_at, updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                "#,
                params![
                    row.name,
                    row.task_type.as_str(),
                    row.repo_id,
                    row.prompt,
                    cron_expr.clone(),
                    initial_status.as_str(),
                    encode_datetime(created_at),
                    encode_datetime(updated_at),
                ],
            )?;
            let task_id = tx.last_insert_rowid();
            definition_map.insert(key.clone(), (task_id, updated_at));
            (task_id, updated_at)
        };

        if updated_at > last_updated {
            tx.execute(
                "UPDATE task_definitions SET updated_at = ?1 WHERE id = ?2",
                params![encode_datetime(updated_at), task_id],
            )?;
            if let Some(entry) = definition_map.get_mut(&key) {
                entry.1 = updated_at;
            }
        }

        if cron_expr.is_some() || matches!(row.status, TaskStatus::Pending | TaskStatus::Running) {
            tx.execute(
                "UPDATE task_definitions SET status = 'active', updated_at = ?1 WHERE id = ?2",
                params![encode_datetime(updated_at), task_id],
            )?;
        }

        let finished_at = matches!(row.status, TaskStatus::Done | TaskStatus::Failed | TaskStatus::Cancelled)
            .then(|| encode_datetime(updated_at));
        tx.execute(
            r#"
            INSERT INTO task_runs (
                task_id, scheduled_at, started_at, finished_at, status, result, log, retry_count, created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8)
            "#,
            params![
                task_id,
                encode_datetime(row.scheduled_at),
                row.started_at.map(encode_datetime),
                finished_at,
                row.status.as_str(),
                row.result,
                row.retry_count,
                encode_datetime(created_at),
            ],
        )?;
    }

    tx.commit()?;
    Ok(())
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    let exists = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1 LIMIT 1",
            [table],
            |_row| Ok(()),
        )
        .optional()?
        .is_some();
    Ok(exists)
}

pub fn next_run_from_cron(expr: &str, after: DateTime<Utc>) -> Result<DateTime<Utc>> {
    let schedule = normalize_cron_expression(expr).parse::<Schedule>()?;
    schedule
        .upcoming(Utc)
        .find(|candidate| *candidate > after)
        .context("cron expression has no future schedule")
}

fn normalize_cron_expression(expr: &str) -> String {
    let parts = expr.split_whitespace().collect::<Vec<_>>();
    match parts.len() {
        // Support standard 5-field cron expressions from the UI/README by
        // prepending a zero seconds field and remapping weekday numbers
        // from standard cron semantics (0/7=Sun, 1=Mon) to this parser.
        5 => format!(
            "0 {} {} {} {} {}",
            parts[0],
            parts[1],
            parts[2],
            parts[3],
            normalize_standard_day_of_week(parts[4]),
        ),
        _ => expr.trim().to_string(),
    }
}

fn normalize_standard_day_of_week(field: &str) -> String {
    field
        .split(',')
        .map(normalize_standard_day_of_week_segment)
        .collect::<Vec<_>>()
        .join(",")
}

fn normalize_standard_day_of_week_segment(segment: &str) -> String {
    if segment == "*" || segment.contains(char::is_alphabetic) {
        return segment.to_string();
    }

    let (base, step) = segment
        .split_once('/')
        .map_or((segment, None), |(base, step)| (base, Some(step)));

    let normalized_base = if let Some((start, end)) = base.split_once('-') {
        format!(
            "{}-{}",
            standard_weekday_token(start),
            standard_weekday_token(end)
        )
    } else {
        standard_weekday_token(base)
    };

    match step {
        Some(step) => format!("{normalized_base}/{step}"),
        None => normalized_base,
    }
}

fn standard_weekday_token(token: &str) -> String {
    match token.trim() {
        "0" | "7" => "Sun".to_string(),
        "1" => "Mon".to_string(),
        "2" => "Tue".to_string(),
        "3" => "Wed".to_string(),
        "4" => "Thu".to_string(),
        "5" => "Fri".to_string(),
        "6" => "Sat".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone};
    use tempfile::tempdir;

    use super::*;
    use crate::db::models::{NewMessage, NewTask, NewUser, TaskType};

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

    #[test]
    fn user_activation_and_message_flow_work() {
        let dir = tempdir().unwrap();
        let db = Database::new(dir.path().join("test.db"));
        db.init().unwrap();

        let user_id = db
            .insert_user(&NewUser {
                email: "dev@example.com".to_string(),
                display_name: "Dev".to_string(),
                password_hash: None,
                avatar_url: None,
            })
            .unwrap();

        let user = db.get_user_by_email("dev@example.com").unwrap().unwrap();
        assert_eq!(user.id, user_id);
        assert!(user.password_hash.is_none());

        let activated = db
            .activate_user("dev@example.com", "hashed-password")
            .unwrap()
            .unwrap();
        assert_eq!(activated.password_hash.as_deref(), Some("hashed-password"));

        db.insert_message(&NewMessage {
            user_id,
            title: "review".to_string(),
            repo_name: Some("repo".to_string()),
            content: "full content".to_string(),
            summary: "summary".to_string(),
            report_path: Some("check/repo/report.md".to_string()),
            commit_range: Some("a..b".to_string()),
        })
        .unwrap();

        let (messages, total, unread_count) = db.list_messages(user_id, false, 1, 20).unwrap();
        assert_eq!(total, 1);
        assert_eq!(unread_count, 1);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].title, "review");

        let updated = db.mark_message_read(user_id, messages[0].id).unwrap();
        assert!(updated);
        assert_eq!(db.unread_message_count(user_id).unwrap(), 0);
    }

    #[test]
    fn next_run_from_five_field_cron_works() {
        let after = Utc.with_ymd_and_hms(2026, 3, 13, 8, 30, 0).unwrap();
        let next = next_run_from_cron("0 */1 * * 1-5", after).unwrap();

        assert_eq!(next, Utc.with_ymd_and_hms(2026, 3, 13, 9, 0, 0).unwrap());
    }

    #[test]
    fn next_run_from_five_field_cron_maps_sunday_correctly() {
        let after = Utc.with_ymd_and_hms(2026, 3, 13, 8, 30, 0).unwrap();
        let next = next_run_from_cron("0 9 * * 0", after).unwrap();

        assert_eq!(next, Utc.with_ymd_and_hms(2026, 3, 15, 9, 0, 0).unwrap());
    }
}
