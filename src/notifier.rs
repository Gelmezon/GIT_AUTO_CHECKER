use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use tokio::task::JoinSet;
use tracing::{error, info, warn};

use crate::config::{ChannelConfig, NotifierConfig};

#[derive(Debug, Clone)]
pub struct Notification {
    pub task_name: String,
    pub task_type: String,
    pub repo_name: Option<String>,
    pub status: String,
    pub summary: String,
    pub report_path: Option<String>,
    pub duration_secs: u64,
}

#[async_trait]
pub trait Notifier: Send + Sync {
    fn name(&self) -> &str;
    async fn send(&self, notification: &Notification) -> Result<()>;
}

#[derive(Clone, Default)]
pub struct NotifierDispatcher {
    channels: Arc<Vec<Arc<dyn Notifier>>>,
}

impl NotifierDispatcher {
    pub fn from_config(config: &NotifierConfig) -> Result<Self> {
        let mut channels: Vec<Arc<dyn Notifier>> = Vec::new();

        for channel in config.channels.iter().filter(|channel| channel.enabled) {
            match build_notifier(channel)? {
                Some(notifier) => channels.push(notifier),
                None => {
                    warn!(kind = %channel.kind, name = %channel.name, "unknown notifier kind skipped")
                }
            }
        }

        Ok(Self {
            channels: Arc::new(channels),
        })
    }

    pub fn is_enabled(&self) -> bool {
        !self.channels.is_empty()
    }

    pub async fn broadcast(&self, notification: Notification) {
        if self.channels.is_empty() {
            return;
        }

        let mut set = JoinSet::new();
        for notifier in self.channels.iter().cloned() {
            let notification = notification.clone();
            set.spawn(async move {
                let name = notifier.name().to_string();
                let result = notifier.send(&notification).await;
                (name, result)
            });
        }

        while let Some(result) = set.join_next().await {
            match result {
                Ok((name, Ok(()))) => info!(channel = %name, "notification sent"),
                Ok((name, Err(err))) => {
                    error!(channel = %name, error = %err, "notification send failed")
                }
                Err(err) => error!(error = %err, "notification task join failed"),
            }
        }
    }
}

fn build_notifier(channel: &ChannelConfig) -> Result<Option<Arc<dyn Notifier>>> {
    let notifier: Arc<dyn Notifier> = match channel.kind.as_str() {
        "wecom" => Arc::new(WecomNotifier::new(
            channel.name.clone(),
            required_field(channel.webhook_url.clone(), "webhook_url", &channel.name)?,
        )),
        "telegram" => Arc::new(TelegramNotifier::new(
            channel.name.clone(),
            required_field(channel.bot_token.clone(), "bot_token", &channel.name)?,
            required_field(channel.chat_id.clone(), "chat_id", &channel.name)?,
        )),
        "whatsapp" => Arc::new(WhatsAppNotifier::new(
            channel.name.clone(),
            required_field(channel.api_url.clone(), "api_url", &channel.name)?,
            required_field(channel.access_token.clone(), "access_token", &channel.name)?,
            required_field(channel.recipient.clone(), "recipient", &channel.name)?,
        )),
        _ => return Ok(None),
    };

    Ok(Some(notifier))
}

fn required_field(value: Option<String>, field: &str, channel_name: &str) -> Result<String> {
    value.with_context(|| format!("notifier channel {channel_name} requires {field}"))
}

struct WecomNotifier {
    name: String,
    client: Client,
    webhook_url: String,
}

impl WecomNotifier {
    fn new(name: String, webhook_url: String) -> Self {
        Self {
            name,
            client: Client::new(),
            webhook_url,
        }
    }
}

#[async_trait]
impl Notifier for WecomNotifier {
    fn name(&self) -> &str {
        &self.name
    }

