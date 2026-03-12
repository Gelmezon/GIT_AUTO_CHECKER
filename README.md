# git-helper

基于定时器 + SQLite 的 AI 调度系统，自动拉取 Git 仓库并通过 OpenAI Codex 进行代码审查与测试生成，输出结构化报告并推送通知。

## 功能概览

1. **定时拉取代码** — 按 Cron 表达式周期性 `git pull` 配置的仓库
2. **AI 代码审查** — 将新增 diff 发送给 Codex (`gpt-5.4`)，获取质量评估、Bug 检测、安全风险和改进建议
3. **AI 测试生成** — 检测新增代码，自动生成对应的单元测试用例，输出到 `tests-generated/` 目录
4. **审核报告** — Markdown 格式写入 `check/{repo_name}/{date}-review.md`
5. **多渠道通知** — 任务完成后通过企业微信 / Telegram / WhatsApp 推送摘要
6. **只读安全** — 系统只拉取和读取代码，**绝不 commit、push 或修改源码**
7. **MCP 服务** — 对外暴露标准化的 Git 只读操作（MCP 协议），供其他 AI 工具调用

## 系统架构

```
┌──────────────────────────────────────────────────────────────────┐
│                        git-helper 进程                            │
│                                                                  │
│  Scheduler (1s tick)                                             │
│      │                                                           │
│      ▼                                                           │
│  SQLite DB ──▸ Dispatcher ──▸ Codex (codex exec)                │
│  (tasks / git_repos)              │                              │
│                                   ├──▸ check/ 审核报告            │
│                                   └──▸ tests-generated/ 测试用例  │
│                                          │                       │
│  Git MCP Server (axum, :3100)            ▼                       │
│  Tools: clone/pull/log/diff/status   Notifier (通知分发)          │
│                                      ├─ 企业微信 Webhook          │
│                                      ├─ Telegram Bot API         │
│                                      └─ WhatsApp Cloud API       │
└──────────────────────────────────────────────────────────────────┘
```

**关键设计**：Scheduler、Dispatcher、MCP Server、Notifier 运行在同一进程的不同 tokio task 中，共享数据库连接池。单进程部署，SQLite 通过 WAL 模式支持并发读。

## 前置依赖

| 依赖 | 版本 | 说明 |
|------|------|------|
| Rust | >= 1.85 | 编译工具链（edition 2024） |
| OpenAI Codex CLI | 最新 | `npm install -g @openai/codex` |
| SQLite | - | 通过 `rusqlite` 静态链接，无需单独安装 |
| Git | >= 2.0 | `git2` (libgit2) 不依赖系统 git，但克隆 SSH 仓库需要系统 git 配置 |

## 快速开始

### 1. 克隆与构建

```bash
git clone https://github.com/yourname/git-helper.git
cd git-helper
cargo build --release
```

### 2. 配置环境变量

```bash
# 二选一
export CODEX_API_KEY="sk-..."
export OPENAI_API_KEY="sk-..."
```

### 3. 编辑配置文件

编辑 `config.toml`：

```toml
[scheduler]
interval_secs = 1
task_timeout_secs = 300
max_concurrency = 4
claim_batch_size = 16

[codex]
command = "codex"
model = "gpt-5.4"
max_retries = 2
timeout_secs = 300

[mcp]
host = "127.0.0.1"
port = 3100

[database]
path = "data/scheduler.db"

[runtime]
check_dir = "check"
tests_generated_dir = "tests-generated"

[log]
level = "info"
file = "logs/git-helper.log"
```

### 4. 启动服务

```bash
cargo run
```

### 5. 添加仓库与任务

```bash
# 添加仓库
cargo run -- add-repo \
  --name "my-project" \
  --repo-url "https://github.com/user/my-project.git" \
  --branch main \
  --local-path "./repos/my-project" \
  --review-cron "0 9 * * *"

# 添加代码审查任务
cargo run -- add-task \
  --name "my-project-review" \
  --task-type git_review \
  --repo-id 1 \
  --prompt "审查最近的代码变更，输出中文报告" \
  --cron-expr "0 9 * * *"

# 添加测试生成任务
cargo run -- add-task \
  --name "my-project-test-gen" \
  --task-type test_gen \
  --repo-id 1 \
  --prompt "为新增代码生成单元测试" \
  --cron-expr "0 */2 * * *"
```

### 6. 查看输出

```
check/                          # 审核报告
  └── my-project/
      └── 2026-03-12-review.md

tests-generated/                # 生成的测试用例
  └── my-project/
      └── 2026-03-13/
          ├── _summary.md
          └── src_auth_login.rs
```

## CLI 命令

```bash
git-helper                    # 默认启动调度服务 + MCP 服务（等同于 run）
git-helper run                # 启动调度服务 + MCP 服务

git-helper add-repo           # 添加仓库
  --name <NAME>
  --repo-url <URL>
  --branch <BRANCH>           # 默认 main
  --local-path <PATH>
  --review-cron <CRON>        # 可选

git-helper add-task           # 添加任务
  --name <NAME>
  --task-type <TYPE>          # git_review / test_gen / custom
  --prompt <PROMPT>
  --repo-id <ID>              # 可选，审核/测试生成任务需要
  --cron-expr <CRON>          # 可选，周期任务
  --scheduled-at <RFC3339>    # 可选，默认立即执行

git-helper list-repos         # 列出所有仓库
git-helper list-tasks         # 查看所有任务
```

通过 `--config <PATH>` 指定配置文件路径（默认 `config.toml`）。

## 任务类型

| 类型 | 说明 |
|------|------|
| `git_review` | 定时代码审查 — 拉取最新代码，diff 对比后调用 Codex 生成审查报告 |
| `test_gen` | 自动测试生成 — 检测新增代码，调用 Codex 为新增函数生成单元测试 |
| `custom` | 自定义任务 — 直接执行用户提供的 Prompt |

