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
# 推荐：直接使用本机 Codex 登录态
codex login

# 可选：如果你仍想显式传 key，也支持下面两种环境变量
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

## 详细使用指南

### 场景一：从零开始搭建自动代码审查

**目标**：每天早上 9 点自动审查团队项目的新提交，生成报告并推送到企业微信。

```bash
# 1. 构建项目
git clone https://github.com/yourname/git-helper.git
cd git-helper
cargo build --release

# 2. 构建前端
cd web && npm install && npm run build && cd ..

# 3. 准备 Codex 认证
# 推荐直接使用本机登录态
codex login

# 或者继续使用环境变量
# export CODEX_API_KEY="sk-..."

# 4. 编辑 config.toml（至少配置 notifier 和 web.jwt_secret）
#    参考上方「编辑配置文件」章节

# 5. 启动服务（后台运行）
./target/release/git-helper run &

# 6. 验证服务启动
curl http://127.0.0.1:3100/health
# 返回 {"status":"ok"} 表示正常

# 7. 添加要审查的仓库
./target/release/git-helper add-repo \
  --name "my-backend" \
  --repo-url "https://github.com/team/my-backend.git" \
  --branch main \
  --local-path "./repos/my-backend"

# 8. 添加每日审查任务（每天 09:00 执行）
./target/release/git-helper add-task \
  --name "daily-review" \
  --task-type git_review \
  --repo-id 1 \
  --prompt "审查最近的代码变更，重点关注安全风险和性能问题，输出中文报告" \
  --cron-expr "0 9 * * *"

# 9. 验证仓库和任务
./target/release/git-helper list-repos
./target/release/git-helper list-tasks
```

任务执行后：
- 报告写入 `check/my-backend/2026-03-13-review.md`
- 企业微信收到摘要通知
- 提交人在 Web 界面 `http://127.0.0.1:3100/messages` 查看完整报告

### 场景二：添加多个仓库 + 测试生成

```bash
# 添加第二个仓库
./target/release/git-helper add-repo \
  --name "my-frontend" \
  --repo-url "https://github.com/team/my-frontend.git" \
  --branch develop \
  --local-path "./repos/my-frontend"

# 为第二个仓库添加审查 + 测试生成
./target/release/git-helper add-task \
  --name "frontend-review" \
  --task-type git_review \
  --repo-id 2 \
  --prompt "审查前端代码变更，关注 XSS 风险和组件设计" \
  --cron-expr "0 9 * * 1-5"

./target/release/git-helper add-task \
  --name "frontend-test-gen" \
  --task-type test_gen \
  --repo-id 2 \
  --prompt "为新增的 Vue 组件和工具函数生成单元测试" \
  --cron-expr "0 */4 * * *"
```

### 场景三：立即执行一次性任务

不设置 `--cron-expr`，任务将在下一个调度周期（1 秒内）立即执行：

```bash
./target/release/git-helper add-task \
  --name "urgent-review" \
  --task-type git_review \
  --repo-id 1 \
  --prompt "紧急审查最近 3 天的所有变更，重点排查安全漏洞"
```

### 场景四：Web 界面使用流程

```
1. 启动服务后访问 http://127.0.0.1:3100

2. 首次使用 → 点击「激活账号」
   - 输入你的 Git 提交邮箱（系统从 git log 自动发现）
   - 设置登录密码
   - 激活成功后自动登录

3. 登录后进入消息中心
   - 左侧：消息列表（按时间倒序）
   - 右侧：选中消息的审核报告全文（Markdown 渲染）
   - 未读消息左侧有紫色圆点标识
   - 支持「全部 / 未读」筛选

4. 后续每次审核完成，你会自动收到新消息
```

### 场景五：前端开发模式

如果需要修改前端界面：

```bash
# 终端 1：启动后端
cargo run

# 终端 2：启动前端开发服务器（HMR 热更新）
cd web
npm install
npm run dev
# 访问 http://localhost:5173（自动代理 /api 和 /mcp 到后端 :3100）
```

### 场景六：配合 MCP 工具链使用

git-helper 的 MCP 服务可被 Claude Code、Cursor 等 AI 工具直接调用：

```json
// Claude Code 的 MCP 配置示例（.claude/settings.json）
{
  "mcpServers": {
    "git-helper": {
      "url": "http://127.0.0.1:3100/mcp"
    }
  }
}
```

