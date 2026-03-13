# AI 调度系统 — 项目设计文档

## 一、项目核心需求

构建一个以 **定时器 + 数据库** 为核心的 AI 调度系统，具备以下能力：

1. **定时扫描引擎** — 每 1 秒扫描数据库，发现到期任务并派发执行。
2. **Prompt 数据库** — 持久化存储待执行的 AI Prompt、调度时间、执行状态等元数据。
3. **Codex 执行器** — 扫描到到期任务后，调用 OpenAI Codex（模型 `gpt-5.4`）执行 Prompt。
4. **Git 管理 MCP 服务** — 以 MCP（Model Context Protocol）协议暴露 Git 操作能力，在本机监听端口，供调度系统及其他 AI 工具调用。
5. **机器人通知** — 任务执行完成后，通过企业微信 / Telegram / WhatsApp 等渠道推送审核报告摘要和结果通知。
6. **首个业务功能：定时 Git 审核** — 定时拉取配置的 Git 项目，使用 Codex 进行代码审核，将审核报告输出到 `check/` 目录，并通过机器人推送通知。

---

## 二、项目目的

- 将重复性的 AI 任务（代码审核、文档生成、质量检查等）从手动触发转为 **自动化定时调度**。
- 通过 MCP 协议标准化 Git 操作，使调度系统与 Git 管理解耦，同时对外暴露能力供其他 AI 工具复用。
- 以「定时拉取 + Codex 审核」作为第一个端到端功能验证整体架构。

---

## 三、原设计评估与改进

| 问题 | 改进 |
|------|------|
| `tasks` 表缺少任务类型字段，无法区分审核 / 生成 / 其他任务 | 增加 `task_type` 字段（枚举：`git_review` / `custom`） |
| `tasks` 与 `git_repos` 无关联，审核任务找不到目标仓库 | 增加 `repo_id` 外键，审核任务关联到具体仓库 |
| `cron_expr` 与 `scheduled_at` 关系不清晰 | 明确：`scheduled_at` 由 `cron_expr` 计算得出，任务完成后重新计算下次时间并插入新记录 |
| 缺少故障恢复的超时判定 | 增加 `started_at` 字段，进程启动时将超过 5 分钟的 `running` 任务回退为 `pending` |
| 缺少配置管理 | 增加 `config.toml` 统一管理 API Key、端口、扫描间隔、日志级别 |
| 缺少日志系统 | 使用 `tracing` 结构化日志，输出到终端和文件 |

---

## 四、系统架构

```
┌──────────────────────────────────────────────────────────────────────────┐
│                            git-helper 进程                                │
│                                                                          │
│  ┌─────────────┐   1s tick   ┌──────────────┐                           │
│  │  Scheduler   │ ─────────▸ │  SQLite DB    │                           │
│  │  (tokio)     │            │  tasks        │                           │
│  └──────┬──────┘            │  git_repos    │                           │
│         │                    │  bot_channels │                           │
│         │ spawn task         └──────┬───────┘                           │
│         ▼                           ▼                                    │
│  ┌─────────────┐            ┌──────────────┐   ┌────────────────┐       │
│  │  Dispatcher  │ ─────────▸│ Codex 执行器  │──▸│ check/ 报告    │       │
│  │  (并发控制)  │            │ (gpt-5.4)    │   └───────┬────────┘       │
│  └─────────────┘            └──────────────┘           │                │
│                                                         ▼                │
│  ┌──────────────────────────────────────┐   ┌──────────────────────┐    │
│  │  Git MCP Server (axum, :3100)        │   │  Notifier (通知分发)  │    │
│  │  Tools: clone/pull/log/diff/status   │   │  ├─ 企业微信 Webhook  │    │
│  └──────────────────────────────────────┘   │  ├─ Telegram Bot API │    │
│                                              │  └─ WhatsApp Cloud   │    │
│                                              └──────────────────────┘    │
└──────────────────────────────────────────────────────────────────────────┘
        ▲ HTTP/SSE                                       │ HTTPS
        │                                                ▼
   外部 AI 工具 / curl / MCP Inspector          企业微信 / Telegram / WhatsApp
```

**关键决策**：Scheduler、Dispatcher、MCP Server、Notifier 运行在同一进程的不同 tokio task 中，共享数据库连接池。单进程部署简单，SQLite 通过 WAL 模式支持并发读。通知发送异步化，不阻塞任务主流程。

---

## 五、模块设计

### 5.1 配置管理（`config.toml`）

```toml
[scheduler]
interval_secs = 1
task_timeout_secs = 300        # running 超时阈值

[codex]
# api_key = "sk-..."           # 可选；留空时直接使用本机 `codex login`
model = "gpt-5.4"
max_retries = 2
timeout_secs = 300

[mcp]
host = "127.0.0.1"
port = 3100

[log]
level = "info"                 # trace / debug / info / warn / error
file = "logs/git-helper.log"

# ── 机器人通知（可配置多个渠道，按需启用） ──

[[notifier.channels]]
name = "dev-team-wecom"
kind = "wecom"                 # 企业微信
enabled = true
webhook_url = "https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=YOUR_KEY"

[[notifier.channels]]
name = "dev-team-telegram"
kind = "telegram"
enabled = false
bot_token = "123456:ABC-DEF..."
chat_id = "-1001234567890"     # 群组 ID（负数）或用户 ID

[[notifier.channels]]
name = "dev-team-whatsapp"
kind = "whatsapp"
enabled = false
api_url = "https://graph.facebook.com/v21.0/PHONE_NUMBER_ID/messages"
access_token = "EAAG..."
recipient = "8613800138000"    # 接收方手机号（含国际区号）
```

### 5.2 定时器引擎（Scheduler）

| 项目 | 说明 |
|------|------|
| 扫描间隔 | 1 秒（可配置） |
| 实现方式 | `tokio::time::interval(Duration::from_secs(1))` |
| 职责 | 每秒查询 `status = 'pending' AND scheduled_at <= now()` |
| 并发控制 | SQLite 事务内 `UPDATE ... SET status='running' WHERE status='pending'`，利用数据库锁保证原子性 |
| 故障恢复 | 启动时扫描 `status='running' AND started_at < now() - timeout`，回退为 `pending` |

### 5.3 数据库（SQLite + WAL）

使用 SQLite WAL 模式，支持一写多读。通过 `rusqlite` 同步访问，用 `tokio::task::spawn_blocking` 避免阻塞异步运行时。

#### 核心表：`tasks`

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER PK | 自增主键 |
| name | TEXT NOT NULL | 任务名称 |
| task_type | TEXT NOT NULL | 任务类型：`git_review` / `custom` |
| repo_id | INTEGER | 关联 `git_repos.id`，仅审核任务需要 |
| prompt | TEXT NOT NULL | 发送给 Codex 的完整 Prompt |
| cron_expr | TEXT | Cron 表达式（周期任务），NULL 表示一次性任务 |
| scheduled_at | DATETIME NOT NULL | 下次执行时间（由 cron_expr 计算或手动指定） |
| started_at | DATETIME | 开始执行时间（用于超时判定） |
| status | TEXT NOT NULL DEFAULT 'pending' | `pending` → `running` → `done` / `failed` |
| result | TEXT | 执行结果或错误信息 |
| retry_count | INTEGER DEFAULT 0 | 已重试次数 |
| created_at | DATETIME DEFAULT CURRENT_TIMESTAMP | 创建时间 |
| updated_at | DATETIME DEFAULT CURRENT_TIMESTAMP | 最后更新时间 |

**状态流转**：
```
pending ──▸ running ──▸ done
                │
                ├──▸ failed (重试耗尽)
                │
                └──▸ pending (超时回退 / 重试)
```

**周期任务处理**：任务完成后（done/failed），若 `cron_expr` 非空，则根据 cron 计算下次 `scheduled_at`，插入新的 `pending` 记录。

#### 核心表：`git_repos`

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER PK | 自增主键 |
| name | TEXT NOT NULL | 项目名称 |
| repo_url | TEXT NOT NULL | Git 仓库地址 |
| branch | TEXT DEFAULT 'main' | 跟踪分支 |
| local_path | TEXT NOT NULL | 本地克隆路径 |
| review_cron | TEXT | 审核调度 Cron 表达式 |
| last_commit | TEXT | 上次审核时的 commit hash |
| enabled | BOOLEAN DEFAULT 1 | 是否启用 |

### 5.4 Codex 执行器

#### 调用方式选型

| 方案 | 说明 | 适合场景 | 本项目适用性 |
|------|------|---------|-------------|
| **`codex exec`（CLI 非交互模式）** | 通过 `std::process::Command` 调用 Codex CLI | 单次任务下发、脚本自动化 | **首选方案** |
| `codex --as-mcp-server`（MCP 模式） | Codex 作为 MCP Server，通过 stdio 协议通信 | 需要多轮对话、上下文保持 | 备选（审核任务不需要多轮） |
| Codex TypeScript SDK | `@openai/codex-sdk`，Node.js 环境 | Node.js/TS 项目 | 不适用（本项目为 Rust） |
| OpenAI REST API 直调 | `reqwest` 调用 `POST /v1/responses` | 需要精细控制请求参数 | 备选（缺少 Codex 的沙箱和工具链能力） |

