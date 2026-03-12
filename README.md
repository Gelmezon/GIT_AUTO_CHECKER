# git-helper

基于定时器 + SQLite 的 AI 调度系统，自动拉取 Git 仓库并通过 OpenAI Codex 进行代码审查与测试生成，输出结构化报告并推送通知。内置用户管理与消息系统，审核建议自动同步到提交人的消息列表，配合 Vue3 Web 界面查看。

## 功能概览

1. **定时拉取代码** — 按 Cron 表达式周期性 `git pull` 配置的仓库
2. **AI 代码审查** — 将新增 diff 发送给 Codex (`gpt-5.4`)，获取质量评估、Bug 检测、安全风险和改进建议
3. **AI 测试生成** — 检测新增代码，自动生成对应的单元测试用例，输出到 `tests-generated/` 目录
4. **审核报告** — Markdown 格式写入 `check/{repo_name}/{date}-review.md`
5. **消息同步** — 审核完成后，根据 git log 的提交人自动将报告同步到对应用户的消息列表
6. **用户管理** — 基于 git commit author email 自动发现用户，支持邮箱 + 密码登录
7. **Web 界面** — Vue3 前端（Figma 设计风格），支持登录和消息查看
8. **多渠道通知** — 任务完成后通过企业微信 / Telegram / WhatsApp 推送摘要
9. **只读安全** — 系统只拉取和读取代码，**绝不 commit、push 或修改源码**
10. **MCP 服务** — 对外暴露标准化的 Git 只读操作（MCP 协议），供其他 AI 工具调用

## 系统架构

```
┌──────────────────────────────────────────────────────────────────┐
│                        git-helper 进程                            │
│                                                                  │
│  Scheduler (1s tick)                                             │
│      │                                                           │
│      ▼                                                           │
│  SQLite DB ──▸ Dispatcher ──▸ Codex (codex exec)                │
│  (tasks/git_repos/              │                                │
│   users/messages)               ├──▸ check/ 审核报告              │
│                                 ├──▸ tests-generated/ 测试用例    │
│                                 └──▸ 消息同步 (commit author)     │
│                                        │                         │
│  Git MCP Server + REST API (axum, :3100)                         │
│  ├─ MCP: clone/pull/log/diff/status                              │
│  ├─ API: /api/auth/* · /api/messages/*                           │
│  └─ Static: Vue3 SPA (web/dist/)                                 │
│                                        │                         │
│  Notifier (通知分发)                    ▼                         │
│  ├─ 企业微信 Webhook              Vue3 前端                       │
│  ├─ Telegram Bot API             (登录 · 消息列表 · 报告详情)     │
│  └─ WhatsApp Cloud API                                           │
└──────────────────────────────────────────────────────────────────┘
```

**关键设计**：Scheduler、Dispatcher、MCP Server、REST API、Notifier 运行在同一进程的不同 tokio task 中，共享数据库连接池。单进程部署，SQLite 通过 WAL 模式支持并发读。审核完成后根据 git log commit author 自动同步消息到对应用户。

## 前置依赖

| 依赖 | 版本 | 说明 |
|------|------|------|
| Rust | >= 1.85 | 编译工具链（edition 2024） |
| Node.js | >= 18 | Vue3 前端构建 |
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

[web]
jwt_secret = "your-256-bit-secret"
token_expire_hours = 168
static_dir = "web/dist"

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

## 用户与消息

系统基于 git commit 的 author email 自动发现用户：

1. `git_review` 任务完成后，提取 commit range 内所有 commit 的 author email
2. 首次出现的 email 自动创建用户（状态 `inactive`）
3. 用户访问 Web 界面设置密码后激活（状态 `active`）
4. 整份审核报告作为一条消息同步到每个相关提交人的消息列表

## Web 界面

Vue3 前端（Figma 设计风格），由 axum 托管静态资源，与 MCP 服务共用 `:3100` 端口。

| 页面 | 路径 | 说明 |
|------|------|------|
| 登录 | `/login` | 邮箱 + 密码登录 |
| 激活 | `/activate` | 首次设置密码 |
| 消息 | `/messages` | 消息列表 + 报告详情（master-detail 布局） |

前端开发：

