use std::path::Path;

use anyhow::{Context, Result};
use git2::{
    AutotagOption, Cred, DiffFormat, FetchOptions, Oid, RemoteCallbacks, Repository,
    build::RepoBuilder,
};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::git_auth::ResolvedGitAuth;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitCloneArgs {
    pub url: String,
    pub path: String,
    #[serde(default)]
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitPullArgs {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitLogArgs {
    pub path: String,
    #[serde(default)]
    pub count: Option<usize>,
    #[serde(default)]
    pub since: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitDiffArgs {
    pub path: String,
    pub from: String,
    #[serde(default)]
    pub to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStatusArgs {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitCloneOutput {
    pub path: String,
    pub branch: String,
    pub head: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitPullOutput {
    pub branch: String,
    pub updated: bool,
    pub head: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitEntry {
    pub id: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitLogOutput {
    pub entries: Vec<CommitEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitDiffOutput {
    pub from: String,
    pub to: String,
    pub changed_files: Vec<String>,
    pub patch: String,
    pub is_empty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStatusOutput {
    pub branch: Option<String>,
    pub head: Option<String>,
    pub clean: bool,
    pub entries: Vec<String>,
}

pub fn git_clone(args: &GitCloneArgs, auth: Option<&ResolvedGitAuth>) -> Result<GitCloneOutput> {
    let mut builder = RepoBuilder::new();
    if let Some(branch) = &args.branch {
        builder.branch(branch);
    }
    builder.fetch_options(fetch_options(auth));

    let repo = match builder.clone(&args.url, Path::new(&args.path)) {
        Ok(repo) => repo,
        Err(error) => {
            log_git2_error(
                "git clone failed",
                Some(&args.url),
                &args.path,
                args.branch.as_deref(),
                auth,
                &error,
            );
            return Err(error).with_context(|| format!("failed to clone {}", args.url));
        }
    };
    Ok(GitCloneOutput {
        path: args.path.clone(),
        branch: args.branch.clone().unwrap_or_else(|| "HEAD".to_string()),
        head: head_commit(&repo)?,
    })
}

pub fn git_pull(args: &GitPullArgs, auth: Option<&ResolvedGitAuth>) -> Result<GitPullOutput> {
    let repo = Repository::open(&args.path)
        .with_context(|| format!("failed to open repository {}", args.path))?;
    let branch = current_branch(&repo)?;
    let mut remote = repo
        .find_remote("origin")
        .context("origin remote not found")?;
    let remote_url = remote.url().map(str::to_string);
    let refspec = format!("refs/heads/{branch}:refs/remotes/origin/{branch}");
    let mut fetch = fetch_options(auth);
    if let Err(error) = remote.fetch(&[&refspec], Some(&mut fetch), None) {
        log_git2_error(
            "git fetch failed",
            remote_url.as_deref(),
            &args.path,
            Some(&branch),
            auth,
            &error,
        );
        return Err(error).context("git fetch failed");
    }

    let fetch_head = repo.find_reference(&format!("refs/remotes/origin/{branch}"))?;
    let fetch_commit = repo.reference_to_annotated_commit(&fetch_head)?;
    let (analysis, _) = repo.merge_analysis(&[&fetch_commit])?;

    let updated = if analysis.is_fast_forward() {
        fast_forward(&repo, &branch, &fetch_commit.id())?;
        true
    } else {
        false
    };

    Ok(GitPullOutput {
        branch,
        updated,
        head: head_commit(&repo)?,
    })
}

pub fn git_log(args: &GitLogArgs) -> Result<GitLogOutput> {
    let repo = Repository::open(&args.path)
        .with_context(|| format!("failed to open repository {}", args.path))?;
    let mut walk = repo.revwalk()?;
    walk.push_head()?;

    let entries = walk
        .take(args.count.unwrap_or(10))
        .filter_map(Result::ok)
        .filter_map(|oid| repo.find_commit(oid).ok())
        .filter(|commit| match &args.since {
            Some(since) => commit.time().seconds().to_string() >= *since,
            None => true,
        })
        .map(|commit| CommitEntry {
            id: commit.id().to_string(),
            summary: commit.summary().unwrap_or("").to_string(),
        })
        .collect();

    Ok(GitLogOutput { entries })
}

pub fn diff_repo(args: &GitDiffArgs) -> Result<GitDiffOutput> {
    let repo = Repository::open(&args.path)
        .with_context(|| format!("failed to open repository {}", args.path))?;
    let from_commit = repo
        .find_commit(Oid::from_str(&args.from)?)
        .with_context(|| format!("failed to find commit {}", args.from))?;
    let to_oid = args
        .to
        .as_deref()
        .map(Oid::from_str)
        .transpose()?
        .unwrap_or(repo.head()?.target().context("head has no target")?);
    let to_commit = repo
        .find_commit(to_oid)
        .with_context(|| format!("failed to find commit {to_oid}"))?;

    let from_tree = from_commit.tree()?;
    let to_tree = to_commit.tree()?;
    let diff = repo.diff_tree_to_tree(Some(&from_tree), Some(&to_tree), None)?;

    let changed_files = diff
        .deltas()
        .filter_map(|delta| {
            delta
                .new_file()
                .path()
                .or_else(|| delta.old_file().path())
                .map(|path| path.display().to_string())
        })
        .collect::<Vec<_>>();

    let mut patch = String::new();
    diff.print(DiffFormat::Patch, |_delta, _hunk, line| {
        patch.push_str(std::str::from_utf8(line.content()).unwrap_or_default());
        true
    })?;

    Ok(GitDiffOutput {
        from: args.from.clone(),
        to: to_commit.id().to_string(),
        changed_files,
        is_empty: patch.trim().is_empty(),
        patch,
    })
}

pub fn git_status(args: &GitStatusArgs) -> Result<GitStatusOutput> {
    let repo = Repository::open(&args.path)
        .with_context(|| format!("failed to open repository {}", args.path))?;
    let statuses = repo.statuses(None)?;
    let entries = statuses
        .iter()
        .filter_map(|entry| entry.path().map(str::to_string))
        .collect::<Vec<_>>();

    Ok(GitStatusOutput {
        branch: repo
            .head()
            .ok()
            .and_then(|head| head.shorthand().map(str::to_string)),
        head: head_commit(&repo)?,
        clean: entries.is_empty(),
        entries,
    })
}

fn fetch_options(auth: Option<&ResolvedGitAuth>) -> FetchOptions<'static> {
    let auth = auth.cloned();
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(move |_url, username, _allowed| match auth.as_ref() {
        Some(ResolvedGitAuth::Token {
            username: auth_username,
            token,
        }) => Cred::userpass_plaintext(auth_username, token),
        Some(ResolvedGitAuth::Basic {
            username: auth_username,
            password,
        }) => Cred::userpass_plaintext(auth_username, password),
        Some(ResolvedGitAuth::Ssh {
            username: auth_username,
            key_path,
        }) => Cred::ssh_key(
            username.unwrap_or(auth_username),
            None,
            Path::new(key_path),
            None,
        ),
        None => Cred::credential_helper(
            &git2::Config::open_default()?,
            username.unwrap_or("git"),
            None,
        ),
    });

    let mut options = FetchOptions::new();
    options.remote_callbacks(callbacks);
    options.download_tags(AutotagOption::All);
    options
}

fn log_git2_error(
    operation: &str,
    remote_url: Option<&str>,
    repo_path: &str,
    branch: Option<&str>,
    auth: Option<&ResolvedGitAuth>,
    error: &git2::Error,
) {
    error!(
        operation,
        repo_path,
        remote_url = remote_url.unwrap_or("<unknown>"),
        branch = branch.unwrap_or("<unknown>"),
        auth_mode = auth_mode_label(auth),
        git2_code = ?error.code(),
        git2_class = ?error.class(),
        git2_message = %error.message(),
        "git operation failed"
    );
}

fn auth_mode_label(auth: Option<&ResolvedGitAuth>) -> &'static str {
    match auth {
        Some(ResolvedGitAuth::Token { .. }) => "token",
        Some(ResolvedGitAuth::Basic { .. }) => "basic",
        Some(ResolvedGitAuth::Ssh { .. }) => "ssh",
        None => "credential-helper",
    }
}

fn head_commit(repo: &Repository) -> Result<Option<String>> {
    Ok(repo
        .head()
        .ok()
        .and_then(|head| head.target().map(|oid| oid.to_string())))
}

fn current_branch(repo: &Repository) -> Result<String> {
    Ok(repo.head()?.shorthand().unwrap_or("main").to_string())
}

fn fast_forward(repo: &Repository, branch: &str, oid: &Oid) -> Result<()> {
    let refname = format!("refs/heads/{branch}");
    let mut reference = repo
        .find_reference(&refname)
        .or_else(|_| repo.reference(&refname, *oid, true, "create fast-forward reference"))?;
    reference.set_target(*oid, "fast-forward")?;
    repo.set_head(&refname)?;
    repo.checkout_head(Some(
        git2::build::CheckoutBuilder::default()
            .allow_conflicts(false)
            .force(),
    ))?;
    Ok(())
}