#### 首选方案：`codex exec` 非交互模式

##### 基本原理

`codex exec` 是 Codex CLI 的非交互子命令（别名 `codex e`），运行时：
- **stderr** 输出执行进度流
- **stdout** 仅输出最终结果（agent 的最后一条消息）
- 支持 `--json` 输出 JSONL 格式的结构化事件流
- 默认在只读沙箱中运行，安全可控

##### 核心命令格式

```bash
# 基本调用
codex exec "审查以下代码的质量问题" \
  --model gpt-5.4 \
  --approval-mode never

# 指定工作目录（让 Codex 读取目标仓库）
codex exec "审查最近的代码变更，输出中文报告" \
  --model gpt-5.4 \
  --approval-mode never \
  --path /repos/my-project

# JSON 结构化输出（推荐，便于 Rust 解析）
codex exec "review code quality" \
  --model gpt-5.4 \
  --approval-mode never \
  --json

# 输出 pipe 到文件
codex exec "generate review report" \
  --model gpt-5.4 \
  --approval-mode never \
  | tee check/repo-name/2026-03-12-review.md
```

##### 关键参数说明

| 参数 | 说明 | 本项目取值 |
|------|------|-----------|
| `--model` | 指定模型 | `gpt-5.4`（配置于 `config.toml`） |
| `--approval-mode` | 审批模式 | `never`（全自动，无需人工确认） |
| `--path` | 工作目录 | 目标仓库的 `local_path` |
| `--json` | JSONL 输出 | 启用，便于解析结构化结果 |
| `--timeout` | 执行超时 | 由 Rust 侧 `tokio::time::timeout` 控制 |

##### Rust 调用封装

```rust
use std::process::Command;
use tokio::task::spawn_blocking;

pub struct CodexExecutor {
    model: String,           // "gpt-5.4"
    timeout_secs: u64,       // 300
    max_retries: u32,        // 2
}

impl CodexExecutor {
    /// 调用 codex exec 执行任务
    pub async fn execute(&self, prompt: &str, work_dir: &str) -> Result<String> {
        let model = self.model.clone();
        let prompt = prompt.to_string();
        let work_dir = work_dir.to_string();

        // codex exec 是同步阻塞的，放到 blocking 线程池
        let output = spawn_blocking(move || {
            Command::new("codex")
                .args([
                    "exec",
                    "--model", &model,
                    "--approval-mode", "never",
                    "--path", &work_dir,
                    &prompt,
                ])
                .output()
        }).await??;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow!("Codex 执行失败: {}", stderr))
        }
    }

    /// 带重试的执行
    pub async fn execute_with_retry(&self, prompt: &str, work_dir: &str) -> Result<String> {
        let mut last_err = None;
        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                let delay = Duration::from_secs(1 << attempt); // 2s, 4s
                tokio::time::sleep(delay).await;
            }
            match self.execute(prompt, work_dir).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    tracing::warn!(attempt, error = %e, "Codex 调用失败，准备重试");
                    last_err = Some(e);
                }
            }
        }
        Err(last_err.unwrap())
    }
}
```

##### JSON 输出解析（`--json` 模式）

`codex exec --json` 输出 JSONL 事件流，每行一个 JSON 对象：

```jsonl
{"type": "thread.started", "thread_id": "thread_abc123"}
{"type": "turn.started"}
{"type": "item.agent_message", "content": "以下是代码审查报告..."}
{"type": "item.command_execution", "command": "cat src/main.rs", "exit_code": 0}
{"type": "item.file_change", "path": "report.md", "action": "created"}
{"type": "turn.completed", "usage": {"input_tokens": 1200, "output_tokens": 800}}
```

Rust 解析关键事件：

```rust
#[derive(Deserialize)]
#[serde(tag = "type")]
enum CodexEvent {
    #[serde(rename = "item.agent_message")]
    AgentMessage { content: String },
    #[serde(rename = "turn.completed")]
    TurnCompleted { usage: Option<TokenUsage> },
    #[serde(rename = "turn.failed")]
    TurnFailed { error: String },
    // 其他事件忽略
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct TokenUsage {
    input_tokens: u32,
    output_tokens: u32,
}
```

##### 环境变量与认证

```bash
# 推荐：直接使用本机 Codex 登录态
codex login

# 可选：如果你希望显式传 key，也支持下面两种环境变量
export CODEX_API_KEY="sk-..."
export OPENAI_API_KEY="sk-..."
```

Rust 侧在启动时检查：

```rust
fn validate_codex_env() -> Result<()> {
    // 无需强制 API Key；允许直接复用本机 `codex login`
    Ok(())
}
```

#### 执行器总览

| 项目 | 说明 |
|------|------|
| 模型 | `gpt-5.4`（通过 `--model` 指定） |
| 调用方式 | `std::process::Command` → `codex exec`（非交互 CLI） |
| 输入 | 从 `tasks.prompt` 读取，作为 `codex exec` 的位置参数 |
| 工作目录 | 通过 `--path` 指定为目标仓库的 `local_path` |
| 输出解析 | `--json` JSONL 事件流，提取 `item.agent_message` |
| 结果存储 | 写回 `tasks.result`，审核任务同时写入 `check/` 目录 |
| 超时 | `tokio::time::timeout(Duration::from_secs(300))` 包裹整个调用 |
| 重试 | 失败后最多重试 2 次，指数退避（2s → 4s） |
| 认证 | 优先复用本机 `codex login`；也兼容 `CODEX_API_KEY` / `OPENAI_API_KEY` |

### 5.5 Git MCP 服务

以 MCP 协议暴露 Git 操作，默认监听 `127.0.0.1:3100`。

#### 安全原则：只读操作

> **重点：MCP 服务严禁暴露任何写操作。不提供 commit、push、merge、rebase、reset 等修改仓库的能力。**
> 系统定位是「拉取 + 审查 + 出报告」，所有改进建议以文档形式输出到 `check/` 目录，**绝不自动修改源码**。

#### 暴露的 Tool 列表（仅只读操作）

| Tool 名称 | 参数 | 说明 |
|-----------|------|------|
| `git_clone` | `url`, `path`, `branch?` | 克隆仓库到指定路径（首次获取） |
| `git_pull` | `path` | 拉取指定仓库的最新代码 |
| `git_log` | `path`, `count?`, `since?` | 获取 commit 记录 |
| `git_diff` | `path`, `from`, `to?` | 获取两个 commit 之间的 diff |
| `git_status` | `path` | 获取仓库工作区状态 |

#### 明确禁止的操作

| 操作 | 原因 |
|------|------|
| `commit` / `add` | 系统不产生代码变更，无需暂存或提交 |
| `push` | 禁止向远程写入任何内容 |
| `checkout` / `switch` | 避免切换分支导致工作区状态不一致 |
| `merge` / `rebase` / `reset` | 禁止修改仓库历史 |

#### MCP 传输方式

- **Streamable HTTP**：`POST http://127.0.0.1:3100/mcp`
- 基于 `axum` 实现 JSON-RPC over HTTP
- SSE 端点用于流式返回大 diff 结果

#### 内部调用

调度系统通过 MCP 客户端（`reqwest`）调用本机 MCP 服务。虽然同进程，但走 HTTP 保持架构一致性，且方便未来拆分为独立服务。

### 5.6 首个功能：定时 Git 审核

#### 执行流程

```
1. Scheduler 发现到期的 git_review 类型任务
2. 通过 repo_id 查询 git_repos，获取仓库信息
3. 调用 Git MCP → git_pull 拉取最新代码
4. 调用 Git MCP → git_diff(from=last_commit, to=HEAD) 获取变更
5. 若无新 commit → 标记 done，跳过审核
6. 组装 Prompt：diff 内容 + 审核指令模板
7. 调用 Codex (gpt-5.4) 执行审核
8. 将审核报告写入 check/{repo_name}/{date}-review.md
9. 更新 git_repos.last_commit = HEAD
10. 标记 tasks.status = done
11. 若 cron_expr 非空，计算下次时间并创建新 pending 任务
```

#### 审核报告格式（`check/` 目录）

```
check/
  └── {repo_name}/
      └── 2026-03-12-review.md
```

每份报告包含：
- 审核时间、commit 范围（from..to）
- 变更文件列表及统计
- 代码质量评估
- 潜在 Bug 与逻辑问题
- 安全风险提示
- 改进建议

### 5.7 机器人通知模块（Notifier）

任务执行完成后，将审核报告摘要推送到企业微信 / Telegram / WhatsApp 等渠道。

#### 设计原则

- **异步非阻塞** — 通知发送在独立 tokio task 中执行，不阻塞任务主流程
- **多渠道广播** — 同一事件可同时推送到多个已启用的渠道
- **发送失败不影响任务状态** — 通知失败仅记录日志告警，不回退任务状态
- **统一 trait 抽象** — 新增渠道只需实现 `Notifier` trait，无需修改调度逻辑

#### 渠道对比

| 渠道 | 协议 | 认证方式 | 消息格式 | 适合场景 |
|------|------|---------|---------|---------|
| 企业微信 | Webhook (HTTPS POST) | URL 中的 `key` 参数 | Markdown | 国内团队、无需申请 Bot |
| Telegram | Bot API (HTTPS POST) | `bot_token` | Markdown / HTML | 海外团队、功能丰富 |
| WhatsApp | Cloud API (HTTPS POST) | Bearer Token | 模板消息 / 文本 | 商务场景、触达客户 |

