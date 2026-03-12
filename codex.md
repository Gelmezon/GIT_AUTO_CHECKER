# AI 调度系统 — 项目设计文档

## 一、项目核心需求

构建一个以 **定时器 + 数据库** 为核心的 AI 调度系统，具备以下能力：

1. **定时扫描引擎** — 每 1 秒扫描数据库，发现到期任务并派发执行。
2. **Prompt 数据库** — 持久化存储待执行的 AI Prompt、调度时间、执行状态等元数据。
3. **Codex 执行器** — 扫描到到期任务后，调用 OpenAI Codex（模型 `gpt-5.4`）执行 Prompt。
4. **Git 管理 MCP 服务** — 以 MCP（Model Context Protocol）协议暴露 Git 操作能力，在本机监听端口，供调度系统及其他 AI 工具调用。
5. **首个业务功能：定时 Git 审核** — 定时拉取配置的 Git 项目，使用 Codex 进行代码审核，将审核报告输出到 `check/` 目录。

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
┌─────────────────────────────────────────────────────────────────────┐
│                         git-helper 进程                             │
│                                                                     │
│  ┌─────────────┐   1s tick   ┌──────────────┐                      │
│  │  Scheduler   │ ─────────▸ │  SQLite DB    │                      │
│  │  (tokio)     │            │  tasks        │                      │
│  └──────┬──────┘            │  git_repos    │                      │
│         │                    └──────┬───────┘                      │
│         │ spawn task                │                               │
│         ▼                           ▼                               │
│  ┌─────────────┐            ┌──────────────┐   ┌────────────────┐  │
│  │  Dispatcher  │ ─────────▸│ Codex 执行器  │──▸│ check/ 报告    │  │
│  │  (并发控制)  │            │ (gpt-5.4)    │   └────────────────┘  │
│  └─────────────┘            └──────────────┘                       │
│                                                                     │
│  ┌──────────────────────────────────────┐                          │
│  │  Git MCP Server (axum, :3100)        │                          │
│  │  Tools: clone/pull/log/diff/status   │                          │
│  └──────────────────────────────────────┘                          │
└─────────────────────────────────────────────────────────────────────┘
        ▲ HTTP/SSE
        │
   外部 AI 工具 / curl / MCP Inspector
```

**关键决策**：Scheduler、Dispatcher、MCP Server 运行在同一进程的不同 tokio task 中，共享数据库连接池。单进程部署简单，SQLite 通过 WAL 模式支持并发读。

---

## 五、模块设计

### 5.1 配置管理（`config.toml`）

```toml
[scheduler]
interval_secs = 1
task_timeout_secs = 300        # running 超时阈值

[codex]
api_key = "sk-..."             # 或通过环境变量 OPENAI_API_KEY
model = "gpt-5.4"
max_retries = 2
timeout_secs = 300

[mcp]
host = "127.0.0.1"
port = 3100

[log]
level = "info"                 # trace / debug / info / warn / error
file = "logs/git-helper.log"
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
# 必须设置 API Key（codex exec 支持此环境变量）
export CODEX_API_KEY="sk-..."

# 或者使用 OPENAI_API_KEY（codex 也认）
export OPENAI_API_KEY="sk-..."
```

Rust 侧在启动时检查：

```rust
fn validate_codex_env() -> Result<()> {
    if std::env::var("CODEX_API_KEY").is_err()
        && std::env::var("OPENAI_API_KEY").is_err()
    {
        bail!("必须设置 CODEX_API_KEY 或 OPENAI_API_KEY 环境变量");
    }
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
| 认证 | `CODEX_API_KEY` 或 `OPENAI_API_KEY` 环境变量 |

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
