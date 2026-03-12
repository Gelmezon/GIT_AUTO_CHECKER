use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use chrono::Utc;

use crate::config::AppConfig;
use crate::db::Database;
use crate::db::models::{GitRepo, Task};
use crate::executor::codex::CodexExecutor;
use crate::jobs::JobOutput;
use crate::mcp::client::McpClient;
use crate::mcp::tools::git::{GitCloneArgs, GitDiffArgs, GitLogArgs, GitPullArgs};

pub async fn execute(
    config: Arc<AppConfig>,
    database: Database,
    executor: &CodexExecutor,
    task: &Task,
) -> Result<JobOutput> {
    let repo_id = task
        .repo_id
        .ok_or_else(|| anyhow!("git_review task requires repo_id"))?;
    let repo = database
        .get_repo(repo_id)?
        .ok_or_else(|| anyhow!("repository {repo_id} not found"))?;
    let mcp = McpClient::new(&config);

    sync_repo(&mcp, &repo).await?;
    let review_range = resolve_review_range(&mcp, &repo).await?;

    if review_range.head == review_range.from {
        database.update_repo_last_commit(repo.id, Some(&review_range.head))?;
        return Ok(JobOutput {
            task_result: "no new commits to review".to_string(),
            summary: "no new commits to review".to_string(),
            repo_name: Some(repo.name.clone()),
            report_path: None,
        });
    }

    let diff = mcp
        .git_diff(&GitDiffArgs {
            path: repo.local_path.clone(),
            from: review_range.from.clone(),
            to: Some(review_range.head.clone()),
        })
        .await?;
    if diff.is_empty {
        database.update_repo_last_commit(repo.id, Some(&review_range.head))?;
        return Ok(JobOutput {
            task_result: "diff is empty".to_string(),
            summary: "diff is empty".to_string(),
            repo_name: Some(repo.name.clone()),
            report_path: None,
        });
    }

    let prompt = build_review_prompt(
        task,
        &repo,
        &diff.patch,
        &diff.changed_files,
        &review_range.from,
        &review_range.head,
    );
    let review = executor
        .execute(&prompt, Some(Path::new(&repo.local_path)))
        .await?;
    let report_path = write_report(
        &config.runtime.check_dir,
        &repo,
        &review_range.from,
        &review_range.head,
        &review,
    )?;

    database.update_repo_last_commit(repo.id, Some(&review_range.head))?;

    Ok(JobOutput {
        task_result: report_path.to_string_lossy().to_string(),
        summary: review,
        repo_name: Some(repo.name),
        report_path: Some(report_path.to_string_lossy().to_string()),
    })
}

struct ReviewRange {
    from: String,
    head: String,
}

async fn resolve_review_range(mcp: &McpClient, repo: &GitRepo) -> Result<ReviewRange> {
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

    Ok(ReviewRange { from, head })
}

async fn sync_repo(mcp: &McpClient, repo: &GitRepo) -> Result<()> {
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