#### 核心 Trait 定义

```rust
use async_trait::async_trait;

/// 通知事件，由 Dispatcher 在任务完成后构建
#[derive(Debug, Clone)]
pub struct Notification {
    pub task_name: String,
    pub task_type: String,           // "git_review" / "custom"
    pub repo_name: Option<String>,
    pub status: String,              // "done" / "failed"
    pub summary: String,             // 报告摘要（前 500 字）
    pub report_path: Option<String>, // "check/my-project/2026-03-12-review.md"
    pub duration_secs: u64,          // 任务耗时
}

/// 所有通知渠道实现此 trait
#[async_trait]
pub trait Notifier: Send + Sync {
    /// 渠道名称（用于日志标识）
    fn name(&self) -> &str;

    /// 发送通知，返回 Ok(()) 或错误
    async fn send(&self, notification: &Notification) -> Result<()>;
}
```

#### 企业微信实现

```rust
pub struct WecomNotifier {
    name: String,
    client: Client,
    webhook_url: String,
}

impl WecomNotifier {
    pub fn new(name: String, webhook_url: String) -> Self {
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

    async fn send(&self, n: &Notification) -> Result<()> {
        let emoji = if n.status == "done" { "✅" } else { "❌" };
        let repo_line = n.repo_name.as_deref().unwrap_or("-");

        let markdown_content = format!(
            "{emoji} **{task_name}**\n\
             > 仓库: {repo}\n\
             > 状态: {status}\n\
             > 耗时: {duration}s\n\n\
             **摘要**\n{summary}",
            emoji = emoji,
            task_name = n.task_name,
            repo = repo_line,
            status = n.status,
            duration = n.duration_secs,
            summary = truncate(&n.summary, 2000),
        );

        let body = json!({
            "msgtype": "markdown",
            "markdown": {
                "content": markdown_content
            }
        });

        self.client
            .post(&self.webhook_url)
            .json(&body)
            .send()
            .await
            .context("wecom webhook request failed")?
            .error_for_status()
            .context("wecom webhook returned error")?;

        Ok(())
    }
}
```

#### Telegram 实现

```rust
pub struct TelegramNotifier {
    name: String,
    client: Client,
    bot_token: String,
    chat_id: String,
}

impl TelegramNotifier {
    pub fn new(name: String, bot_token: String, chat_id: String) -> Self {
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

    async fn send(&self, n: &Notification) -> Result<()> {
        let emoji = if n.status == "done" { "✅" } else { "❌" };
        let repo_line = n.repo_name.as_deref().unwrap_or("\\-");

        let text = format!(
            "{emoji} *{task_name}*\n\
             仓库: `{repo}`\n\
             状态: {status}\n\
             耗时: {duration}s\n\n\
             *摘要*\n{summary}",
            emoji = emoji,
            task_name = escape_markdown(&n.task_name),
            repo = repo_line,
            status = n.status,
            duration = n.duration_secs,
            summary = truncate(&escape_markdown(&n.summary), 3000),
        );

        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            self.bot_token
        );

        self.client
            .post(&url)
            .json(&json!({
                "chat_id": self.chat_id,
                "text": text,
                "parse_mode": "MarkdownV2",
            }))
            .send()
            .await
            .context("telegram api request failed")?
            .error_for_status()
            .context("telegram api returned error")?;

        Ok(())
    }
}

/// Telegram MarkdownV2 要求转义特殊字符
fn escape_markdown(s: &str) -> String {
    let special = ['_', '*', '[', ']', '(', ')', '~', '`', '>', '#', '+', '-', '=', '|', '{', '}', '.', '!'];
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        if special.contains(&c) {
            result.push('\\');
        }
        result.push(c);
    }
    result
}
```

#### WhatsApp 实现

```rust
pub struct WhatsAppNotifier {
    name: String,
    client: Client,
    api_url: String,
    access_token: String,
    recipient: String,
}

impl WhatsAppNotifier {
    pub fn new(
        name: String,
        api_url: String,
        access_token: String,
        recipient: String,
    ) -> Self {
        Self { name, client: Client::new(), api_url, access_token, recipient }
    }
}

#[async_trait]
impl Notifier for WhatsAppNotifier {
    fn name(&self) -> &str {
        &self.name
    }

    async fn send(&self, n: &Notification) -> Result<()> {
        let emoji = if n.status == "done" { "✅" } else { "❌" };
        let repo_line = n.repo_name.as_deref().unwrap_or("-");

        let text = format!(
            "{emoji} {task_name}\n\
             仓库: {repo}\n\
             状态: {status}\n\
             耗时: {duration}s\n\n\
             摘要:\n{summary}",
            emoji = emoji,
            task_name = n.task_name,
            repo = repo_line,
            status = n.status,
            duration = n.duration_secs,
            summary = truncate(&n.summary, 1500),
        );

        self.client
            .post(&self.api_url)
            .bearer_auth(&self.access_token)
            .json(&json!({
                "messaging_product": "whatsapp",
                "to": self.recipient,
                "type": "text",
                "text": { "body": text }
            }))
            .send()
            .await
            .context("whatsapp api request failed")?
            .error_for_status()
            .context("whatsapp api returned error")?;

        Ok(())
    }
}
```

#### 通知分发器

```rust
/// 管理所有启用的通知渠道，统一广播
pub struct NotifierDispatcher {
    channels: Vec<Box<dyn Notifier>>,
}

impl NotifierDispatcher {
    /// 从配置构建，仅加载 enabled = true 的渠道
    pub fn from_config(config: &[ChannelConfig]) -> Result<Self> {
        let mut channels: Vec<Box<dyn Notifier>> = Vec::new();

        for ch in config.iter().filter(|c| c.enabled) {
            let notifier: Box<dyn Notifier> = match ch.kind.as_str() {
                "wecom" => Box::new(WecomNotifier::new(
                    ch.name.clone(),
                    ch.webhook_url.clone().context("wecom requires webhook_url")?,
                )),
                "telegram" => Box::new(TelegramNotifier::new(
                    ch.name.clone(),
                    ch.bot_token.clone().context("telegram requires bot_token")?,
                    ch.chat_id.clone().context("telegram requires chat_id")?,
                )),
                "whatsapp" => Box::new(WhatsAppNotifier::new(
                    ch.name.clone(),
                    ch.api_url.clone().context("whatsapp requires api_url")?,
                    ch.access_token.clone().context("whatsapp requires access_token")?,
                    ch.recipient.clone().context("whatsapp requires recipient")?,
                )),
                other => {
                    tracing::warn!(kind = other, "unknown notifier kind, skipping");
                    continue;
                }
            };
            channels.push(notifier);
        }

        Ok(Self { channels })
    }

    /// 向所有渠道广播通知（并发发送，失败仅记录日志）
    pub async fn broadcast(&self, notification: &Notification) {
        let futures: Vec<_> = self.channels.iter().map(|ch| {
            let n = notification.clone();
            let name = ch.name().to_string();
            async move {
                if let Err(e) = ch.send(&n).await {
                    tracing::error!(
                        channel = %name,
                        error = %e,
                        "notification send failed"
                    );
                } else {
                    tracing::info!(channel = %name, "notification sent");
                }
            }
        }).collect();

        futures::future::join_all(futures).await;
    }
}
```

#### 通知配置结构体

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct NotifierConfig {
    #[serde(default)]
    pub channels: Vec<ChannelConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelConfig {
    pub name: String,
    pub kind: String,           // "wecom" / "telegram" / "whatsapp"
    #[serde(default = "default_true")]
    pub enabled: bool,

    // 企业微信
    pub webhook_url: Option<String>,

    // Telegram
    pub bot_token: Option<String>,
    pub chat_id: Option<String>,

    // WhatsApp
    pub api_url: Option<String>,
    pub access_token: Option<String>,
    pub recipient: Option<String>,
}

fn default_true() -> bool { true }
```

#### 在 Dispatcher 中集成

```rust
// dispatcher.rs — 任务完成后触发通知
async fn on_task_complete(task: &Task, result: &str, notifier: &NotifierDispatcher) {
    let notification = Notification {
        task_name: task.name.clone(),
        task_type: task.task_type.clone(),
        repo_name: task.repo_name.clone(),
        status: task.status.clone(),
        summary: truncate(result, 500).to_string(),
        report_path: task.report_path.clone(),
        duration_secs: task.duration_secs(),
    };

    // 异步发送，不阻塞主流程
    let notifier = notifier.clone();
    tokio::spawn(async move {
        notifier.broadcast(&notification).await;
    });
}
```

#### 辅助函数

```rust
fn truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}
```

#### 各渠道 API 参考