```bash
cd web && npm install && npm run dev    # 开发模式（HMR + 代理后端）
cd web && npm run build                 # 生产构建 → web/dist/
```

## REST API

API 路径前缀 `/api`，认证方式为 JWT（`Authorization: Bearer <token>`）。

| 方法 | 路径 | 认证 | 说明 |
|------|------|------|------|
| POST | `/api/auth/login` | 否 | 邮箱 + 密码登录，返回 JWT |
| POST | `/api/auth/activate` | 否 | 首次设置密码（激活账号） |
| GET | `/api/me` | 是 | 获取当前用户信息 |
| GET | `/api/messages` | 是 | 消息列表（分页、筛选未读） |
| GET | `/api/messages/:id` | 是 | 消息详情（含审核报告全文） |
| PUT | `/api/messages/:id/read` | 是 | 标记单条已读 |
| PUT | `/api/messages/read-all` | 是 | 标记全部已读 |
| GET | `/api/messages/unread-count` | 是 | 未读消息数 |

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
│   ├── main.rs             # 入口：CLI 解析，启动 Scheduler + MCP + Web
│   ├── lib.rs              # 模块导出
│   ├── config.rs           # 配置结构体 + 加载逻辑
│   ├── error.rs            # 统一错误类型 (thiserror)
│   ├── notifier.rs         # 通知分发（企业微信 / Telegram / WhatsApp）
│   ├── prompts/
│   │   └── test_gen.md     # 测试生成 Prompt 模板
│   ├── db/
│   │   ├── mod.rs          # 数据库初始化、连接池、Migration
│   │   └── models.rs       # Task / GitRepo / User / Message 结构体
│   ├── scheduler/
│   │   └── mod.rs          # Scheduler 主循环 + 任务派发 + 消息同步
│   ├── executor/
│   │   ├── mod.rs
│   │   └── codex.rs        # Codex CLI 调用封装
│   ├── mcp/
│   │   ├── mod.rs          # MCP Server + 静态文件托管 (axum)
│   │   ├── protocol.rs     # JSON-RPC 请求/响应结构
│   │   ├── client.rs       # MCP 内部客户端
│   │   └── tools/
│   │       └── git.rs      # Git Tool 实现 (git2)
│   ├── web/                # REST API 模块
│   │   ├── mod.rs          # API 路由注册
│   │   ├── auth.rs         # 登录/激活接口
│   │   ├── messages.rs     # 消息查询接口
│   │   └── middleware.rs   # JWT 验证中间件
│   └── jobs/
│       ├── mod.rs          # 任务类型分发
│       ├── git_review.rs   # 定时 Git 审核业务逻辑
│       └── test_gen.rs     # 自动测试生成业务逻辑
├── web/                    # Vue3 前端项目
│   ├── src/
│   │   ├── views/          # 页面组件（Login / Activate / Messages）
│   │   ├── components/     # 通用组件（MessageCard / AppHeader 等）
│   │   ├── stores/         # Pinia 状态管理
│   │   └── api/            # HTTP 客户端封装
│   └── dist/               # 构建产物（axum 托管）
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
| HTTP 服务 | axum + tower-http (ServeDir) |
| HTTP 客户端 | reqwest (rustls) |
| Git 操作 | git2 (libgit2, vendored) |
| AI 引擎 | OpenAI Codex CLI (`codex exec`) |
| 认证 | jsonwebtoken (JWT) + bcrypt |
| Cron 解析 | cron |
| 日志 | tracing + tracing-subscriber + tracing-appender |
| CLI | clap (derive) |
| 错误处理 | anyhow + thiserror |
| 序列化 | serde + serde_json + toml |
| 前端 | Vue 3 + TypeScript + Vite + Tailwind CSS + Pinia |

## 开发

```bash
# 运行测试
cargo test

# 运行并查看详细日志
RUST_LOG=debug cargo run

# 检查编译
cargo check

# 前端开发（HMR + 代理后端 API）
cd web && npm run dev

# 前端构建
cd web && npm run build
```

## 设计文档

完整的架构设计、数据库表结构、状态流转、Prompt 设计、测试策略等详见 [codex.md](codex.md)。

## License

MIT
