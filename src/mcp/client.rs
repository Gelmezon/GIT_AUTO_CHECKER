use anyhow::{Context, Result, anyhow};
use reqwest::Client;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::config::AppConfig;
use crate::mcp::protocol::ToolResponse;
use crate::mcp::tools::git::{
    GitCloneArgs, GitCloneOutput, GitDiffArgs, GitDiffOutput, GitLogArgs, GitLogOutput,
    GitPullArgs, GitPullOutput, GitStatusArgs, GitStatusOutput,
};

#[derive(Debug, Clone)]
pub struct McpClient {
    client: Client,
    endpoint: String,
}

impl McpClient {
    pub fn new(config: &AppConfig) -> Self {
        Self {
            client: Client::new(),
            endpoint: format!("{}/mcp", config.mcp_base_url()),
        }
    }

    pub async fn git_clone(&self, args: &GitCloneArgs) -> Result<GitCloneOutput> {
        self.call_tool("git_clone", args).await
    }

    pub async fn git_pull(&self, args: &GitPullArgs) -> Result<GitPullOutput> {
        self.call_tool("git_pull", args).await
    }

    pub async fn git_log(&self, args: &GitLogArgs) -> Result<GitLogOutput> {
        self.call_tool("git_log", args).await
    }

    pub async fn git_diff(&self, args: &GitDiffArgs) -> Result<GitDiffOutput> {
        self.call_tool("git_diff", args).await
    }

    pub async fn git_status(&self, args: &GitStatusArgs) -> Result<GitStatusOutput> {
        self.call_tool("git_status", args).await
    }

    async fn call_tool<T, A>(&self, name: &str, arguments: &A) -> Result<T>
    where
        T: DeserializeOwned,
        A: Serialize + ?Sized,
    {
        let response = self
            .client
            .post(&self.endpoint)
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "tools/call",
                "params": {
                    "name": name,
                    "arguments": arguments,
                },
                "id": 1,
            }))
            .send()
            .await
            .with_context(|| format!("failed to call MCP tool {name}"))?
            .error_for_status()
            .with_context(|| format!("MCP tool {name} returned an error status"))?;

        let value: Value = response.json().await.context("invalid MCP response body")?;
        if let Some(error) = value.get("error") {
            return Err(anyhow!("MCP error: {}", error));
        }

        let result = value
            .get("result")
            .cloned()
            .context("MCP response missing result")?;
        let tool_response: ToolResponse =
            serde_json::from_value(result).context("invalid MCP tool response envelope")?;
        let text = tool_response
            .content
            .first()
            .map(|item| item.text.as_str())
            .context("MCP tool response had no content")?;
        serde_json::from_str(text).with_context(|| format!("invalid payload returned by {name}"))
    }
}
