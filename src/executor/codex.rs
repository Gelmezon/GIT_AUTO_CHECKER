use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use tokio::task::spawn_blocking;
use tokio::time::{sleep, timeout};

use crate::config::CodexConfig;

#[derive(Debug, Clone)]
pub struct CodexExecutor {
    config: CodexConfig,
}

impl CodexExecutor {
    pub fn new(config: CodexConfig) -> Result<Self> {
        validate_codex_config(&config)?;
        Ok(Self { config })
    }

    pub async fn execute(&self, prompt: &str, work_dir: Option<&Path>) -> Result<String> {
        let attempts = self.config.max_retries + 1;
        let mut last_error = None;

        for attempt in 0..attempts {
            match self.execute_once(prompt, work_dir).await {
                Ok(output) => return Ok(output),
                Err(error) => {
                    last_error = Some(error);
                    if attempt + 1 < attempts {
                        sleep(Duration::from_secs(1_u64 << attempt)).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("codex command failed without error")))
    }

    async fn execute_once(&self, prompt: &str, work_dir: Option<&Path>) -> Result<String> {
        let config = self.config.clone();
        let prompt = prompt.to_string();
        let work_dir = work_dir.map(Path::to_path_buf);

        let execution = spawn_blocking(move || run_codex_command(&config, &prompt, work_dir));
        let output = timeout(Duration::from_secs(self.config.timeout_secs), execution)
            .await
            .context("codex exec timed out")?
            .context("codex command join failed")??;

        Ok(parse_codex_output(&output.stdout)?)
    }
}

#[derive(Debug)]
struct CommandOutput {
    stdout: String,
}

fn run_codex_command(
    config: &CodexConfig,
    prompt: &str,
    work_dir: Option<PathBuf>,
) -> Result<CommandOutput> {
    let mut command = Command::new(&config.command);
    command.args([
        "exec",
        "--model",
        &config.model,
        "--approval-mode",
        "never",
        "--json",
    ]);
    if let Some(work_dir) = work_dir {
        command.arg("--path").arg(work_dir);
    }
    command.arg(prompt);

    if !config.api_key.trim().is_empty() {
        command.env("CODEX_API_KEY", &config.api_key);
        command.env("OPENAI_API_KEY", &config.api_key);
    }

    let output = command
        .output()
        .with_context(|| format!("failed to run codex command {}", config.command))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        bail!("codex exec failed: {}", stderr);
    }

    Ok(CommandOutput {
        stdout: String::from_utf8(output.stdout).context("codex output was not valid utf-8")?,
    })
}

fn validate_codex_config(config: &CodexConfig) -> Result<()> {
    if config.command.trim().is_empty() {
        bail!("codex command is empty");
    }
    Ok(())
}

fn parse_codex_output(stdout: &str) -> Result<String> {
    let mut messages = Vec::new();

    for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
        if let Ok(event) = serde_json::from_str::<CodexEvent>(line) {
            match event {
                CodexEvent::AgentMessage { content } if !content.trim().is_empty() => {
                    messages.push(content);
                }
                CodexEvent::TurnFailed { error } => bail!("codex turn failed: {error}"),
                CodexEvent::Other => {}
                CodexEvent::AgentMessage { .. } => {}
            }
        } else {
            messages.push(line.to_string());
        }
    }

    let output = messages.join("\n").trim().to_string();
    if output.is_empty() {
        bail!("codex returned no agent message");
    }
    Ok(output)
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum CodexEvent {
    #[serde(rename = "item.agent_message")]
    AgentMessage { content: String },
    #[serde(rename = "turn.failed")]
    TurnFailed { error: String },
    #[serde(other)]
    Other,
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use tempfile::tempdir;

    use super::*;
    use crate::config::CodexConfig;

    #[tokio::test]
    async fn executor_parses_jsonl_agent_messages() {
        let dir = tempdir().unwrap();
        let command = create_fake_codex_command(
            dir.path(),
            "Write-Output '{\"type\":\"thread.started\"}'\nWrite-Output '{\"type\":\"item.agent_message\",\"content\":\"review result\"}'\n",
        );

        let executor = CodexExecutor::new(CodexConfig {
            api_key: "test-key".to_string(),
            command: command.to_string_lossy().to_string(),
            model: "gpt-5.4".to_string(),
            max_retries: 0,
            timeout_secs: 5,
        })
        .unwrap();

        let output = executor.execute("hello", None).await.unwrap();
        assert_eq!(output, "review result");
    }

    #[tokio::test]
    async fn executor_allows_local_codex_auth_without_api_key() {
        let dir = tempdir().unwrap();
        let command = create_fake_codex_command(
            dir.path(),
            "Write-Output '{\"type\":\"item.agent_message\",\"content\":\"ok\"}'\n",
        );

        let executor = CodexExecutor::new(CodexConfig {
            api_key: String::new(),
            command: command.to_string_lossy().to_string(),
            model: "gpt-5.4".to_string(),
            max_retries: 0,
            timeout_secs: 5,
        })
        .unwrap();

        let output = executor.execute("hello", None).await.unwrap();
        assert_eq!(output, "ok");
    }

    #[tokio::test]
    async fn executor_includes_work_dir_argument() {
        let dir = tempdir().unwrap();
        let target_dir = dir.path().join("repo");
        fs::create_dir_all(&target_dir).unwrap();
        let capture_path = dir.path().join("args.txt");
        let script = format!(
            "$args -join \"\\n\" | Set-Content -Encoding utf8 '{}'\nWrite-Output '{{\"type\":\"item.agent_message\",\"content\":\"ok\"}}'\n",
            capture_path.display()
        );
        let command = create_fake_codex_command(dir.path(), &script);

        let executor = CodexExecutor::new(CodexConfig {
            api_key: "test-key".to_string(),
            command: command.to_string_lossy().to_string(),
            model: "gpt-5.4".to_string(),
            max_retries: 0,
            timeout_secs: 5,
        })
        .unwrap();

        let output = executor
            .execute("payload", Some(&target_dir))
            .await
            .unwrap();
        assert_eq!(output, "ok");

        let args = fs::read_to_string(capture_path).unwrap();
        assert!(args.contains("--path"));
        assert!(args.contains(&target_dir.to_string_lossy().to_string()));
    }

    #[cfg(windows)]
    fn create_fake_codex_command(dir: &Path, body: &str) -> PathBuf {
        let script_path = dir.join("codex-script.ps1");
        fs::write(&script_path, body).unwrap();
        let wrapper = dir.join("codex.cmd");
        fs::write(
            &wrapper,
            format!(
                "@echo off\r\npowershell -ExecutionPolicy Bypass -File \"{}\" %*\r\n",
                script_path.display()
            ),
        )
        .unwrap();
        wrapper
    }

    #[cfg(not(windows))]
    fn create_fake_codex_command(dir: &Path, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let script = dir.join("codex");
        fs::write(&script, format!("#!/usr/bin/env bash\n{}", body)).unwrap();
        let mut permissions = fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).unwrap();
        script
    }
}
