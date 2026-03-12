# git-helper

基于定时器 + SQLite 的 AI 调度系统，自动拉取 Git 仓库并通过 OpenAI Codex 进行代码审查，输出结构化审核报告。

## 它能做什么

1. **定时拉取代码** — 按 Cron 表达式周期性 `git pull` 你配置的仓库
2. **AI 代码审查** — 将新增 diff 发送给 Codex (`gpt-5.4`)，获取质量评估、Bug 检测、安全风险和改进建议
3. **输出审核报告** — 报告以 Markdown 文件写入 `check/{repo_name}/{date}-review.md`
4. **只读安全** — 系统只拉取和读取代码，**绝不 commit、push 或修改源码**
5. **MCP 服务** — 对外暴露标准化的 Git 只读操作，供其他 AI 工具调用

## 系统架构

```
┌──────────────────────────────────────────────────────────┐
│                     git-helper 进程                       │
│                                                          │
│   Scheduler (1s tick)                                    │
│       │                                                  │
│       ▼                                                  │
│   SQLite DB ──▸ Dispatcher ──▸ Codex (codex exec)       │
│   (tasks / git_repos)              │                     │
│                                    ▼                     │
│                              check/ 报告                 │
│                                                          │
│   Git MCP Server (axum, :3100)                           │
│   Tools: clone / pull / log / diff / status              │
└──────────────────────────────────────────────────────────┘
```

## 前置依赖

| 依赖 | 版本 | 说明 |
|------|------|------|
| Rust | >= 1.75 | 编译工具链 |
| OpenAI Codex CLI | 最新 | `npm install -g @openai/codex` |
| SQLite | 系统自带 | 通过 `rusqlite` 静态链接，无需单独安装 |
| Git | >= 2.0 | `git2` (libgit2) 不依赖系统 git，但克隆 SSH 仓库需要系统 git 配置 |

## 快速开始

### 1. 克隆项目

```bash
git clone https://github.com/yourname/git-helper.git
cd git-helper
```

### 2. 配置环境变量

```bash
# 二选一
export CODEX_API_KEY="sk-..."
export OPENAI_API_KEY="sk-..."
```

### 3. 编辑配置文件

```bash
cp config.example.toml config.toml
```

编辑 `config.toml`：

```toml
[scheduler]
interval_secs = 1
task_timeout_secs = 300

[codex]
model = "gpt-5.4"
max_retries = 2
timeout_secs = 300

[mcp]
host = "127.0.0.1"
port = 3100

[log]
level = "info"
file = "logs/git-helper.log"
```

### 4. 构建并运行

```bash
cargo build --release
cargo run
```

### 5. 添加要审查的仓库

通过 CLI 子命令添加仓库：

```bash
# 添加一个每天 9 点审查的仓库
cargo run -- repo add \
  --name "my-project" \
  --url "https://github.com/user/my-project.git" \
  --branch main \
  --local-path "./repos/my-project" \
  --cron "0 9 * * *"
```

### 6. 查看审核报告

```
check/
  └── my-project/
      └── 2026-03-12-review.md
```

## CLI 命令

```bash
git-helper                        # 启动调度服务 + MCP 服务
git-helper repo add [OPTIONS]     # 添加仓库
git-helper repo list              # 列出所有仓库
git-helper repo remove <name>     # 移除仓库
git-helper task list              # 查看任务状态
git-helper task trigger <id>      # 手动触发一个任务
```

## MCP 服务

启动后默认监听 `127.0.0.1:3100`，暴露以下只读 Tool：

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

## 审核报告内容

每份报告包含：

- 审核时间与 commit 范围 (`from..to`)
- 变更文件列表及统计
- 代码质量评估
- 潜在 Bug 与逻辑问题
- 安全风险提示
- 改进建议

## 目录结构

```
git-helper/
├── README.md               # 本文件
├── codex.md                # 详细设计文档
├── Cargo.toml
├── config.toml             # 运行时配置
├── src/
│   ├── main.rs             # 入口
│   ├── config.rs           # 配置加载
│   ├── error.rs            # 错误类型
│   ├── db/                 # 数据库层
│   │   ├── mod.rs
│   │   ├── models.rs
│   │   ├── tasks.rs
│   │   └── repos.rs
│   ├── scheduler/          # 调度引擎
│   │   ├── mod.rs
│   │   └── dispatcher.rs
│   ├── executor/           # Codex 执行器
│   │   └── codex.rs
│   ├── mcp/                # MCP 服务
│   │   ├── mod.rs
│   │   ├── protocol.rs
│   │   └── tools/
│   │       └── git.rs
│   └── jobs/               # 业务任务
│       └── git_review.rs
├── check/                  # 审核报告输出
├── data/                   # SQLite 数据库
├── logs/                   # 日志
└── tests/                  # 集成测试
```

## 技术栈

| 组件 | 选型 |
|------|------|
| 语言 | Rust |
| 异步运行时 | tokio |
| 数据库 | SQLite (rusqlite, WAL 模式) |
| HTTP 服务 | axum |
| HTTP 客户端 | reqwest |
| Git 操作 | git2 (libgit2) |
| AI 引擎 | OpenAI Codex CLI (`codex exec`) |
| 日志 | tracing |
| CLI | clap |

## 开发

```bash
# 运行测试
cargo test

# 运行并查看详细日志
RUST_LOG=debug cargo run

# 仅启动 MCP 服务（不启动调度器）
cargo run -- --mcp-only
```

## 设计文档

完整的架构设计、数据库表结构、状态流转、测试策略等详见 [codex.md](codex.md)。

## License

MIT
