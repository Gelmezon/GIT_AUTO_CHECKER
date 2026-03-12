pub mod client;
pub mod protocol;
pub mod tools;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::{Json, Router, extract::State, routing::get, routing::post};
use serde_json::json;
use tracing::info;

use crate::config::AppConfig;
use crate::db::Database;
use crate::mcp::protocol::{
    JsonRpcError, JsonRpcRequest, JsonRpcResponse, ToolCallParams, ToolDefinition, ToolResponse,
};
use crate::mcp::tools::git::{
    GitCloneArgs, GitDiffArgs, GitLogArgs, GitPullArgs, GitStatusArgs, diff_repo, git_clone,
    git_log, git_pull, git_status,
};

#[derive(Clone)]
struct McpState {
    config: Arc<AppConfig>,
    #[allow(dead_code)]
    database: Database,
}

pub async fn serve(config: Arc<AppConfig>, database: Database) -> Result<()> {
    let state = McpState { config, database };
    let app = Router::new()
        .route("/health", get(health))
        .route("/mcp", post(handle_mcp))
        .with_state(state.clone());

    let address: SocketAddr = format!("{}:{}", state.config.mcp.host, state.config.mcp.port)
        .parse()
        .context("invalid MCP bind address")?;
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .with_context(|| format!("failed to bind MCP server on {address}"))?;

    info!(%address, "mcp server listening");
    axum::serve(listener, app)
        .await
        .context("mcp server failed")
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}

async fn handle_mcp(
    State(_state): State<McpState>,
    Json(request): Json<JsonRpcRequest>,
) -> Json<JsonRpcResponse> {
    let id = request.id.clone();
    let response = match request.method.as_str() {
        "tools/list" => JsonRpcResponse::success(
            id,
            json!({
                "tools": ToolDefinition::defaults(),
            }),
        ),
        "tools/call" => {
            let params = serde_json::from_value::<ToolCallParams>(
                request.params.unwrap_or_else(|| json!({})),
            );
            match params {
                Ok(params) => call_tool(params)
                    .map(|value| JsonRpcResponse::success(id.clone(), value))
                    .unwrap_or_else(|error| {
                        JsonRpcResponse::error(id, JsonRpcError::internal(error))
                    }),
                Err(error) => JsonRpcResponse::error(id, JsonRpcError::invalid_params(error)),
            }
        }
        _ => JsonRpcResponse::error(id, JsonRpcError::method_not_found()),
    };

    Json(response)
}

fn call_tool(params: ToolCallParams) -> Result<serde_json::Value> {
    let output = match params.name.as_str() {
        "git_clone" => {
            let args: GitCloneArgs = serde_json::from_value(params.arguments)?;
            serde_json::to_value(git_clone(&args)?)?
        }
        "git_pull" => {
            let args: GitPullArgs = serde_json::from_value(params.arguments)?;
            serde_json::to_value(git_pull(&args)?)?
        }
        "git_log" => {
            let args: GitLogArgs = serde_json::from_value(params.arguments)?;
            serde_json::to_value(git_log(&args)?)?
        }
        "git_diff" => {
            let args: GitDiffArgs = serde_json::from_value(params.arguments)?;
            serde_json::to_value(diff_repo(&args)?)?
        }
        "git_status" => {
            let args: GitStatusArgs = serde_json::from_value(params.arguments)?;
            serde_json::to_value(git_status(&args)?)?
        }
        other => anyhow::bail!("unsupported MCP tool: {other}"),
    };

    Ok(serde_json::to_value(ToolResponse::from_payload(output))?)
}
