pub mod models;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use cron::Schedule;
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::db::models::{
    GitRepo, Message, NewGitRepo, NewMessage, NewTask, NewUser, Task, TaskStatus, User,
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

            CREATE INDEX IF NOT EXISTS idx_tasks_schedule ON tasks(status, scheduled_at);
            CREATE INDEX IF NOT EXISTS idx_tasks_repo_id ON tasks(repo_id);
            CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
            CREATE INDEX IF NOT EXISTS idx_messages_user_created ON messages(user_id, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_messages_user_unread ON messages(user_id, is_read);
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
}
