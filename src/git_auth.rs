use anyhow::{Result, anyhow, bail};

use crate::config::AppConfig;
use crate::credentials::CredentialCipher;
use crate::db::Database;
use crate::db::models::{GitAuthType, GitCredential, GitPlatform, GitRepo};

#[derive(Debug, Clone)]
pub enum ResolvedGitAuth {
    Token { username: String, token: String },
    Basic { username: String, password: String },
    Ssh { username: String, key_path: String },
}

pub fn resolve_repo_auth_by_local_path(
    database: &Database,
    config: &AppConfig,
    local_path: &str,
) -> Result<Option<ResolvedGitAuth>> {
    let repo = match database.get_repo_by_local_path(local_path)? {
        Some(repo) => repo,
        None => return Ok(None),
    };
    resolve_repo_auth(database, config, &repo)
}

pub fn resolve_repo_auth_by_url_or_path(
    database: &Database,
    config: &AppConfig,
    repo_url: &str,
    local_path: &str,
) -> Result<Option<ResolvedGitAuth>> {
    let repo = database
        .get_repo_by_local_path(local_path)?
        .or_else(|| database.get_repo_by_repo_url(repo_url).ok().flatten());
    match repo {
        Some(repo) => resolve_repo_auth(database, config, &repo),
        None => Ok(None),
    }
}

fn resolve_repo_auth(
    database: &Database,
    config: &AppConfig,
    repo: &GitRepo,
) -> Result<Option<ResolvedGitAuth>> {
    let credential_id = match repo.credential_id {
        Some(id) => id,
        None => return Ok(None),
    };
    let credential = database
        .get_git_credential(credential_id)?
        .ok_or_else(|| anyhow!("git credential {credential_id} not found"))?;
    let cipher = CredentialCipher::from_hex_key(&config.credentials.encryption_key)?;
    Ok(Some(decrypt_git_credential(&cipher, credential)?))
}

fn decrypt_git_credential(
    cipher: &CredentialCipher,
    credential: GitCredential,
) -> Result<ResolvedGitAuth> {
    match credential.auth_type {
        GitAuthType::Token => {
            let token = credential
                .token
                .ok_or_else(|| anyhow!("git token credential is missing token"))?;
            let token = cipher.decrypt(&token)?;
            let username = credential
                .username
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| default_token_username(credential.platform).to_string());
            Ok(ResolvedGitAuth::Token { username, token })
        }
        GitAuthType::Basic => {
            let username = credential
                .username
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| anyhow!("git basic credential is missing username"))?;
            let password = credential
                .password
                .ok_or_else(|| anyhow!("git basic credential is missing password"))?;
            Ok(ResolvedGitAuth::Basic {
                username,
                password: cipher.decrypt(&password)?,
            })
        }
        GitAuthType::Ssh => {
            let key_path = credential
                .ssh_key_path
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| anyhow!("git ssh credential is missing ssh_key_path"))?;
            let username = credential
                .username
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "git".to_string());
            Ok(ResolvedGitAuth::Ssh { username, key_path })
        }
    }
}

pub fn validate_git_credential_payload(
    auth_type: GitAuthType,
    platform: GitPlatform,
    username: Option<&str>,
    token: Option<&str>,
    password: Option<&str>,
    ssh_key_path: Option<&str>,
) -> Result<()> {
    match auth_type {
        GitAuthType::Token => {
            if token.is_none_or(|value| value.trim().is_empty()) {
                bail!("token auth requires token");
            }
            let _ = username;
            let _ = platform;
        }
        GitAuthType::Basic => {
            if username.is_none_or(|value| value.trim().is_empty()) {
                bail!("basic auth requires username");
            }
            if password.is_none_or(|value| value.trim().is_empty()) {
                bail!("basic auth requires password");
            }
        }
        GitAuthType::Ssh => {
            if ssh_key_path.is_none_or(|value| value.trim().is_empty()) {
                bail!("ssh auth requires ssh_key_path");
            }
        }
    }
    Ok(())
}

fn default_token_username(platform: GitPlatform) -> &'static str {
    match platform {
        GitPlatform::Github => "x-access-token",
        GitPlatform::Gitlab | GitPlatform::Gitee | GitPlatform::Other => "oauth2",
    }
}
