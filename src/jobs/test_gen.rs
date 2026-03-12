use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow, bail};
use chrono::Utc;

use crate::config::AppConfig;
use crate::db::Database;
use crate::db::models::{GitRepo, Task};
use crate::executor::codex::CodexExecutor;
use crate::jobs::JobOutput;
use crate::mcp::client::McpClient;
use crate::mcp::tools::git::{GitCloneArgs, GitDiffArgs, GitLogArgs, GitPullArgs};

const TEST_GEN_PROMPT_TEMPLATE: &str = include_str!("../prompts/test_gen.md");

pub async fn execute(
    config: Arc<AppConfig>,
    database: Database,
    executor: &CodexExecutor,
    task: &Task,
) -> Result<JobOutput> {
    let repo_id = task
        .repo_id
        .ok_or_else(|| anyhow!("test_gen task requires repo_id"))?;
    let repo = database
        .get_repo(repo_id)?
        .ok_or_else(|| anyhow!("repository {repo_id} not found"))?;
    let mcp = McpClient::new(&config);

    sync_repo(&mcp, &repo).await?;
    let review_range = resolve_range(&mcp, &repo).await?;

    if review_range.head == review_range.from {
        database.update_repo_last_commit(repo.id, Some(&review_range.head))?;
        return Ok(JobOutput {
            task_result: "no new commits to generate tests".to_string(),
            summary: "no new commits to generate tests".to_string(),
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

    let language = detect_language(Path::new(&repo.local_path));
    let existing_test_dir = detect_test_dir(Path::new(&repo.local_path))
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "-".to_string());
    let prompt = build_test_gen_prompt(&task.prompt, &language, &existing_test_dir, &diff.patch);
    let generated = executor
        .execute(&prompt, Some(Path::new(&repo.local_path)))
        .await?;

    let output_dir = config
        .runtime
        .tests_generated_dir
        .join(&repo.name)
        .join(Utc::now().format("%Y-%m-%d").to_string());
    let write_result = write_generated_tests(&output_dir, &generated)?;
    database.update_repo_last_commit(repo.id, Some(&review_range.head))?;

    Ok(JobOutput {
        task_result: output_dir.to_string_lossy().to_string(),
        summary: write_result.summary,
        repo_name: Some(repo.name),
        report_path: Some(write_result.summary_path.to_string_lossy().to_string()),
    })
}

struct ReviewRange {
    from: String,
    head: String,
}

struct GeneratedWriteResult {
    summary: String,
    summary_path: PathBuf,
}

async fn resolve_range(mcp: &McpClient, repo: &GitRepo) -> Result<ReviewRange> {
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

fn build_test_gen_prompt(
    base_prompt: &str,
    language: &str,
    existing_test_dir: &str,
    diff_content: &str,
) -> String {
    let template = TEST_GEN_PROMPT_TEMPLATE
        .replace("{language}", language)
        .replace("{existing_test_dir}", existing_test_dir)
        .replace("{diff_content}", diff_content);

    if base_prompt.trim().is_empty() {
        template
    } else {
        format!("{base_prompt}\n\n{template}")
    }
}

fn detect_language(repo_path: &Path) -> String {
    if repo_path.join("Cargo.toml").exists() {
        "Rust".to_string()
    } else if repo_path.join("package.json").exists() {
        "JavaScript/TypeScript".to_string()
    } else if repo_path.join("go.mod").exists() {
        "Go".to_string()
    } else if repo_path.join("pyproject.toml").exists()
        || repo_path.join("setup.py").exists()
        || repo_path.join("requirements.txt").exists()
    {
        "Python".to_string()
    } else if repo_path.join("pom.xml").exists() || repo_path.join("build.gradle").exists() {
        "Java".to_string()
    } else {
        "Unknown".to_string()
    }
}

fn detect_test_dir(repo_path: &Path) -> Option<PathBuf> {
    // First, check for common test directory names at the repository root (existing behavior).
    for candidate in ["tests", "test", "__tests__"] {
        let path = repo_path.join(candidate);
        if path.is_dir() {
            return Some(path);
        }
    }

    // If nothing is found at the root, perform a bounded recursive search for test directories.
    // This helps catch common layouts like src/tests, app/tests, or nested __tests__ folders.
    fn find_test_dir_recursive(dir: &Path, depth: usize, max_depth: usize) -> Option<PathBuf> {
        if depth > max_depth {
            return None;
        }

        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => return None,
        };

        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if ["tests", "test", "__tests__"].iter().any(|candidate| *candidate == name) {
                    return Some(path);
                }
                if let Some(found) = find_test_dir_recursive(&path, depth + 1, max_depth) {
                    return Some(found);
                }
            }
        }

        None
    }

    // Search up to a small depth to balance coverage and performance.
    find_test_dir_recursive(repo_path, 0, 3)
}

