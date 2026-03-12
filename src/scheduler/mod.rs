use std::sync::Arc;

use anyhow::Result;
use tokio::sync::Semaphore;
use tokio::time::MissedTickBehavior;
use tracing::{error, info, warn};

use crate::config::AppConfig;
use crate::db::Database;
use crate::db::models::{NewMessage, Task, TaskStatus, TaskType};
use crate::executor::codex::CodexExecutor;
use crate::jobs::{self, JobOutput};
use crate::notifier::{Notification, NotifierDispatcher};

#[derive(Clone)]
pub struct AppContext {
    pub config: Arc<AppConfig>,
    pub database: Database,
    pub dispatcher: Dispatcher,
}

#[derive(Clone)]
pub struct Dispatcher {
    config: Arc<AppConfig>,
    database: Database,
    executor: Arc<CodexExecutor>,
    notifier: NotifierDispatcher,
    permits: Arc<Semaphore>,
}

impl Dispatcher {
    pub fn new(config: Arc<AppConfig>, database: Database) -> Result<Self> {
        let executor = CodexExecutor::new(config.codex.clone())?;
        let notifier = NotifierDispatcher::from_config(&config.notifier)?;
        let max_concurrency = config.scheduler.max_concurrency;
        Ok(Self {
            config,
            database,
            executor: Arc::new(executor),
            notifier,
            permits: Arc::new(Semaphore::new(max_concurrency)),
        })
    }

    pub fn dispatch(&self, task: Task) {
        let dispatcher = self.clone();
        tokio::spawn(async move {
            let permit = match dispatcher.permits.acquire().await {
                Ok(permit) => permit,
                Err(error) => {
                    error!(%error, task_id = task.id, "failed to acquire semaphore");
                    return;
                }
            };

            let result = dispatcher.run_task(&task).await;
            if let Err(error) = dispatcher.complete_task(&task, result).await {
                error!(%error, task_id = task.id, "failed to finalize task");
            }

            drop(permit);
        });
    }

    async fn run_task(&self, task: &Task) -> Result<JobOutput> {
        match task.task_type {
            TaskType::Custom => {
                let output = self.executor.execute(&task.prompt, None).await?;
                Ok(JobOutput {
                    task_result: output.clone(),
                    content: output.clone(),
                    summary: output,
                    repo_name: None,
                    report_path: None,
                    commit_range: None,
                })
            }
            TaskType::GitReview => {
                jobs::git_review::execute(
                    self.config.clone(),
                    self.database.clone(),
                    &self.executor,
                    task,
                )
                .await
            }
            TaskType::TestGen => {
                jobs::test_gen::execute(
                    self.config.clone(),
                    self.database.clone(),
                    &self.executor,
                    task,
                )
                .await
            }
        }
    }

    async fn complete_task(&self, task: &Task, result: Result<JobOutput>) -> Result<()> {
        match result {
            Ok(output) => {
                self.database
                    .finish_task(task, TaskStatus::Done, Some(&output.task_result))?;
                self.persist_message(task, &output)?;
                info!(task_id = task.id, "task completed");
                self.notify(
                    task,
                    TaskStatus::Done,
                    &output.summary,
                    output.repo_name,
                    output.report_path,
                )
                .await;
            }
            Err(error) => {
                let message = error.to_string();
                self.database
                    .finish_task(task, TaskStatus::Failed, Some(&message))?;
                error!(%error, task_id = task.id, "task execution failed");
                self.notify(task, TaskStatus::Failed, &message, None, None)
                    .await;
            }
        }
        Ok(())
    }

    fn persist_message(&self, task: &Task, output: &JobOutput) -> Result<()> {
        let users = self.database.list_users()?;
        if users.is_empty() {
            return Ok(());
        }

        let title = build_message_title(task, output);
        for user in users {
            self.database.insert_message(&NewMessage {
                user_id: user.id,
                title: title.clone(),
                repo_name: output.repo_name.clone(),
                content: output.content.clone(),
                summary: truncate_chars(&output.summary, 500).to_string(),
                report_path: output.report_path.clone(),
                commit_range: output.commit_range.clone(),
            })?;
        }

        Ok(())
    }

    async fn notify(
        &self,
        task: &Task,
        status: TaskStatus,
        summary: &str,
        repo_name: Option<String>,
        report_path: Option<String>,
    ) {
        if !self.notifier.is_enabled() {
            return;
        }

        let duration_secs = task
            .started_at
            .map(|started| (chrono::Utc::now() - started).num_seconds().max(0) as u64)
            .unwrap_or_default();
        self.notifier
            .broadcast(Notification {
                task_name: task.name.clone(),
                task_type: task.task_type.as_str().to_string(),
                repo_name,
                status: status.as_str().to_string(),
                summary: truncate_chars(summary, 500).to_string(),
                report_path,
                duration_secs,
            })
            .await;
    }
}

pub async fn run(context: AppContext) -> Result<()> {
    let recovered = context
        .database
        .recover_stalled_tasks(context.config.task_timeout())?;
    if recovered > 0 {
        warn!(recovered, "recovered stalled tasks");
    }

    let mut ticker = tokio::time::interval(context.config.scheduler_interval());
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        ticker.tick().await;
        let tasks = context
            .database
            .claim_due_tasks(context.config.scheduler.claim_batch_size)?;
        if !tasks.is_empty() {
            info!(count = tasks.len(), "claimed due tasks");
        }
        for task in tasks {
            context.dispatcher.dispatch(task);
        }
    }
}

fn truncate_chars(input: &str, max_chars: usize) -> &str {
    match input.char_indices().nth(max_chars) {
        Some((idx, _)) => &input[..idx],
        None => input,
    }
}

fn build_message_title(task: &Task, output: &JobOutput) -> String {
    match (&output.repo_name, &output.commit_range) {
        (Some(repo_name), Some(commit_range)) => {
            format!("{repo_name} {} {}", task.task_type.as_str(), commit_range)
        }
        (Some(repo_name), None) => format!("{repo_name} {}", task.task_type.as_str()),
        _ => task.name.clone(),
    }
}
