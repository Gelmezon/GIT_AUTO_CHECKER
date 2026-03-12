use std::sync::Arc;

use anyhow::Result;
use tokio::sync::Semaphore;
use tokio::time::MissedTickBehavior;
use tracing::{error, info, warn};

use crate::config::AppConfig;
use crate::db::Database;
use crate::db::models::{Task, TaskStatus, TaskType};
use crate::executor::codex::CodexExecutor;
use crate::jobs::git_review;

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
    permits: Arc<Semaphore>,
}

impl Dispatcher {
    pub fn new(config: Arc<AppConfig>, database: Database) -> Self {
        let executor =
            CodexExecutor::new(config.codex.clone()).expect("executor config must be valid");
        let max_concurrency = config.scheduler.max_concurrency;
        Self {
            config,
            database,
            executor: Arc::new(executor),
            permits: Arc::new(Semaphore::new(max_concurrency)),
        }
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

            if let Err(error) = dispatcher.run_task(&task).await {
                error!(%error, task_id = task.id, "task execution failed");
                if let Err(finish_error) = dispatcher.database.finish_task(
                    &task,
                    TaskStatus::Failed,
                    Some(&error.to_string()),
                ) {
                    error!(%finish_error, task_id = task.id, "failed to persist task failure");
                }
            }

            drop(permit);
        });
    }

    async fn run_task(&self, task: &Task) -> Result<()> {
        match task.task_type {
            TaskType::Custom => {
                let output = self.executor.execute(&task.prompt).await?;
                self.database
                    .finish_task(task, TaskStatus::Done, Some(&output))?;
                info!(task_id = task.id, "custom task completed");
                Ok(())
            }
            TaskType::GitReview => {
                let output = git_review::execute(
                    self.config.clone(),
                    self.database.clone(),
                    &self.executor,
                    task,
                )
                .await?;
                info!(
                    task_id = task.id,
                    report = output,
                    "git review task completed"
                );
                Ok(())
            }
        }
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