### 任务状态流转

```
pending ──▸ running ──▸ done
               │
               ├──▸ failed (重试耗尽)
               │
               └──▸ pending (超时回退 / 重试)
```

周期任务（`cron_expr` 非空）完成后会自动计算下次执行时间并创建新的 `pending` 记录。

## MCP 服务

启动后默认监听 `127.0.0.1:3100`，通过 Streamable HTTP 暴露以下只读 Tool：

| Tool | 参数 | 说明 |
|------|------|------|
| `git_clone` | `url`, `path`, `branch?` | 克隆仓库 |
| `git_pull` | `path` | 拉取最新代码 |
| `git_log` | `path`, `count?`, `since?` | 查看 commit 历史 |
| `git_diff` | `path`, `from`, `to?` | 查看两个 commit 之间的 diff |
| `git_status` | `path` | 查看工作区状态 |

调用示例：

```bash
curl -X POST http://127.0.0.1:3100/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "tools/call",
    "params": {
      "name": "git_status",
      "arguments": {"path": "./repos/my-project"}
    },
    "id": 1
  }'
```

> **安全约束**：MCP 服务仅暴露只读操作，不提供 commit / push / merge / rebase / reset 等写操作。

## 通知配置

在 `config.toml` 中配置通知渠道，支持同时启用多个：

```toml
[[notifier.channels]]
name = "dev-team-wecom"
kind = "wecom"
enabled = true
webhook_url = "https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=YOUR_KEY"

[[notifier.channels]]
name = "dev-team-telegram"
kind = "telegram"
enabled = false
bot_token = "123456:ABC-DEF..."
chat_id = "-1001234567890"

[[notifier.channels]]
name = "dev-team-whatsapp"
kind = "whatsapp"
enabled = false
api_url = "https://graph.facebook.com/v21.0/PHONE_NUMBER_ID/messages"
access_token = "EAAG..."
recipient = "8613800138000"
```

| 渠道 | 认证方式 | 消息格式 |
|------|---------|---------|
| 企业微信 | Webhook URL 中的 key 参数 | Markdown |
| Telegram | Bot Token + Chat ID | MarkdownV2 |
| WhatsApp | Bearer Token | 纯文本 |

通知发送异步化，发送失败仅记录日志，不影响任务状态。

## Codex 调用方式

系统通过 `codex exec` 非交互模式调用 Codex CLI：

```bash
codex exec "<prompt>" \
  --model gpt-5.4 \
  --approval-mode never \
  --path <repo_path> \
  --json
```

- `--approval-mode never`：全自动执行，无需人工确认
- `--json`：输出 JSONL 事件流，便于程序解析
- `--path`：指定 Codex 的工作目录为目标仓库

Rust 侧通过 `std::process::Command` 调用，放在 `tokio::task::spawn_blocking` 中避免阻塞异步运行时。失败自动重试（最多 2 次，指数退避 2s → 4s）。

## 目录结构

```
git-helper/
├── README.md               # 本文件
├── codex.md                # 详细设计文档
├── Cargo.toml
├── config.toml             # 运行时配置
├── src/
│   ├── main.rs             # 入口：CLI 解析，启动 Scheduler + MCP Server
│   ├── lib.rs              # 模块导出
│   ├── config.rs           # 配置结构体 + 加载逻辑
│   ├── error.rs            # 统一错误类型 (thiserror)
│   ├── notifier.rs         # 通知分发（企业微信 / Telegram / WhatsApp）
│   ├── prompts/
│   │   └── test_gen.md     # 测试生成 Prompt 模板
│   ├── db/
│   │   ├── mod.rs          # 数据库初始化、连接池、Migration
│   │   └── models.rs       # Task / GitRepo 结构体
│   ├── scheduler/
│   │   └── mod.rs          # Scheduler 主循环 + 任务派发
│   ├── executor/
│   │   ├── mod.rs
│   │   └── codex.rs        # Codex CLI 调用封装
│   ├── mcp/
│   │   ├── mod.rs          # MCP Server 启动 (axum)
│   │   ├── protocol.rs     # JSON-RPC 请求/响应结构
│   │   ├── client.rs       # MCP 内部客户端
│   │   └── tools/
│   │       └── git.rs      # Git Tool 实现 (git2)
│   └── jobs/
│       ├── mod.rs          # 任务类型分发
│       ├── git_review.rs   # 定时 Git 审核业务逻辑
│       └── test_gen.rs     # 自动测试生成业务逻辑
├── check/                  # 审核报告输出目录
├── tests-generated/        # 生成的测试用例输出目录
├── data/                   # SQLite 数据库（运行时生成）
└── logs/                   # 日志文件
```

## 技术栈

| 组件 | 选型 |
|------|------|
| 语言 | Rust (edition 2024) |
| 异步运行时 | tokio |
| 数据库 | SQLite (rusqlite, WAL 模式) |
| HTTP 服务 | axum |
| HTTP 客户端 | reqwest (rustls) |
| Git 操作 | git2 (libgit2, vendored) |
| AI 引擎 | OpenAI Codex CLI (`codex exec`) |
| Cron 解析 | cron |
| 日志 | tracing + tracing-subscriber + tracing-appender |
| CLI | clap (derive) |
| 错误处理 | anyhow + thiserror |
| 序列化 | serde + serde_json + toml |

## 开发

```bash
# 运行测试
cargo test

# 运行并查看详细日志
RUST_LOG=debug cargo run

# 检查编译
cargo check
```

## 设计文档

完整的架构设计、数据库表结构、状态流转、Prompt 设计、测试策略等详见 [codex.md](codex.md)。

## License

MIT