| 渠道 | API 文档 | 要点 |
|------|---------|------|
| 企业微信 | [群机器人配置说明](https://developer.work.weixin.qq.com/document/path/91770) | Webhook URL 直接 POST，无需 access_token 刷新 |
| Telegram | [Bot API sendMessage](https://core.telegram.org/bots/api#sendmessage) | 需先通过 @BotFather 创建 Bot，获取 `bot_token` |
| WhatsApp | [Cloud API Messages](https://developers.facebook.com/docs/whatsapp/cloud-api/messages) | 需 Meta Business 账号，配置 WhatsApp Business Platform |

---

## 六、技术选型（Rust）

| 组件 | crate | 理由 |
|------|-------|------|
| 异步运行时 | `tokio` | Rust 异步生态标准，支持定时器、并发任务 |
| 数据库 | `rusqlite` + `tokio::task::spawn_blocking` | 成熟的 SQLite 绑定，WAL 模式支持并发读 |
| HTTP 服务 (MCP) | `axum` | 轻量、与 tokio 深度集成 |
| HTTP 客户端 | `reqwest` | 调用 OpenAI API + 内部 MCP 调用 |
| Git 操作 | `git2` (libgit2) | 无需依赖系统 git，纯库调用 |
| Cron 解析 | `cron` | 解析 Cron 表达式计算下次执行时间 |
| 序列化 | `serde` + `serde_json` | JSON 序列化/反序列化 |
| 配置 | `toml` + `serde` | 解析 config.toml |
| 日志 | `tracing` + `tracing-subscriber` | 结构化日志，支持终端 + 文件输出 |
| 错误处理 | `anyhow` + `thiserror` | `anyhow` 用于应用层，`thiserror` 用于库层 |
| CLI | `clap` | 命令行参数解析（启动、手动触发任务等） |

---

## 七、目录结构

```
git-helper/
├── codex.md                # 本文档
├── Cargo.toml
├── config.toml             # 运行时配置
├── src/
│   ├── main.rs             # 入口：解析 CLI，启动 Scheduler + MCP Server
│   ├── config.rs           # 配置结构体 + 加载逻辑
│   ├── error.rs            # 统一错误类型 (thiserror)
│   ├── db/
│   │   ├── mod.rs          # 数据库初始化、连接池、Migration
│   │   ├── models.rs       # Task / GitRepo 结构体
│   │   ├── tasks.rs        # tasks 表 CRUD
│   │   └── repos.rs        # git_repos 表 CRUD
│   ├── scheduler/
│   │   ├── mod.rs          # Scheduler 主循环 (tokio interval)
│   │   └── dispatcher.rs   # 任务派发 + 并发控制
│   ├── executor/
│   │   └── codex.rs        # OpenAI API 调用封装 (reqwest)
│   ├── mcp/
│   │   ├── mod.rs          # MCP Server 启动 (axum)
│   │   ├── protocol.rs     # JSON-RPC 请求/响应结构
│   │   └── tools/
│   │       └── git.rs      # Git Tool 实现 (git2)
│   └── jobs/
│       └── git_review.rs   # 定时 Git 审核业务逻辑
├── check/                  # 审核报告输出目录
├── data/
│   └── scheduler.db        # SQLite 数据库文件（运行时生成）
├── logs/                   # 日志文件目录
└── tests/
    ├── scheduler_test.rs   # Scheduler 集成测试
    ├── executor_test.rs    # Codex 执行器测试
    └── mcp_test.rs         # MCP 服务端到端测试
```

---

## 八、测试手段

### 8.1 单元测试（`#[cfg(test)]` 模块内）

| 模块 | 测试内容 |
|------|---------|
| `db::tasks` | CRUD 操作、状态流转（pending→running→done/failed）、超时回退逻辑 |
| `db::repos` | 仓库增删改查、`last_commit` 更新 |
| `scheduler` | Mock DB，验证 1s tick 能正确筛选到期任务并更新状态 |
| `dispatcher` | 并发场景下同一任务不会被重复派发（两个"worker"竞争同一任务） |
| `executor::codex` | Mock HTTP（`wiremock`），验证请求格式、超时处理、重试逻辑（1s→2s→4s） |
| `mcp::protocol` | JSON-RPC 序列化/反序列化正确性 |
| `jobs::git_review` | 无新 commit 时跳过审核、报告文件格式正确 |

### 8.2 集成测试（`tests/` 目录）

| 场景 | 测试内容 |
|------|---------|
| MCP 端到端 | 启动 axum 服务 → `reqwest` 调用 `git_clone` / `git_pull` → 验证本地仓库状态 |
| 审核全流程 | 创建测试仓库 → 插入审核任务 → 运行 Scheduler → 验证 `check/` 下生成报告 |
| 任务生命周期 | `pending → running → done` 全链路；含 `failed` + 重试路径 |
| 故障恢复 | 插入 `running` + 过期 `started_at` 的记录 → 启动 Scheduler → 验证回退为 `pending` |

### 8.3 手动验收测试

1. **定时器验证**：`cargo run` 启动后，通过 CLI 插入 `scheduled_at = now()` 任务，观察 1-2 秒内是否被执行。
2. **MCP 验证**：`curl -X POST http://127.0.0.1:3100/mcp -d '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"git_status","arguments":{"path":"."}},"id":1}'`
3. **审核验证**：配置一个公开仓库（如 `https://github.com/user/repo`），等待定时触发，检查 `check/` 目录下报告。
4. **故障恢复**：任务执行中 kill 进程 → 重启 → 验证 stuck 任务被自动恢复。
5. **日志验证**：检查 `logs/git-helper.log` 输出格式和内容。

### 8.4 性能基准（`cargo bench` 或手动测量）

| 场景 | 目标 |
|------|------|
| Scheduler 1s 轮询，1000 条 pending 任务 | 查询 + 状态更新 < 10ms（Rust + SQLite） |
| MCP 服务并发 10 个 git_status 请求 | P99 响应 < 500ms |
| 单次 Codex 调用（不含 API 延迟） | 请求构建 + 响应解析 < 5ms |

---

## 九、开发里程碑

| 阶段 | 内容 | 验收标准 |
|------|------|---------|
| M1 | 项目脚手架 + 配置 + DB + Scheduler 空循环 | `cargo run` 启动，每秒打印 tick 日志，SQLite 建表成功 |
| M2 | Git MCP Server（axum + git2） | `curl` 调用所有 6 个 Tool 成功 |
| M3 | Codex 执行器 + 任务派发 | 手动插入任务 → 自动调用 Codex → 结果写回 DB |
| M4 | Git 审核业务逻辑 | 端到端：配置仓库 → 定时拉取 → Codex 审核 → 输出报告 |
| M5 | 故障恢复 + 重试 + 日志完善 | kill/restart 后任务自动恢复，日志可追踪全链路 |
| M6 | 自动生成测试用例 | 检测新增代码 → Codex 生成测试 → 输出到 `tests-generated/` 目录 |

---

## 十、Feature：根据新增代码自动生成测试用例

### 10.1 功能概述

在定时审核的基础上新增 `test_gen` 任务类型。当检测到仓库有新增代码（新增文件或新增函数/方法）时，自动调用 Codex 为这些新增代码生成对应的单元测试用例，输出到 `tests-generated/` 目录供开发者审阅和采纳。

### 10.2 执行流程

```
1. Scheduler 发现到期的 test_gen 类型任务
2. 通过 repo_id 查询 git_repos，获取仓库信息
3. 调用 Git MCP → git_pull 拉取最新代码
4. 调用 Git MCP → git_diff(from=last_commit, to=HEAD) 获取变更
5. 若无新 commit → 标记 done，跳过
6. 解析 diff，提取新增代码片段（新文件、新增函数/方法/类）
7. 对每个新增代码片段，组装 Prompt 调用 Codex 生成测试
8. 将生成的测试写入 tests-generated/{repo_name}/{date}/
9. 更新 git_repos.last_commit = HEAD
10. 标记 tasks.status = done，通知渠道推送摘要
```

### 10.3 task_type 扩展

`tasks.task_type` 枚举新增值：`test_gen`

```toml
# config.toml 中新增示例任务
[[tasks]]
name = "my-project-test-gen"
task_type = "test_gen"
repo_id = 1
cron_expr = "0 */2 * * *"     # 每 2 小时检查一次
```

### 10.4 Codex Prompt 设计

#### 主 Prompt（发送给 Codex 的完整指令）

```markdown
你是一个资深测试工程师。请根据以下新增代码为其生成高质量的单元测试用例。

## 约束与要求

1. **测试框架**：自动识别项目所用的测试框架并沿用（如 Rust 用 `#[cfg(test)]` + `#[test]`，Python 用 `pytest`，TypeScript 用 `vitest` 或 `jest`，Go 用 `testing` 包）。如果项目中尚无测试，根据语言选择最主流的框架。
2. **覆盖范围**：
   - 每个公开函数/方法至少生成 **正常路径（happy path）** 和 **异常/边界路径（edge case）** 各一个测试。
   - 若函数有多个分支逻辑（if/match/switch），为每个分支生成至少一个测试。
   - 若函数接受集合类型参数，生成空集合、单元素、多元素的测试。
3. **测试命名**：使用 `test_<函数名>_<场景描述>` 格式，场景描述用蛇形命名法，清晰表达测试意图。例如：`test_calculate_total_with_empty_cart`、`test_parse_config_missing_required_field`。
4. **Mock 与依赖**：
   - 对外部依赖（数据库、HTTP 调用、文件系统）使用 mock/stub，不发起真实 IO。
   - 优先使用项目已有的 mock 工具；若没有，选用语言生态中最常用的（如 Rust 的 `mockall`，Python 的 `unittest.mock`，Go 的 `gomock`）。
5. **断言**：使用精确断言（assert_eq / assertEqual），避免宽泛的 `assert!(true)`。验证返回值、副作用、错误类型。
6. **独立性**：每个测试用例独立运行，不依赖执行顺序，不共享可变状态。
7. **可读性**：每个测试以简短注释说明测试意图（一行即可）。
8. **不修改源码**：只生成测试文件，绝不修改被测源代码。

## 项目上下文

- 项目语言：{language}
- 项目根目录已挂载，你可以读取任意源文件以理解上下文。
- 项目现有测试目录：{existing_test_dir}（如果存在，请参考其风格）

## 新增代码（来自最近的 git diff）

```diff
{diff_content}
```

## 输出要求

1. 为每个新增的源文件生成对应的测试文件。文件路径遵循项目约定（如 Rust 在同模块内 `#[cfg(test)]` 或 `tests/` 目录，Python 在 `tests/test_<module>.py`）。
2. 输出格式为 **完整可运行的测试代码**，开发者复制即可使用。
3. 在输出开头提供一段摘要：列出为哪些函数生成了多少个测试，覆盖了哪些场景。
4. 如果某段新增代码是纯配置/数据定义/常量，无需生成测试，说明跳过原因即可。
```

#### diff 预处理策略

在发送给 Codex 之前，对 `git diff` 输出做预处理：

| 步骤 | 说明 |
|------|------|
| 过滤非代码文件 | 排除 `*.md`、`*.txt`、`*.json`、`*.toml`、`*.yaml`、`*.lock`、图片、字体等 |
| 只保留新增部分 | 仅提取 `+` 开头的行及其上下文（保留 5 行上文用于理解） |
| 识别新增函数 | 基于 diff hunk header（`@@ ... @@` 后的函数签名）定位新增函数 |
| 大 diff 拆分 | 若单次 diff 超过 8000 token，按文件拆分为多次 Codex 调用 |
| 注入文件全文 | 对于新增文件（`new file mode`），传入完整文件内容而非仅 diff |

#### Prompt 变量填充（Rust 侧）

```rust
fn build_test_gen_prompt(
    language: &str,
    existing_test_dir: &str,
    diff_content: &str,
) -> String {
    format!(
        include_str!("../prompts/test_gen.md"),
        language = language,
        existing_test_dir = existing_test_dir,
        diff_content = diff_content,
    )
}
```

Prompt 模板存放于 `src/prompts/test_gen.md`，便于独立迭代调优。

### 10.5 输出目录结构

```
tests-generated/
  └── {repo_name}/
      └── 2026-03-13/
          ├── _summary.md            # 本次生成的摘要报告
          ├── src_auth_login.rs      # 对应 src/auth/login.rs 的测试
          ├── src_db_queries.rs      # 对应 src/db/queries.rs 的测试
          └── ...
```

### 10.6 语言检测

通过仓库文件特征自动识别主语言：

| 特征文件 | 语言 |
|---------|------|
| `Cargo.toml` | Rust |
| `package.json` | JavaScript/TypeScript |
| `go.mod` | Go |
| `pyproject.toml` / `setup.py` / `requirements.txt` | Python |
| `pom.xml` / `build.gradle` | Java |

### 10.7 通知集成

复用现有 Notifier 模块，任务完成后推送摘要：

```
✅ my-project 测试生成完成
> 新增代码: 5 个文件, 12 个函数
> 生成测试: 28 个用例
> 输出目录: tests-generated/my-project/2026-03-13/
```

### 10.8 配置扩展

```toml
[test_gen]
# 过滤规则：只为匹配的文件路径生成测试
include_patterns = ["src/**/*.rs", "lib/**/*.rs"]
exclude_patterns = ["src/generated/**", "**/mod.rs"]
# 单次最大处理 diff 的 token 数
max_diff_tokens = 8000
# 生成测试的保留天数（自动清理旧报告）
retention_days = 30
```

---

## 十一、用户管理模块

### 11.1 功能概述

基于 Git 提交记录中的 author 信息自动发现用户。当 `git_review` 或 `test_gen` 任务完成后，系统从 commit range 中提取所有 commit 的 `author_name` 和 `author_email`，自动在 `users` 表中创建记录。用户通过 Git 邮箱 + 密码登录 Web 界面查看个人相关的审核报告和消息。

### 11.2 数据库表：`users`

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER PK | 自增主键 |
| email | TEXT NOT NULL UNIQUE | Git 提交邮箱（登录账号） |
| display_name | TEXT NOT NULL | Git 提交中的 author name |
| password_hash | TEXT | bcrypt 哈希，NULL 表示尚未设置密码 |
| status | TEXT NOT NULL DEFAULT 'inactive' | `inactive`（自动发现，未设密码）/ `active`（已设密码） |
| avatar_url | TEXT | 头像 URL（可选，默认由邮箱生成 Gravatar） |
| created_at | DATETIME DEFAULT CURRENT_TIMESTAMP | 创建时间 |
| updated_at | DATETIME DEFAULT CURRENT_TIMESTAMP | 最后更新时间 |

### 11.3 用户自动发现流程

```
git_review / test_gen 任务完成
       │
       ▼
从 commit range 提取所有 commit 的 author_email + author_name
       │
       ▼
对每个 author_email：
  ├── 已存在 → 跳过（可选：更新 display_name）
  └── 不存在 → INSERT INTO users (email, display_name, status='inactive')
```

#### CommitEntry 扩展

当前 `CommitEntry` 仅含 `id` 和 `summary`，需扩展：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitEntry {
    pub id: String,
    pub summary: String,
    pub author_name: String,    // 新增
    pub author_email: String,   // 新增
}
```

`git_log` 函数中从 `commit.author()` 提取：

```rust
.map(|commit| CommitEntry {
    id: commit.id().to_string(),
    summary: commit.summary().unwrap_or("").to_string(),
    author_name: commit.author().name().unwrap_or("").to_string(),
    author_email: commit.author().email().unwrap_or("").to_string(),
})
```

### 11.4 用户激活（设置密码）

首次发现的用户处于 `inactive` 状态，需通过 Web 界面设置密码后变为 `active`：

```
POST /api/auth/activate
Content-Type: application/json

{
  "email": "dev@example.com",
  "password": "my-secure-password"
}
```

- 验证 email 存在且 status = `inactive`
- 密码使用 `bcrypt` 哈希（cost=12）
- 更新 `password_hash` 和 `status='active'`
- 返回 JWT token

### 11.5 用户模型（Rust）

```rust
#[derive(Debug, Clone)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub display_name: String,
    pub password_hash: Option<String>,
    pub status: UserStatus,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UserStatus {
    Inactive,  // 自动发现，未设置密码
    Active,    // 已设置密码，可正常登录
}

impl UserStatus {
    pub fn as_str(&self) -> &str {
        match self {
            UserStatus::Inactive => "inactive",
            UserStatus::Active => "active",
        }
    }
}
```

### 11.6 数据库操作

```rust
impl Database {
    /// 根据邮箱查找或创建用户（幂等）
    pub fn ensure_user(&self, email: &str, display_name: &str) -> Result<i64> {
        // INSERT OR IGNORE + SELECT id
    }

    /// 批量确保用户存在（从 commit authors）
    pub fn ensure_users_from_commits(&self, commits: &[CommitEntry]) -> Result<Vec<i64>> {
        // 去重 email → 逐一 ensure_user
    }

    /// 根据邮箱查找用户（登录用）
    pub fn get_user_by_email(&self, email: &str) -> Result<Option<User>> { ... }

    /// 激活用户（设置密码）
    pub fn activate_user(&self, email: &str, password_hash: &str) -> Result<()> { ... }
}
```

---

## 十二、消息模块

### 12.1 功能概述

审核报告（git_review）完成后，系统根据 commit range 内每个 commit 的 author_email，将**整份审核报告**作为一条消息同步到对应用户的消息列表。每个涉及的开发者都会收到同一份报告的引用，但各自独立维护已读/未读状态。

### 12.2 数据库表：`messages`

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER PK | 自增主键 |
| user_id | INTEGER NOT NULL FK | 关联 `users.id`（接收人） |
| task_id | INTEGER NOT NULL FK | 关联 `tasks.id`（来源任务） |
| repo_id | INTEGER FK | 关联 `git_repos.id` |
| title | TEXT NOT NULL | 消息标题（如 "my-project 代码审查报告 2026-03-13"） |
| content | TEXT NOT NULL | 消息正文（审核报告全文或摘要） |
| report_path | TEXT | 报告文件路径（`check/repo/date-review.md`） |
| commit_range | TEXT | 相关 commit 范围（如 `abc123..def456`） |
| is_read | BOOLEAN NOT NULL DEFAULT 0 | 已读标记 |
| created_at | DATETIME DEFAULT CURRENT_TIMESTAMP | 创建时间 |

**索引**：
- `idx_messages_user_id` — 按用户查询消息列表
- `idx_messages_user_unread` — `(user_id, is_read)` 快速统计未读数

### 12.3 消息同步流程

```
git_review 任务执行完成（status = done）
       │
       ▼
从 JobOutput 获取 commit_authors: Vec<CommitAuthor>
       │
       ▼
调用 Database::ensure_users_from_commits() 确保用户存在
       │
       ▼
对每个去重后的 author_email：
  └── 查找 user_id
  └── INSERT INTO messages (user_id, task_id, repo_id, title, content, report_path, commit_range)
       │
       ▼
记录日志：info!(users_count, messages_created, "messages synced")
```

#### 集成切入点：`scheduler/mod.rs`

在 `Dispatcher::complete_task` 中，`git_review` 任务成功完成后增加消息同步调用：

```rust
async fn complete_task(&self, task: &Task, result: Result<JobOutput>) -> Result<()> {
    match result {
        Ok(output) => {
            self.database.finish_task(task, TaskStatus::Done, Some(&output.task_result))?;

            // ── 新增：消息同步 ──
            if task.task_type == TaskType::GitReview {
                if let Some(ref authors) = output.commit_authors {
                    if let Err(e) = self.sync_messages(task, &output, authors) {
                        warn!(%e, task_id = task.id, "message sync failed");
                    }
                }
            }

            self.notify(task, TaskStatus::Done, &output.summary, output.repo_name, output.report_path).await;
        }
        // ...
    }
}
```

#### JobOutput 扩展

```rust
pub struct JobOutput {
    pub task_result: String,
    pub summary: String,
    pub repo_name: Option<String>,
    pub report_path: Option<String>,
    pub commit_authors: Option<Vec<CommitAuthor>>,   // 新增
    pub commit_range: Option<String>,                // 新增
}

#[derive(Debug, Clone)]
pub struct CommitAuthor {
    pub name: String,
    pub email: String,
}
```

### 12.4 消息模型（Rust）

```rust
#[derive(Debug, Clone)]
pub struct Message {
    pub id: i64,
    pub user_id: i64,
    pub task_id: i64,
    pub repo_id: Option<i64>,
    pub title: String,
    pub content: String,
    pub report_path: Option<String>,
    pub commit_range: Option<String>,
    pub is_read: bool,
    pub created_at: DateTime<Utc>,
}

/// 消息列表项（不含全文 content，减少传输量）
#[derive(Debug, Clone, Serialize)]
pub struct MessageSummary {
    pub id: i64,
    pub title: String,
    pub repo_name: Option<String>,
    pub commit_range: Option<String>,
    pub is_read: bool,
    pub created_at: String,
}
```

### 12.5 数据库操作

```rust
impl Database {
    /// 为指定用户创建消息
    pub fn create_message(&self, msg: &NewMessage) -> Result<i64> { ... }

    /// 批量创建消息（一次审核 → 多个用户）
    pub fn create_messages_for_authors(
        &self,
        authors: &[CommitAuthor],
        task: &Task,
        report: &JobOutput,
    ) -> Result<usize> { ... }

    /// 查询用户消息列表（分页 + 可选筛选未读）
    pub fn list_messages(
        &self,
        user_id: i64,
        unread_only: bool,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<MessageSummary>> { ... }

    /// 获取消息详情（含全文内容）
    pub fn get_message(&self, message_id: i64, user_id: i64) -> Result<Option<Message>> { ... }

    /// 标记消息已读
    pub fn mark_message_read(&self, message_id: i64, user_id: i64) -> Result<()> { ... }

    /// 批量标记已读
    pub fn mark_all_read(&self, user_id: i64) -> Result<usize> { ... }

    /// 统计未读消息数
    pub fn unread_count(&self, user_id: i64) -> Result<usize> { ... }
}
```

---

## 十三、REST API 设计

### 13.1 概述

在现有 axum 服务基础上新增 REST API 路由组，提供用户认证和消息查询能力。API 与 MCP 服务共用同一 axum 实例和端口（`:3100`），通过路径前缀 `/api` 区分。

### 13.2 认证方式：JWT

- 登录成功后返回 JWT token（HS256 签名）
- 后续请求通过 `Authorization: Bearer <token>` 头携带
- Token 有效期 7 天，Payload 包含 `user_id`、`email`、`exp`
- 签名密钥通过 `config.toml` 的 `[web]` 段配置

#### JWT Payload

```json
{
  "sub": 1,
  "email": "dev@example.com",
  "exp": 1742000000
}
```

### 13.3 配置扩展

```toml
[web]
jwt_secret = "your-256-bit-secret"       # JWT 签名密钥
token_expire_hours = 168                  # Token 有效期（小时），默认 7 天
static_dir = "web/dist"                  # Vue3 构建产物目录
```

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct WebConfig {
    #[serde(default = "default_jwt_secret")]
    pub jwt_secret: String,
    #[serde(default = "default_token_expire_hours")]
    pub token_expire_hours: u64,
    #[serde(default = "default_static_dir")]
    pub static_dir: PathBuf,
}
```

### 13.4 API 接口清单

#### 公开接口（无需认证）

| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/api/auth/login` | 邮箱 + 密码登录，返回 JWT |
| POST | `/api/auth/activate` | 首次设置密码（激活账号） |

#### 受保护接口（需 JWT）

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/me` | 获取当前用户信息 |
| GET | `/api/messages` | 消息列表（支持分页、筛选未读） |
| GET | `/api/messages/:id` | 消息详情（返回全文，自动标记已读） |
| PUT | `/api/messages/:id/read` | 标记单条消息已读 |
| PUT | `/api/messages/read-all` | 标记全部已读 |
| GET | `/api/messages/unread-count` | 未读消息数量 |

### 13.5 接口详细定义

#### POST `/api/auth/login`

```json
// Request
{
  "email": "dev@example.com",
  "password": "my-password"
}

// Response 200
{
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "user": {
    "id": 1,
    "email": "dev@example.com",
    "display_name": "张三",
    "avatar_url": "https://gravatar.com/..."
  }
}

// Response 401
{
  "error": "invalid credentials"
}
```

#### POST `/api/auth/activate`

```json
// Request
{
  "email": "dev@example.com",
  "password": "new-password"
}

// Response 200
{
  "token": "eyJ...",
  "user": { ... }
}

// Response 400
{
  "error": "account already activated"
}

// Response 404
{
  "error": "user not found"
}
```

#### GET `/api/messages?unread=true&page=1&page_size=20`

```json
// Response 200
{
  "total": 42,
  "unread_count": 5,
  "page": 1,
  "page_size": 20,
  "items": [
    {
      "id": 101,
      "title": "my-project 代码审查报告 2026-03-13",
      "repo_name": "my-project",
      "commit_range": "abc123..def456",
      "is_read": false,
      "created_at": "2026-03-13T09:00:00Z"
    }
  ]
}
```

#### GET `/api/messages/:id`

```json
// Response 200
{
  "id": 101,
  "title": "my-project 代码审查报告 2026-03-13",
  "repo_name": "my-project",
  "content": "# Git Review\n\n- Repository: my-project\n...(完整审核报告)...",
  "report_path": "check/my-project/2026-03-13-review.md",
  "commit_range": "abc123..def456",
  "is_read": true,
  "created_at": "2026-03-13T09:00:00Z"
}
```

### 13.6 JWT 中间件（axum）

```rust
use axum::{
    extract::FromRequestParts,
    http::request::Parts,
    http::StatusCode,
};

pub struct AuthUser {
    pub user_id: i64,
    pub email: String,
}

#[async_trait]
impl<S: Send + Sync> FromRequestParts<S> for AuthUser {
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // 从 Authorization header 提取 Bearer token
        // 验证 JWT 签名和过期时间
        // 返回 AuthUser { user_id, email }
    }
}
```

### 13.7 路由挂载

```rust
// mcp/mod.rs 中扩展 Router
pub async fn serve(config: Arc<AppConfig>, database: Database) -> Result<()> {
    let state = AppState { config, database };
    let app = Router::new()
        .route("/health", get(health))
        .route("/mcp", post(handle_mcp))
        // ── 新增 REST API ──
        .nest("/api", web::api_router())
        // ── 静态文件托管（Vue3 SPA）──
        .fallback_service(ServeDir::new(&state.config.web.static_dir))
        .with_state(state);
    // ...
}
```

### 13.8 新增 Rust 依赖

| crate | 版本 | 用途 |
|-------|------|------|
| `jsonwebtoken` | 9.x | JWT 编码/解码 |
| `bcrypt` | 0.16 | 密码哈希 |
| `tower-http` | 0.6 | `ServeDir` 静态文件服务、CORS |

---

## 十四、Vue3 前端设计

### 14.1 概述

前端采用 Vue3 + TypeScript + Vite 构建，遵循 **Figma 设计风格**（简洁留白、圆角卡片、柔和阴影、层次化排版）。构建产物输出到 `web/dist/`，由 axum `ServeDir` 托管。

### 14.2 技术选型

| 组件 | 选型 | 理由 |
|------|------|------|
| 框架 | Vue 3 (Composition API) | 轻量、响应式 |
| 构建 | Vite | 快速 HMR，开箱即用 TS 支持 |
| 路由 | Vue Router 4 | SPA 路由 |
| 状态 | Pinia | 轻量状态管理 |
| HTTP | fetch / ofetch | 轻量，无需引入 axios |
| 样式 | Tailwind CSS | 原子化 CSS，快速还原 Figma 设计 |
| 图标 | Lucide Icons | 简洁线性图标，匹配 Figma 风格 |
| Markdown | markdown-it | 渲染审核报告正文 |

### 14.3 前端项目结构

```
web/
├── index.html
├── package.json
├── vite.config.ts
├── tsconfig.json
├── tailwind.config.ts
├── src/
│   ├── main.ts                  # 入口
│   ├── App.vue                  # 根组件
│   ├── router/
│   │   └── index.ts             # 路由配置
│   ├── stores/
│   │   ├── auth.ts              # 用户认证状态
│   │   └── messages.ts          # 消息列表状态
│   ├── api/
│   │   ├── client.ts            # HTTP 封装（附带 JWT header）
│   │   ├── auth.ts              # 登录/激活接口
│   │   └── messages.ts          # 消息接口
│   ├── views/
│   │   ├── LoginView.vue        # 登录页
│   │   ├── ActivateView.vue     # 账号激活页（首次设置密码）
│   │   └── MessagesView.vue     # 消息列表 + 详情页
│   ├── components/
│   │   ├── AppHeader.vue        # 顶部导航栏
│   │   ├── MessageCard.vue      # 消息卡片组件
│   │   ├── MessageDetail.vue    # 消息详情（Markdown 渲染）
│   │   ├── EmptyState.vue       # 空状态占位
│   │   └── UnreadBadge.vue      # 未读角标
│   └── styles/
│       └── globals.css          # Tailwind 基础 + 自定义设计 token
├── public/
│   └── favicon.svg
└── dist/                        # 构建输出（.gitignore）
```

### 14.4 路由设计

| 路径 | 组件 | 说明 | 认证 |
|------|------|------|------|
| `/login` | LoginView | 邮箱 + 密码登录 | 否 |
| `/activate` | ActivateView | 首次激活（设置密码） | 否 |
| `/messages` | MessagesView | 消息列表（默认页） | 是 |
| `/messages/:id` | MessagesView (detail) | 消息详情 | 是 |

### 14.5 页面设计（Figma 风格）

#### 设计系统 Token

```
颜色：
  - Primary:     #6C5CE7  (紫色主色)
  - Background:  #FAFBFC  (浅灰底)
  - Surface:     #FFFFFF  (卡片白)
  - Text:        #2D3436  (深灰文字)
  - Text-muted:  #636E72  (次要文字)
  - Success:     #00B894  (成功绿)
  - Error:       #E17055  (错误红)
  - Unread-dot:  #6C5CE7  (未读圆点)
  - Border:      #E9ECEF  (分割线)

圆角：
  - Card:        12px
  - Button:      8px
  - Input:       8px
  - Avatar:      50% (圆形)

阴影：
  - Card:        0 1px 3px rgba(0,0,0,0.08)
  - Card-hover:  0 4px 12px rgba(0,0,0,0.12)
  - Modal:       0 8px 30px rgba(0,0,0,0.15)

间距：
  - Page padding: 32px
  - Card padding: 24px
  - 元素间距: 16px (默认) / 8px (紧凑)
```

#### 登录页（LoginView）

```
┌──────────────────────────────────────────────────────────────────────┐
│                                                                      │
│                    ┌─────────────────────────────┐                   │
│                    │                             │                   │
│                    │        🔧 git-helper        │                   │
│                    │                             │                   │
│                    │   ┌───────────────────────┐ │                   │
│                    │   │ 📧  邮箱地址           │ │                   │
│                    │   └───────────────────────┘ │                   │
│                    │                             │                   │
│                    │   ┌───────────────────────┐ │                   │
│                    │   │ 🔒  密码              │ │                   │
│                    │   └───────────────────────┘ │                   │
│                    │                             │                   │
│                    │   ┌───────────────────────┐ │                   │
│                    │   │      登  录           │ │                   │
│                    │   └───────────────────────┘ │                   │
│                    │                             │                   │
│                    │   首次使用？激活账号 →       │                   │
│                    │                             │                   │
│                    └─────────────────────────────┘                   │
│                                                                      │
│  背景: #FAFBFC     卡片: 白色, 圆角 12px, 阴影                        │
│  按钮: Primary #6C5CE7, 白色文字, 圆角 8px                            │
└──────────────────────────────────────────────────────────────────────┘
```

**设计要点**：
- 居中卡片布局，宽度 400px
- 卡片内 24px 内边距，元素间 16px 间距
- 输入框左侧带图标，placeholder 为灰色
- 主按钮全宽，Primary 色，hover 加深
- 底部「激活账号」为文字链接

#### 消息列表页（MessagesView）

```
┌──────────────────────────────────────────────────────────────────────┐
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │  🔧 git-helper         消息中心         张三 ▾   🔔 5         │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                                                                      │
│  ┌─────────────────────────────────────────┐  ┌────────────────────┐│
│  │                                         │  │                    ││
│  │  🔘  全部 (42)    📩 未读 (5)           │  │  (消息详情区域)     ││
│  │                                         │  │                    ││
│  │  ┌───────────────────────────────────┐  │  │  选中一条消息后     ││
│  │  │ ● my-project 代码审查报告         │  │  │  显示完整报告内容   ││
│  │  │   2026-03-13 · abc12..def45      │  │  │                    ││
│  │  │   检测到 3 个潜在问题             │  │  │  ┌──────────────┐  ││
│  │  └───────────────────────────────────┘  │  │  │  # Git Review │  ││
│  │                                         │  │  │              │  ││
│  │  ┌───────────────────────────────────┐  │  │  │  Repository: │  ││
│  │  │   my-service 代码审查报告         │  │  │  │  my-project  │  ││
│  │  │   2026-03-12 · 123ab..789cd      │  │  │  │              │  ││
│  │  │   代码质量良好，无重大问题         │  │  │  │  Commit:     │  ││
│  │  └───────────────────────────────────┘  │  │  │  abc12..def45│  ││
│  │                                         │  │  │              │  ││
│  │  ┌───────────────────────────────────┐  │  │  │  ## 问题列表  │  ││
│  │  │   api-gateway 代码审查报告        │  │  │  │  1. ...      │  ││
│  │  │   2026-03-11 · aaa11..bbb22      │  │  │  │  2. ...      │  ││
│  │  │   发现 1 个安全风险               │  │  │  │              │  ││
│  │  └───────────────────────────────────┘  │  │  │  ## 建议      │  ││
│  │                                         │  │  │  ...         │  ││
│  │  ◄ 1  2  3 ►                           │  │  └──────────────┘  ││
│  │                                         │  │                    ││
│  └─────────────────────────────────────────┘  └────────────────────┘│
│                                                                      │
│  左栏: 360px, 消息卡片列表                                            │
│  右栏: flex-1, Markdown 渲染区                                        │
│  卡片: 白色底, 圆角 12px, hover 阴影加深                               │
│  未读标识: 左侧紫色圆点 ●                                              │
│  已读卡片: 文字颜色变浅                                                │
└──────────────────────────────────────────────────────────────────────┘
```

**设计要点**：
- 顶部导航栏：左侧 Logo + 标题，右侧用户头像下拉 + 未读角标
- 主体区域为左右分栏（master-detail 布局）
- 左栏消息卡片列表：
  - 未读消息左侧显示紫色圆点 `●`
  - 卡片包含：标题、日期、commit 范围（缩略）、摘要首行
  - 选中卡片高亮（左边框 3px Primary 色）
  - 底部分页器
- 右栏消息详情：
  - 标题 + 元信息（仓库名、commit 范围、审核时间）
  - Markdown 渲染报告全文
  - 顶部「标记已读」/「标记全部已读」操作按钮
- 响应式：移动端左右分栏改为全屏列表 → 点击进入详情

### 14.6 Vite 构建配置

```typescript
// vite.config.ts
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  plugins: [vue()],
  build: {
    outDir: 'dist',
  },
  server: {
    proxy: {
      '/api': 'http://127.0.0.1:3100',
      '/mcp': 'http://127.0.0.1:3100',
    },
  },
})
```

### 14.7 API 客户端封装

```typescript
// src/api/client.ts
const BASE_URL = ''