配置后，AI 工具可直接通过自然语言调用 git_log、git_diff 等操作。

### 场景七：生产环境部署

```bash
# 1. 编译 Release 版本
cargo build --release

# 2. 构建前端
cd web && npm ci && npm run build && cd ..

# 3. 准备目录结构
mkdir -p /opt/git-helper/{data,logs,check,tests-generated,repos}
cp target/release/git-helper /opt/git-helper/
cp config.toml /opt/git-helper/
cp -r web/dist /opt/git-helper/web/dist

# 4. 编辑生产配置（务必修改 jwt_secret）
vi /opt/git-helper/config.toml

# 5. 使用 systemd 管理（Linux）
cat > /etc/systemd/system/git-helper.service << 'EOF'
[Unit]
Description=git-helper AI Code Review
After=network.target

[Service]
Type=simple
User=git-helper
WorkingDirectory=/opt/git-helper
ExecStart=/opt/git-helper/git-helper run --config /opt/git-helper/config.toml
# Optional: only set this if you prefer env-based auth instead of `codex login`
# Environment=CODEX_API_KEY=sk-...
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

systemctl enable --now git-helper
```

## 常见问题 (Q&A)

### 编译相关

**Q: 编译报错 `failed to run custom build command for libgit2-sys`**

libgit2 依赖 C 编译器和 cmake。

```bash
# Ubuntu / Debian
sudo apt install build-essential cmake pkg-config libssl-dev

# macOS
xcode-select --install && brew install cmake

# Windows (需要 Visual Studio Build Tools)
# 确保安装了「使用 C++ 的桌面开发」工作负载
```

**Q: 编译报错 `failed to run custom build command for libsqlite3-sys`**

rusqlite 使用 bundled 模式（自带 SQLite 源码编译），需要 C 编译器：

```bash
# 一般安装 build-essential / Xcode CLI Tools 即可解决
# 如果仍然报错，检查 cc 或 cl.exe 是否在 PATH 中
```

**Q: `error[E0658]: edition 2024 is not yet stable`**

需要 Rust >= 1.85。更新工具链：

```bash
rustup update stable
rustc --version  # 确认 >= 1.85
```

### 启动相关

**Q: 启动后立刻退出，日志无输出**

检查 `config.toml` 是否存在语法错误：

```bash
# 验证 TOML 格式
cargo run -- run --config config.toml
# 如果配置错误，会输出具体的解析错误信息
```

**Q: `error: Address already in use (os error 98)` 或 `os error 10048`（Windows）**

端口 3100 被占用：

```bash
# Linux / macOS：查看占用进程
lsof -i :3100

# Windows
netstat -ano | findstr :3100

# 解决：修改 config.toml 中的端口
# [mcp]
# port = 3101
```

**Q: Codex 调用失败，或提示未登录 / 找不到 `CODEX_API_KEY`**

```bash
# 确认 Codex CLI 已安装且可用
codex --version

# 如果未安装：
npm install -g @openai/codex

# 推荐先登录本机 Codex
codex login

# 如果你走环境变量模式，再检查它是否已设置
echo $CODEX_API_KEY

# 也可以在 config.toml 中直接设置
# [codex]
# api_key = "sk-..."
```

**Q: 启动报 `failed to open database` 或 `unable to open database file`**

数据库目录不存在或无写权限：

```bash
# 检查数据库路径配置
grep path config.toml
# [database]
# path = "data/scheduler.db"

# 手动创建目录
mkdir -p data

# 检查权限（Linux）
ls -la data/
```

**Q: 启动报 `failed to initialize tracing subscriber`**

日志目录不存在：

```bash
mkdir -p logs
```

### 任务执行相关

**Q: 任务一直处于 `pending` 状态不执行**

```bash
# 1. 确认服务正在运行
curl http://127.0.0.1:3100/health

# 2. 查看任务状态
./target/release/git-helper list-tasks

# 3. 检查 scheduled_at 是否是未来时间
#    如果是周期任务，需等到下一个 cron 触发时间

# 4. 查看日志
tail -f logs/git-helper.log
```

**Q: 任务执行失败（status = failed）**

