use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, Registry, layer::SubscriberExt};

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub scheduler: SchedulerConfig,
    #[serde(default)]
    pub codex: CodexConfig,
    #[serde(default)]
    pub admin: AdminConfig,
    #[serde(default)]
    pub mcp: McpConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub runtime: RuntimeConfig,
    #[serde(default)]
    pub notifier: NotifierConfig,
    #[serde(default)]
    pub web: WebConfig,
    #[serde(default)]
    pub log: LogConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SchedulerConfig {
    #[serde(default = "default_interval_secs")]
    pub interval_secs: u64,
    #[serde(default = "default_task_timeout_secs")]
    pub task_timeout_secs: u64,
    #[serde(default = "default_max_concurrency")]
    pub max_concurrency: usize,
    #[serde(default = "default_claim_batch_size")]
    pub claim_batch_size: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CodexConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_codex_command")]
    pub command: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_max_retries")]
    pub max_retries: usize,
    #[serde(default = "default_task_timeout_secs")]
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdminConfig {
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub password: String,
    #[serde(default = "default_admin_display_name")]
    pub display_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_database_path")]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeConfig {
    #[serde(default = "default_check_dir")]
    pub check_dir: PathBuf,
    #[serde(default = "default_tests_generated_dir")]
    pub tests_generated_dir: PathBuf,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct NotifierConfig {
    #[serde(default)]
    pub channels: Vec<ChannelConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelConfig {
    pub name: String,
    pub kind: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub webhook_url: Option<String>,
    pub bot_token: Option<String>,
    pub chat_id: Option<String>,
    pub api_url: Option<String>,
    pub access_token: Option<String>,
    pub recipient: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default = "default_log_file")]
    pub file: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WebConfig {
    #[serde(default = "default_jwt_secret")]
    pub jwt_secret: String,
    #[serde(default = "default_token_expire_hours")]
    pub token_expire_hours: u64,
    #[serde(default = "default_static_dir")]
    pub static_dir: PathBuf,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            scheduler: SchedulerConfig::default(),
            codex: CodexConfig::default(),
            admin: AdminConfig::default(),
            mcp: McpConfig::default(),
            database: DatabaseConfig::default(),
            runtime: RuntimeConfig::default(),
            notifier: NotifierConfig::default(),
            web: WebConfig::default(),
            log: LogConfig::default(),
        }
    }
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            interval_secs: default_interval_secs(),
            task_timeout_secs: default_task_timeout_secs(),
            max_concurrency: default_max_concurrency(),
            claim_batch_size: default_claim_batch_size(),
        }
    }
}

impl Default for CodexConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            command: default_codex_command(),
            model: default_model(),
            max_retries: default_max_retries(),
            timeout_secs: default_task_timeout_secs(),
        }
    }
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
        }
    }
}

impl Default for AdminConfig {
    fn default() -> Self {
        Self {
            email: String::new(),
            password: String::new(),
            display_name: default_admin_display_name(),
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: default_database_path(),
        }
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            check_dir: default_check_dir(),
            tests_generated_dir: default_tests_generated_dir(),
        }
    }
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            file: default_log_file(),
        }
    }
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            jwt_secret: default_jwt_secret(),
            token_expire_hours: default_token_expire_hours(),
            static_dir: default_static_dir(),
        }
    }
}

impl AppConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let mut config = if path.exists() {
            let content = fs::read_to_string(path)
                .with_context(|| format!("failed to read config file {}", path.display()))?;
            toml::from_str::<AppConfig>(&content)
                .with_context(|| format!("failed to parse config file {}", path.display()))?
        } else {
            AppConfig::default()
        };

        if let Ok(api_key) = env::var("CODEX_API_KEY") {
            config.codex.api_key = api_key;
        } else if let Ok(api_key) = env::var("OPENAI_API_KEY") {
            config.codex.api_key = api_key;
        }

        Ok(config)
    }

    pub fn init_logging(&self) -> Result<WorkerGuard> {
        if let Some(parent) = self.log.file.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        let file_appender = tracing_appender::rolling::never(
            self.log
                .file
                .parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(".")),
            self.log
                .file
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("git-helper.log"),
        );
        let (writer, guard) = tracing_appender::non_blocking(file_appender);
        let filter =
            EnvFilter::try_new(self.log.level.clone()).unwrap_or_else(|_| EnvFilter::new("info"));

        let subscriber = Registry::default()
            .with(filter)
            .with(tracing_subscriber::fmt::layer().with_writer(std::io::stdout))
            .with(
                tracing_subscriber::fmt::layer()
                    .with_ansi(false)
                    .with_writer(writer),
            );

        tracing::subscriber::set_global_default(subscriber)
            .context("failed to initialize tracing subscriber")?;

        Ok(guard)
    }

    pub fn scheduler_interval(&self) -> Duration {
        Duration::from_secs(self.scheduler.interval_secs)
    }

    pub fn task_timeout(&self) -> Duration {
        Duration::from_secs(self.scheduler.task_timeout_secs)
    }

    pub fn mcp_base_url(&self) -> String {
        format!("http://{}:{}", self.mcp.host, self.mcp.port)
    }
}

impl AdminConfig {
    pub fn is_configured(&self) -> bool {
        !self.email.trim().is_empty() && !self.password.is_empty()
    }
}

fn default_interval_secs() -> u64 {
    1
}

fn default_task_timeout_secs() -> u64 {
    300
}

fn default_max_concurrency() -> usize {
    4
}

fn default_claim_batch_size() -> usize {
    16
}

fn default_model() -> String {
    "gpt-5.4".to_string()
}

fn default_codex_command() -> String {
    "codex".to_string()
}

fn default_max_retries() -> usize {
    2
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_admin_display_name() -> String {
    "Super Admin".to_string()
}

fn default_port() -> u16 {
    3100
}

fn default_database_path() -> PathBuf {
    PathBuf::from("data/scheduler.db")
}

fn default_check_dir() -> PathBuf {
    PathBuf::from("check")
}

fn default_tests_generated_dir() -> PathBuf {
    PathBuf::from("tests-generated")
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_file() -> PathBuf {
    PathBuf::from("logs/git-helper.log")
}

fn default_true() -> bool {
    true
}

fn default_jwt_secret() -> String {
    "change-me-before-production".to_string()
}

fn default_token_expire_hours() -> u64 {
    168
}

fn default_static_dir() -> PathBuf {
    PathBuf::from("web/dist")
}