    async fn send(&self, notification: &Notification) -> Result<()> {
        let title = notification_title(notification);
        let body = format_notification(notification, false);
        self.client
            .post(&self.webhook_url)
            .json(&json!({
                "msgtype": "markdown",
                "markdown": {
                    "content": format!("**{}**\n\n{}", title, body),
                }
            }))
            .send()
            .await
            .context("wecom request failed")?
            .error_for_status()
            .context("wecom returned error status")?;
        Ok(())
    }
}

struct TelegramNotifier {
    name: String,
    client: Client,
    bot_token: String,
    chat_id: String,
}

impl TelegramNotifier {
    fn new(name: String, bot_token: String, chat_id: String) -> Self {
        Self {
            name,
            client: Client::new(),
            bot_token,
            chat_id,
        }
    }
}

#[async_trait]
impl Notifier for TelegramNotifier {
    fn name(&self) -> &str {
        &self.name
    }

    async fn send(&self, notification: &Notification) -> Result<()> {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token);
        let text = escape_markdown(&format!(
            "{}\n{}",
            notification_title(notification),
            format_notification(notification, true)
        ));
        self.client
            .post(url)
            .json(&json!({
                "chat_id": self.chat_id,
                "text": text,
                "parse_mode": "MarkdownV2",
            }))
            .send()
            .await
            .context("telegram request failed")?
            .error_for_status()
            .context("telegram returned error status")?;
        Ok(())
    }
}

struct WhatsAppNotifier {
    name: String,
    client: Client,
    api_url: String,
    access_token: String,
    recipient: String,
}

impl WhatsAppNotifier {
    fn new(name: String, api_url: String, access_token: String, recipient: String) -> Self {
        Self {
            name,
            client: Client::new(),
            api_url,
            access_token,
            recipient,
        }
    }
}

#[async_trait]
impl Notifier for WhatsAppNotifier {
    fn name(&self) -> &str {
        &self.name
    }

    async fn send(&self, notification: &Notification) -> Result<()> {
        self.client
            .post(&self.api_url)
            .bearer_auth(&self.access_token)
            .json(&json!({
                "messaging_product": "whatsapp",
                "to": self.recipient,
                "type": "text",
                "text": {
                    "body": format!("{}\n{}", notification_title(notification), format_notification(notification, false)),
                }
            }))
            .send()
            .await
            .context("whatsapp request failed")?
            .error_for_status()
            .context("whatsapp returned error status")?;
        Ok(())
    }
}

fn notification_title(notification: &Notification) -> String {
    let mark = if notification.status == "done" {
        "OK"
    } else {
        "FAIL"
    };
    format!(
        "[{}] {} ({})",
        mark, notification.task_name, notification.task_type
    )
}

fn format_notification(notification: &Notification, include_markdown_line_breaks: bool) -> String {
    let newline = if include_markdown_line_breaks {
        "\n\n"
    } else {
        "\n"
    };
    let repo = notification.repo_name.as_deref().unwrap_or("-");
    let report = notification.report_path.as_deref().unwrap_or("-");
    format!(
        "仓库: {repo}{newline}状态: {status}{newline}耗时: {duration}s{newline}报告: {report}{newline}{newline}摘要:{newline}{summary}",
        repo = repo,
        status = notification.status,
        duration = notification.duration_secs,
        report = report,
        summary = truncate_chars(&notification.summary, 1500),
        newline = newline,
    )
}

fn truncate_chars(input: &str, max_chars: usize) -> &str {
    match input.char_indices().nth(max_chars) {
        Some((idx, _)) => &input[..idx],
        None => input,
    }
}

fn escape_markdown(input: &str) -> String {
    let special = [
        '_', '*', '[', ']', '(', ')', '~', '`', '>', '#', '+', '-', '=', '|', '{', '}', '.', '!',
    ];
    let mut output = String::with_capacity(input.len());
    for ch in input.chars() {
        if special.contains(&ch) {
            output.push('\\');
        }
        output.push(ch);
    }
    output
}