export async function request<T>(
  path: string,
  options: RequestInit = {},
): Promise<T> {
  const token = localStorage.getItem('token')
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    ...((options.headers as Record<string, string>) || {}),
  }
  if (token) {
    headers['Authorization'] = `Bearer ${token}`
  }

  const res = await fetch(`${BASE_URL}${path}`, {
    ...options,
    headers,
  })

  if (res.status === 401) {
    localStorage.removeItem('token')
    window.location.href = '/login'
    throw new Error('Unauthorized')
  }

  if (!res.ok) {
    const body = await res.json().catch(() => ({}))
    throw new Error(body.error || `HTTP ${res.status}`)
  }

  return res.json()
}
```

### 14.8 认证状态管理（Pinia）

```typescript
// src/stores/auth.ts
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'

export const useAuthStore = defineStore('auth', () => {
  const token = ref(localStorage.getItem('token') || '')
  const user = ref<User | null>(null)

  const isAuthenticated = computed(() => !!token.value)

  function setAuth(t: string, u: User) {
    token.value = t
    user.value = u
    localStorage.setItem('token', t)
  }

  function logout() {
    token.value = ''
    user.value = null
    localStorage.removeItem('token')
  }

  return { token, user, isAuthenticated, setAuth, logout }
})
```

### 14.9 构建集成

前端构建产物由 Rust 后端托管，开发和部署流程：

```bash
# 开发模式（前端 HMR + 代理后端 API）
cd web && npm run dev

