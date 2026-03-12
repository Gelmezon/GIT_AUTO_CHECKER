use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use git_helper::config::AppConfig;
use git_helper::db::Database;
use git_helper::db::models::{NewGitRepo, NewTask, NewUser, TaskType};
use git_helper::scheduler::{AppContext, Dispatcher};
use tokio::signal;
use tracing::info;

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Cli {
    #[arg(long, default_value = "config.toml")]
    config: PathBuf,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    Run,
    AddRepo {
        #[arg(long)]
        name: String,
        #[arg(long)]
        repo_url: String,
        #[arg(long, default_value = "main")]
        branch: String,
        #[arg(long)]
        local_path: PathBuf,
        #[arg(long)]
        review_cron: Option<String>,
    },
    AddTask {
        #[arg(long)]
        name: String,
        #[arg(long)]
        prompt: String,
        #[arg(long)]
        task_type: String,
        #[arg(long)]
        repo_id: Option<i64>,
        #[arg(long)]
        cron_expr: Option<String>,
        #[arg(long)]
        scheduled_at: Option<String>,
    },
    ListRepos,
    ListTasks,
    AddUser {
        #[arg(long)]
        email: String,
        #[arg(long)]
        display_name: String,
    },
    ListUsers,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = AppConfig::load(&cli.config)?;
    let _guard = config.init_logging()?;

    let database = Database::new(&config.database.path);
    database.init()?;

    match cli.command.unwrap_or(Command::Run) {
        Command::Run => run(config, database).await,
        Command::AddRepo {
            name,
            repo_url,
            branch,
            local_path,
            review_cron,
        } => {
            let repo = NewGitRepo {
                name,
                repo_url,
                branch,
                local_path: local_path.to_string_lossy().to_string(),
                review_cron,
            };
            let id = database.insert_repo(&repo)?;
            info!(repo_id = id, "repository added");
            Ok(())
        }
        Command::AddTask {
            name,
            prompt,
            task_type,
            repo_id,
            cron_expr,
            scheduled_at,
        } => {
            let scheduled_at = scheduled_at
                .as_deref()
                .map(parse_datetime)
                .transpose()?
                .unwrap_or_else(Utc::now);
            let task_type = TaskType::from_cli(&task_type)?;
            let task = NewTask {
                name,
                task_type,
                repo_id,
                prompt,
                cron_expr,
                scheduled_at,
            };
            let id = database.insert_task(&task)?;
            info!(task_id = id, "task added");
            Ok(())
        }
        Command::ListRepos => {
            for repo in database.list_repos()? {
                println!(
                    "{}\t{}\t{}\t{}",
                    repo.id, repo.name, repo.branch, repo.local_path
                );
            }
            Ok(())
        }
        Command::ListTasks => {
            for task in database.list_tasks()? {
                println!(
                    "{}\t{}\t{}\t{}",
                    task.id,
                    task.name,
                    task.task_type.as_str(),
                    task.status.as_str()
                );
            }
            Ok(())
        }
        Command::AddUser {
            email,
            display_name,
        } => {
            let id = database.insert_user(&NewUser {
                email,
                display_name,
                password_hash: None,
                avatar_url: None,
            })?;
            info!(user_id = id, "user added");
            Ok(())
        }
        Command::ListUsers => {
            for user in database.list_users()? {
                println!("{}\t{}\t{}", user.id, user.email, user.display_name);
            }
            Ok(())
        }
    }
}

async fn run(config: AppConfig, database: Database) -> Result<()> {
    let config = Arc::new(config);
    let dispatcher = Dispatcher::new(config.clone(), database.clone())?;
    let scheduler_context = AppContext {
        config: config.clone(),
        database: database.clone(),
        dispatcher,
    };

    let server = tokio::spawn(git_helper::mcp::serve(config.clone(), database.clone()));
    let scheduler = tokio::spawn(git_helper::scheduler::run(scheduler_context));

    tokio::select! {
        result = server => {
            result.context("mcp task join failure")??;
        }
        result = scheduler => {
            result.context("scheduler task join failure")??;
        }
        ctrl = signal::ctrl_c() => {
            ctrl.context("failed to listen for ctrl-c")?;
            info!("shutdown signal received");
        }
    }

    Ok(())
}

fn parse_datetime(input: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(input)
        .with_context(|| format!("invalid RFC3339 datetime: {input}"))?
        .with_timezone(&Utc))
}
