use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use tokio::time::sleep;

use crate::config::CodexConfig;

#[derive(Debug, Clone)]
pub struct CodexExecutor {
    client: Client,
    config: CodexConfig,
}

impl CodexExecutor {
    pub fn new(config: CodexConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .context("failed to build reqwest client")?;

        Ok(Self { client, config })
    }

    pub async fn execute(&self, prompt: &str) -> Result<String> {
        if self.config.api_key.trim().is_empty() {
            return Err(anyhow!("OPENAI_API_KEY is not configured"));
        }

        let attempts = self.config.max_retries + 1;
        let mut last_error = None;
        for attempt in 0..attempts {
            match self.execute_once(prompt).await {
                Ok(output) => return Ok(output),
                Err(error) => {
                    last_error = Some(error);
                    if attempt + 1 < attempts {
                        sleep(Duration::from_secs(1 << attempt)).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("codex call failed without error")))
    }

    async fn execute_once(&self, prompt: &str) -> Result<String> {
        let response = self
            .client
            .post(&self.config.response_url)
            .bearer_auth(&self.config.api_key)
            .json(&json!({
                "model": self.config.model,
                "input": prompt,
            }))
            .send()
            .await
            .context("failed to call responses api")?
            .error_for_status()
            .context("responses api returned an error status")?;

        let payload: ResponsesApiResponse = response.json().await.context("invalid json body")?;
        payload.output_text()
    }
}

#[derive(Debug, Deserialize)]
struct ResponsesApiResponse {
    #[serde(default)]
    output: Vec<ResponseOutput>,
}

#[derive(Debug, Deserialize)]
struct ResponseOutput {
    #[serde(default)]
    content: Vec<ResponseContent>,
}

#[derive(Debug, Deserialize)]
struct ResponseContent {
    #[serde(default)]
    r#type: String,
    #[serde(default)]
    text: String,
}

impl ResponsesApiResponse {
    fn output_text(&self) -> Result<String> {
        let joined = self
            .output
            .iter()
            .flat_map(|item| item.content.iter())
            .filter(|content| content.r#type == "output_text" || content.r#type.is_empty())
            .map(|content| content.text.as_str())
            .collect::<Vec<_>>()
            .join("");

        if joined.trim().is_empty() {
            Err(anyhow!("responses api returned no output text"))
        } else {
            Ok(joined)
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::{Json, Router, routing::post};
    use serde_json::{Value, json};
    use tokio::net::TcpListener;

    use super::*;
    use crate::config::CodexConfig;

    #[tokio::test]
    async fn executor_returns_output_text() {
        let app = Router::new().route(
            "/responses",
            post(|| async {
                Json(json!({
                    "output": [{
                        "content": [{
                            "type": "output_text",
                            "text": "review result"
                        }]
                    }]
                }))
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let executor = CodexExecutor::new(CodexConfig {
            api_key: "test-key".to_string(),
            model: "gpt-5.4".to_string(),
            max_retries: 0,
            timeout_secs: 5,
            response_url: format!("http://{address}/responses"),
        })
        .unwrap();

        let output = executor.execute("hello").await.unwrap();
        assert_eq!(output, "review result");
    }

    #[tokio::test]
    async fn executor_sends_prompt_payload() {
        let app = Router::new().route(
            "/responses",
            post(|Json(payload): Json<Value>| async move {
                assert_eq!(payload["input"], "payload");
                Json(json!({
                    "output": [{
                        "content": [{
                            "type": "output_text",
                            "text": "ok"
                        }]
                    }]
                }))
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let executor = CodexExecutor::new(CodexConfig {
            api_key: "test-key".to_string(),
            model: "gpt-5.4".to_string(),
            max_retries: 0,
            timeout_secs: 5,
            response_url: format!("http://{address}/responses"),
        })
        .unwrap();

        let output = executor.execute("payload").await.unwrap();
        assert_eq!(output, "ok");
    }
}