# 生产构建
cd web && npm run build    # 输出到 web/dist/

# 启动后端（自动托管 web/dist/）
cargo run
```

axum 静态文件托管：

```rust
use tower_http::services::ServeDir;

let app = Router::new()
    .route("/health", get(health))
    .route("/mcp", post(handle_mcp))
    .nest("/api", web::api_router())
    .fallback_service(
        ServeDir::new(&config.web.static_dir)
            .append_index_html_on_directories(true)
    );
```

SPA 路由需要 fallback 到 `index.html`，确保 Vue Router 的 history 模式正常工作。

### 14.10 目录结构更新

```
git-helper/
├── src/
│   ├── web/                    # 新增：Web API 模块
│   │   ├── mod.rs              # 路由注册
│   │   ├── auth.rs             # 登录/激活接口
│   │   ├── messages.rs         # 消息查询接口
│   │   └── middleware.rs       # JWT 验证中间件
│   └── ...（现有模块不变）
├── web/                        # 新增：Vue3 前端项目
│   ├── src/
│   │   ├── views/
│   │   ├── components/
│   │   ├── stores/
│   │   └── api/
│   └── dist/                   # 构建产物
└── ...
```

### 14.11 开发里程碑扩展

| 阶段 | 内容 | 验收标准 |
|------|------|---------|
| M7 | 用户管理 + 消息表 + CommitEntry 扩展 | git_review 完成后 users/messages 表自动填充 |
| M8 | REST API（登录 + 消息接口） | curl 调用登录获取 token，查询消息列表正常 |
| M9 | Vue3 前端页面 | 登录 → 查看消息列表 → 查看报告详情 全流程通过 |
| M10 | 集成联调 + 样式完善 | Figma 风格还原度达标，移动端响应式正常 |
| M11 | 管理员模式 + 项目管理页面 | superAdmin 登录后可通过页面完成所有 CLI 操作 |
| M12 | 用户管理 + 任务管理页面 | 管理员可在页面上管理用户、查看/创建任务 |

---

## 十五、管理员模式（superAdmin）

### 15.1 设计目标

将现有 CLI 操作（`add-repo`、`add-task`、`add-user`、`list-repos`、`list-tasks`、`list-users`）全部迁移到 Web 页面，由管理员通过浏览器完成，普通用户无需接触命令行。

### 15.2 角色定义

| 角色 | 说明 |
|------|------|
| `superAdmin` | 超级管理员，密码在 `config.toml` 中配置，拥有所有管理权限 |
| `user` | 普通用户，仅可查看消息、报告，无管理权限 |

- `superAdmin` 是系统内置账户，不存储在 `users` 表中，通过配置文件认证
- 系统启动时自动识别 superAdmin 配置，无需手动 `add-user`

### 15.3 config.toml 新增配置

```toml
[admin]
email = "admin@example.com"
password = "change-me-in-production"
display_name = "Super Admin"
```

- `email` — 管理员登录邮箱
- `password` — 明文密码（仅存于配置文件，登录时与输入做 bcrypt 比对或直接比对）
- `display_name` — 页面显示名称

### 15.4 登录认证流程

```
用户输入 email + password
        │
        ▼
  email 匹配 config.toml [admin].email ?
        │
    ┌───┴───┐
    Yes     No
    │       │
    ▼       ▼
  比对密码   查询 users 表
  (直接比对)  (bcrypt 验证)
    │       │
    ▼       ▼
  签发 JWT   签发 JWT
  role=superAdmin  role=user
