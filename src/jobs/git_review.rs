use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use chrono::Utc;

use crate::config::AppConfig;
use crate::db::Database;
use crate::db::models::{GitRepo, Task, TaskStatus};
use crate::executor::codex::CodexExecutor;
use crate::mcp::client::McpClient;
use crate::mcp::tools::git::{GitCloneArgs, GitDiffArgs, GitLogArgs, GitPullArgs};

pub async fn execute(
    config: Arc<AppConfig>,
    database: Database,
    executor: &CodexExecutor,
    task: &Task,
) -> Result<String> {
    let repo_id = task
        .repo_id
        .ok_or_else(|| anyhow!("git_review task requires repo_id"))?;
    let repo = database
        .get_repo(repo_id)?
        .ok_or_else(|| anyhow!("repository {repo_id} not found"))?;
    let mcp = McpClient::new(&config);

    ensure_local_repo(&mcp, &repo).await?;

    let log = mcp
        .git_log(&GitLogArgs {
            path: repo.local_path.clone(),
            count: Some(2),
            since: None,
        })
        .await?;
    let head = log
        .entries
        .first()
        .map(|entry| entry.id.clone())
        .ok_or_else(|| anyhow!("repository has no commits"))?;

    let from = repo
        .last_commit
        .clone()
        .or_else(|| log.entries.get(1).map(|entry| entry.id.clone()))
        .unwrap_or_else(|| head.clone());

    if from == head {
        database.update_repo_last_commit(repo.id, Some(&head))?;
        database.finish_task(task, TaskStatus::Done, Some("no new commits to review"))?;
        return Ok("no new commits to review".to_string());
    }

    let diff = mcp
        .git_diff(&GitDiffArgs {
            path: repo.local_path.clone(),
            from,
            to: Some(head.clone()),
        })
        .await?;
    if diff.is_empty {
        database.update_repo_last_commit(repo.id, Some(&head))?;
        database.finish_task(task, TaskStatus::Done, Some("diff is empty"))?;
        return Ok("diff is empty".to_string());
    }

    let prompt = build_review_prompt(
        task,
        &repo,
        &diff.patch,
        &diff.changed_files,
        &diff.from,
        &head,
    );
    let review = executor.execute(&prompt).await?;
    let report_path = write_report(&config.runtime.check_dir, &repo, &diff.from, &head, &review)?;

    database.update_repo_last_commit(repo.id, Some(&head))?;
    database.finish_task(
        task,
        TaskStatus::Done,
        Some(report_path.to_string_lossy().as_ref()),
    )?;

    Ok(review)
}

async fn ensure_local_repo(mcp: &McpClient, repo: &GitRepo) -> Result<()> {
    let git_dir = Path::new(&repo.local_path).join(".git");
    if !git_dir.exists() {
        mcp.git_clone(&GitCloneArgs {
            url: repo.repo_url.clone(),
            path: repo.local_path.clone(),
            branch: Some(repo.branch.clone()),
        })
        .await?;
    } else {
        mcp.git_pull(&GitPullArgs {
            path: repo.local_path.clone(),
        })
        .await?;
    }
    Ok(())
}

fn build_review_prompt(
    task: &Task,
    repo: &GitRepo,
    diff: &str,
    changed_files: &[String],
    from: &str,
    to: &str,
) -> String {
    format!(
        "{}\n\nRepository: {}\nCommit range: {}..{}\nChanged files:\n{}\n\nPlease perform a code review focused on bugs, regressions, security risks, and missing tests.\n\nDiff:\n{}",
        task.prompt,
        repo.name,
        from,
        to,
        changed_files.join("\n"),
        diff
    )
}

fn write_report(
    base_dir: &Path,
    repo: &GitRepo,
    from: &str,
    to: &str,
    review: &str,
) -> Result<PathBuf> {
    let repo_dir = base_dir.join(&repo.name);
    fs::create_dir_all(&repo_dir)
        .with_context(|| format!("failed to create {}", repo_dir.display()))?;
    let filename = format!("{}-review.md", Utc::now().format("%Y-%m-%d"));
    let path = repo_dir.join(filename);
    let body = format!(
        "# Git Review\n\n- Repository: {}\n- Commit range: {}..{}\n- Generated at: {}\n\n{}\n",
        repo.name,
        from,
        to,
        Utc::now().to_rfc3339(),
        review
    );
    fs::write(&path, body).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}