```bash
# 查看任务列表，result 字段包含错误信息
./target/release/git-helper list-tasks

# 常见原因：
# 1. Codex API Key 无效或额度用尽
# 2. 仓库 local_path 不存在或无法 git pull
# 3. Codex CLI 超时（默认 300 秒）
# 4. diff 内容过大超出模型 token 限制

# 查看详细日志
grep "task execution failed" logs/git-helper.log
```

**Q: 任务卡在 `running` 状态**

系统自动检测超时任务（默认 300 秒），超时后回退为 `pending` 重新执行。如需手动干预：

```bash
# 重启服务，启动时会自动恢复 stalled 任务
# 日志中会显示 "recovered stalled tasks"
```

**Q: 周期任务停止了，不再自动创建新任务**

确认 `cron_expr` 格式正确（标准 5 段 cron 表达式）：

```
分 时 日 月 周
0  9  *  *  *      # 每天 09:00
0  */2 *  *  *     # 每 2 小时
0  9  *  *  1-5    # 工作日 09:00
*/30 *  *  *  *    # 每 30 分钟
```

### Git 仓库相关

**Q: `failed to clone` 或 `authentication required`**

```bash
# HTTPS 仓库：确保 Git 凭证管理器已配置
git config --global credential.helper store  # 或 manager

# SSH 仓库：确保 SSH key 已添加
ssh -T git@github.com

# 私有仓库：使用 Personal Access Token
# repo_url = "https://<TOKEN>@github.com/team/repo.git"
```

**Q: `git pull` 报 `not a fast-forward`**

git-helper 仅支持 fast-forward 拉取。如果远程分支有 force push 或 rebase：

```bash
# 手动进入仓库目录重置
cd repos/my-project
git fetch origin
git reset --hard origin/main
```

### Web 界面相关

**Q: 访问 `http://127.0.0.1:3100` 显示 404 或空白页**

```bash
# 确认前端已构建
ls web/dist/index.html

# 如果文件不存在，需要构建前端
cd web && npm install && npm run build

# 确认 config.toml 中的 static_dir 路径正确
# [web]
# static_dir = "web/dist"
```

**Q: 登录页面输入邮箱后提示「用户不存在」**

用户需要先被系统自动发现（通过 git_review 任务执行）。手动创建用户：

```bash
./target/release/git-helper add-user \
  --email "dev@example.com" \
  --name "张三"
```

或等待第一次 `git_review` 任务完成，系统会从 git log 中自动提取 commit author 并创建用户。

**Q: 登录后消息列表为空**

消息在 `git_review` 或 `test_gen` 任务完成后自动创建。检查：
1. 是否已添加并执行过任务 — `list-tasks` 查看是否有 `done` 状态的任务
2. 日志中是否有 `"messages synced"` 记录

**Q: 前端开发模式下接口报 CORS 错误**

确保使用 Vite 开发服务器（`npm run dev`，端口 5173），它已配置代理到 `:3100`。不要直接从 5173 向 3100 发跨域请求。

### 通知相关

**Q: 任务完成但没收到通知**

```bash
# 1. 确认 config.toml 中至少有一个 enabled = true 的渠道
grep -A3 "notifier.channels" config.toml

# 2. 检查日志中的通知发送记录
grep -i "notif" logs/git-helper.log

# 3. 企业微信 Webhook 常见问题：
#    - Key 过期（企业微信 Webhook 有效期为创建后永久，但可能被管理员禁用）
#    - IP 不在白名单中

# 4. Telegram Bot 常见问题：
#    - chat_id 需要是字符串格式（群组 ID 带负号）
#    - Bot 未被添加到目标群组
```

**Q: 通知发送失败会影响任务结果吗？**

不会。通知异步发送，失败仅记录日志，不影响任务状态和报告生成。

### 性能相关

**Q: 多个任务同时执行导致系统卡顿**

调整并发数：

```toml
[scheduler]
max_concurrency = 2  # 降低并发数（默认 4）

[codex]
timeout_secs = 600   # 增加超时时间
```

**Q: SQLite 报 `database is locked`**

通常在高并发写入时出现。系统已启用 WAL 模式和 5 秒 busy timeout，一般不会触发。如果频繁出现：

```toml
[scheduler]
max_concurrency = 1   # 降到单任务串行
claim_batch_size = 4  # 减少批量认领数
```

## 设计文档

完整的架构设计、数据库表结构、状态流转、Prompt 设计、测试策略等详见 [codex.md](codex.md)。

## License

MIT