```

- JWT payload 新增 `role` 字段：`"superAdmin"` 或 `"user"`
- 后端中间件解析 token 后，将 role 注入请求上下文
- 管理接口统一校验 `role == "superAdmin"`，否则返回 403

### 15.5 后端 API 设计

#### 15.5.1 Git 项目管理

| 方法 | 路径 | 说明 | 权限 |
|------|------|------|------|
| `GET` | `/api/admin/repos` | 获取所有 Git 项目列表 | superAdmin |
| `POST` | `/api/admin/repos` | 新增 Git 项目 | superAdmin |
| `PUT` | `/api/admin/repos/{id}` | 编辑 Git 项目 | superAdmin |
| `DELETE` | `/api/admin/repos/{id}` | 删除 Git 项目 | superAdmin |
| `POST` | `/api/admin/repos/{id}/sync` | 手动触发同步 | superAdmin |

新增项目请求体：

```json
{
  "name": "my-project",
  "repo_url": "https://github.com/org/repo.git",
  "branch": "main",
  "local_path": "/repos/my-project",
  "review_cron": "0 9 * * 1-5",
  "enabled": true
}
```

#### 15.5.2 用户管理

| 方法 | 路径 | 说明 | 权限 |
|------|------|------|------|
| `GET` | `/api/admin/users` | 获取所有用户列表 | superAdmin |
| `POST` | `/api/admin/users` | 新增用户 | superAdmin |
| `PUT` | `/api/admin/users/{id}` | 编辑用户 | superAdmin |
| `DELETE` | `/api/admin/users/{id}` | 删除用户 | superAdmin |

#### 15.5.3 任务管理

| 方法 | 路径 | 说明 | 权限 |
|------|------|------|------|
| `GET` | `/api/admin/tasks` | 获取任务列表（支持分页/筛选） | superAdmin |
| `POST` | `/api/admin/tasks` | 新增任务 | superAdmin |
| `DELETE` | `/api/admin/tasks/{id}` | 删除任务 | superAdmin |
| `GET` | `/api/admin/dashboard` | 仪表盘统计数据 | superAdmin |

### 15.6 前端页面设计

#### 15.6.1 新增路由

```typescript
// web/src/router/index.ts 新增
{
  path: '/admin',
  component: AdminLayout,
  meta: { requiresAuth: true, role: 'superAdmin' },
  children: [
    { path: '', redirect: '/admin/dashboard' },
    { path: 'dashboard', component: () => import('@/views/admin/Dashboard.vue') },
    { path: 'repos', component: () => import('@/views/admin/Repos.vue') },
    { path: 'users', component: () => import('@/views/admin/Users.vue') },
    { path: 'tasks', component: () => import('@/views/admin/Tasks.vue') },
  ]
}
```

- 路由守卫：检查 JWT 中的 `role`，非 superAdmin 跳转到普通用户首页

#### 15.6.2 页面功能说明

**Dashboard（仪表盘）**
- 项目总数、任务总数、用户总数、今日执行任务数
- 最近任务执行状态列表（最近 10 条）

**Repos（项目管理）**
- 表格展示所有 Git 项目：名称、仓库地址、分支、Cron 表达式、启用状态
- 操作：新增、编辑、删除、手动同步
- 新增/编辑弹窗表单：填写 name、repo_url、branch、local_path、review_cron、enabled

**Users（用户管理）**
- 表格展示所有用户：邮箱、显示名、激活状态、创建时间
- 操作：新增、编辑、删除
- 新增弹窗：填写 email、display_name（密码由用户自行激活）

**Tasks（任务管理）**
- 表格展示任务列表：任务名、类型、关联项目、Cron、状态、上次执行时间
- 支持按状态筛选（pending / running / done / failed）
- 操作：新增、删除
- 新增弹窗：选择任务类型、关联项目、填写 Prompt、Cron 表达式

#### 15.6.3 目录结构新增

```
web/src/
├── views/
│   ├── admin/
│   │   ├── Dashboard.vue      # 仪表盘
│   │   ├── Repos.vue          # 项目管理
│   │   ├── Users.vue          # 用户管理
│   │   └── Tasks.vue          # 任务管理
│   └── ...
├── components/
│   ├── admin/
│   │   ├── AdminLayout.vue    # 管理后台布局（侧边栏导航）
│   │   ├── RepoForm.vue       # 项目新增/编辑表单
│   │   ├── UserForm.vue       # 用户新增/编辑表单
│   │   └── TaskForm.vue       # 任务新增表单
│   └── ...
└── api/
    └── admin.ts               # 管理员 API 调用封装
```

### 15.7 权限中间件实现要点

```rust
// src/web/middleware.rs 扩展
pub struct AuthUser {
    pub user_id: Option<i64>,  // superAdmin 时为 None
    pub email: String,
    pub role: String,          // "superAdmin" | "user"
}

/// 管理员权限守卫
pub struct RequireAdmin(pub AuthUser);
// 从 AuthUser 中检查 role == "superAdmin"，否则返回 403
```

### 15.8 安全注意事项

1. `config.toml` 中的 admin 密码应在部署时修改，避免使用默认值
2. 生产环境建议对 `config.toml` 设置文件权限 `600`
3. 管理员 API 全部挂载在 `/api/admin/` 前缀下，统一通过中间件鉴权
4. 删除操作需前端二次确认弹窗，防止误操作
5. 日志记录所有管理员操作，便于审计