fn write_generated_tests(base_dir: &Path, generated: &str) -> Result<GeneratedWriteResult> {
    fs::create_dir_all(base_dir)
        .with_context(|| format!("failed to create {}", base_dir.display()))?;

    let sections = parse_generated_sections(generated);
    let summary_path = base_dir.join("_summary.md");
    let summary = if let Some(summary) = sections.summary {
        summary
    } else {
        generated.to_string()
    };
    fs::write(&summary_path, &summary)
        .with_context(|| format!("failed to write {}", summary_path.display()))?;

    for file in sections.files {
        let path = sanitize_output_path(base_dir, &file.path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&path, file.content)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }

    Ok(GeneratedWriteResult {
        summary,
        summary_path,
    })
}

struct ParsedSections {
    files: Vec<GeneratedFile>,
    summary: Option<String>,
}

struct GeneratedFile {
    path: String,
    content: String,
}

fn parse_generated_sections(generated: &str) -> ParsedSections {
    let mut files = Vec::new();
    let mut summary = None;
    let mut current_path: Option<String> = None;
    let mut current_lines = Vec::new();
    let mut summary_lines = Vec::new();
    let mut in_summary = false;

    for line in generated.lines() {
        if let Some(path) = line
            .strip_prefix("=== FILE: ")
            .and_then(|line| line.strip_suffix(" ==="))
        {
            if let Some(path) = current_path.take() {
                files.push(GeneratedFile {
                    path,
                    content: strip_code_fence(&current_lines.join("\n")),
                });
                current_lines.clear();
            }
            in_summary = false;
            current_path = Some(path.trim().to_string());
            continue;
        }

        if line.trim() == "=== SUMMARY ===" {
            if let Some(path) = current_path.take() {
                files.push(GeneratedFile {
                    path,
                    content: strip_code_fence(&current_lines.join("\n")),
                });
                current_lines.clear();
            }
            in_summary = true;
            continue;
        }

        if in_summary {
            summary_lines.push(line);
        } else if current_path.is_some() {
            current_lines.push(line);
        }
    }

    if let Some(path) = current_path.take() {
        files.push(GeneratedFile {
            path,
            content: strip_code_fence(&current_lines.join("\n")),
        });
    }
    if !summary_lines.is_empty() {
        summary = Some(summary_lines.join("\n").trim().to_string());
    }

    ParsedSections { files, summary }
}

fn strip_code_fence(content: &str) -> String {
    let trimmed = content.trim();
    if let Some(rest) = trimmed.strip_prefix("```") {
        let mut lines = rest.lines();
        let _ = lines.next();
        let body = lines.collect::<Vec<_>>().join("\n");
        return body.strip_suffix("```").unwrap_or(&body).trim().to_string();
    }
    trimmed.to_string()
}

fn sanitize_output_path(base_dir: &Path, relative: &str) -> Result<PathBuf> {
    let relative = relative.replace('\\', "/");
    let mut path = PathBuf::from(base_dir);
    for segment in relative.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            bail!("generated file path cannot escape output directory");
        }
        path.push(segment);
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_generated_files_and_summary() {
        let parsed = parse_generated_sections(
            "=== FILE: tests/test_login.py ===\n```python\nassert True\n```\n=== SUMMARY ===\ncreated 1 file\n",
        );

        assert_eq!(parsed.files.len(), 1);
        assert_eq!(parsed.files[0].path, "tests/test_login.py");
        assert_eq!(parsed.files[0].content, "assert True");
        assert_eq!(parsed.summary.as_deref(), Some("created 1 file"));
    }
}
