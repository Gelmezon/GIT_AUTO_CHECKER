use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
    #[serde(default)]
    pub id: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToolCallParams {
    pub name: String,
    #[serde(default)]
    pub arguments: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    pub name: &'static str,
    pub description: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolContent {
    #[serde(rename = "type")]
    pub kind: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResponse {
    pub content: Vec<ToolContent>,
}

impl JsonRpcResponse {
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<Value>, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(error),
        }
    }
}

impl JsonRpcError {
    pub fn method_not_found() -> Self {
        Self {
            code: -32601,
            message: "method not found".to_string(),
        }
    }

    pub fn invalid_params(error: impl std::fmt::Display) -> Self {
        Self {
            code: -32602,
            message: format!("invalid params: {error}"),
        }
    }

    pub fn internal(error: impl std::fmt::Display) -> Self {
        Self {
            code: -32000,
            message: format!("internal error: {error}"),
        }
    }
}

impl ToolDefinition {
    pub fn defaults() -> Vec<Self> {
        vec![
            Self {
                name: "git_clone",
                description: "Clone a git repository to a local path",
            },
            Self {
                name: "git_pull",
                description: "Fetch and fast-forward the current branch",
            },
            Self {
                name: "git_log",
                description: "List recent commits for a repository",
            },
            Self {
                name: "git_diff",
                description: "Produce a patch between two commits",
            },
            Self {
                name: "git_status",
                description: "Inspect the current working tree state",
            },
        ]
    }
}

impl ToolResponse {
    pub fn from_payload(payload: Value) -> Self {
        Self {
            content: vec![ToolContent {
                kind: "text".to_string(),
                text: json!(payload).to_string(),
            }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_success_response() {
        let response = JsonRpcResponse::success(Some(json!(1)), json!({"ok": true}));
        let payload = serde_json::to_value(response).unwrap();
        assert_eq!(payload["jsonrpc"], "2.0");
        assert_eq!(payload["result"]["ok"], true);
    }
}
