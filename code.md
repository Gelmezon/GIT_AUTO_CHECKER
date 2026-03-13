# 任务管理系统优化设计

## 一、现状问题

当前 `tasks` 表同时承担"任务定义"和"执行实例"两个职责：
- cron 任务每次执行完后插入新行作为下次调度，导致表中大量重复任务行
- 无法区分"暂停一个周期性任务"和"取消一次执行"
- 执行日志存储在 `result` 字段中，无法追溯历史执行记录
- 删除任务会连带丢失所有执行历史

## 二、核心模型拆分

### 2.1 任务定义表 `task_definitions`

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER PK | 自增主键 |
| name | TEXT NOT NULL | 任务名称（唯一标识） |
| task_type | TEXT NOT NULL | 任务类型：git_review / test_gen / custom |
| repo_id | INTEGER NULL | 关联仓库 FK → git_repos(id) |
| cron_expr | TEXT NULL | cron 调度表达式，NULL 表示一次性任务 |
| prompt | TEXT NOT NULL | 任务 Prompt 指令 |
| status | TEXT NOT NULL DEFAULT 'active' | active（运行中）/ paused（暂停） |
| created_at | TEXT NOT NULL | 创建时间 |
| updated_at | TEXT NOT NULL | 更新时间 |

**说明：**
- 一个任务定义只有一行记录，不会因为多次执行而膨胀
- `status` 仅控制调度开关：`active` 时调度器会按 cron 生成执行实例，`paused` 时停止调度
- 一次性任务（cron_expr 为 NULL）创建后立即生成一个执行实例，执行完成后自动将 status 设为 paused
- 支持软删除：删除任务定义时标记为已删除，保留历史执行记录可查

### 2.2 任务执行实例表 `task_runs`

| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER PK | 自增主键 |
| task_id | INTEGER NOT NULL | FK → task_definitions(id) |
| scheduled_at | TEXT NOT NULL | 计划执行时间 |
| started_at | TEXT NULL | 实际开始时间 |
| finished_at | TEXT NULL | 实际结束时间 |
| status | TEXT NOT NULL DEFAULT 'pending' | pending / running / done / failed / cancelled |
| result | TEXT NULL | 执行结果摘要 |
| log | TEXT NULL | 详细执行日志 |
| retry_count | INTEGER NOT NULL DEFAULT 0 | 重试次数 |
| created_at | TEXT NOT NULL | 记录创建时间 |

**说明：**
- 每次调度触发生成一条 `task_runs` 记录，status 初始为 `pending`
- `log` 字段存储完整执行日志（标准输出 + 错误输出），`result` 存储结构化结果摘要
- 暂停任务时，已生成但未执行的实例标记为 `cancelled`
- 支持按 task_id 查询某个任务的所有历史执行记录

## 三、状态机

### 3.1 任务定义状态

```
创建 → active ⇄ paused → 删除
```

- `active`：调度器正常调度
- `paused`：调度器跳过，不生成新的执行实例
- 切换状态时不影响已存在的执行实例

### 3.2 执行实例状态

```
pending → running → done
                  → failed
pending → cancelled（任务暂停/删除时）
```

- `pending`：等待执行，调度时间未到或排队中
- `running`：正在执行
- `done`：执行成功完成
- `failed`：执行失败（含超时）
- `cancelled`：被取消（任务暂停或手动取消）

## 四、调度器逻辑变更

### 4.1 调度循环（每 interval_secs 执行一次）

```
1. 扫描 task_definitions WHERE status = 'active' AND cron_expr IS NOT NULL
2. 对每个任务，检查是否需要生成新的执行实例：
   - 根据 cron_expr 计算下一次执行时间
   - 如果该时间点不存在对应的 task_runs 记录，则插入一条 pending 实例
   - 避免重复生成：检查是否已有 pending/running 状态的实例
3. 扫描 task_runs WHERE status = 'pending' AND scheduled_at <= now
4. 按 claim_batch_size 批量认领，更新为 running
5. 分发到执行器
```

### 4.2 执行完成回调

```
1. 更新 task_runs：status = done/failed, finished_at = now, result, log
2. 发送通知、创建消息（与现有逻辑一致）
3. 不再需要"插入下一条任务"——调度循环会自动处理
```

### 4.3 任务暂停处理

```
1. 更新 task_definitions.status = 'paused'
2. 将该任务所有 pending 状态的 task_runs 标记为 cancelled
3. 不影响 running 状态的实例（让其自然完成）
```

### 4.4 任务恢复处理

```
1. 更新 task_definitions.status = 'active'
2. 调度循环下一轮自动生成新的 pending 实例
```

### 4.5 超时恢复

```
启动时扫描 task_runs WHERE status = 'running' AND started_at < now - timeout
→ 标记为 failed，log 记录 "execution timeout on recovery"
```

## 五、API 接口设计

### 5.1 任务定义 CRUD

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | /api/admin/tasks | 任务定义列表（支持 status/task_type 筛选 + 分页） |
| POST | /api/admin/tasks | 创建任务定义 |
| GET | /api/admin/tasks/{id} | 获取单个任务定义详情 |
| PUT | /api/admin/tasks/{id} | 更新任务定义（名称、prompt、cron 等） |
| DELETE | /api/admin/tasks/{id} | 删除任务定义 |
| POST | /api/admin/tasks/{id}/pause | 暂停任务 |
| POST | /api/admin/tasks/{id}/resume | 恢复任务 |
| POST | /api/admin/tasks/{id}/trigger | 手动触发一次执行（立即生成 pending 实例） |

### 5.2 执行实例查询

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | /api/admin/tasks/{id}/runs | 某任务的执行实例列表（分页，按时间倒序） |
| GET | /api/admin/runs/{run_id} | 单个执行实例详情（含完整日志） |
| POST | /api/admin/runs/{run_id}/cancel | 取消 pending 状态的实例 |
| GET | /api/admin/runs | 全局执行实例列表（支持 status/task_id 筛选 + 分页） |

### 5.3 请求/响应结构

**CreateTaskRequest：**
```json
{
  "name": "每日代码审查",
  "task_type": "git_review",
  "repo_id": 1,
  "prompt": "审查最近24小时的提交...",
  "cron_expr": "0 9 * * 1-5"
}
```

**TaskDefinitionResponse：**
```json
{
  "id": 1,
  "name": "每日代码审查",
  "task_type": "git_review",
  "repo_id": 1,
  "repo_name": "my-project",
  "prompt": "审查最近24小时的提交...",
  "cron_expr": "0 9 * * 1-5",
  "status": "active",
  "last_run_at": "2026-03-13T09:00:00Z",
  "last_run_status": "done",
  "next_run_at": "2026-03-14T09:00:00Z",
  "total_runs": 42,
  "created_at": "2026-01-01T00:00:00Z",
  "updated_at": "2026-03-13T09:00:05Z"
}
```

**TaskRunResponse：**
```json
{
  "id": 100,
  "task_id": 1,
  "task_name": "每日代码审查",
  "scheduled_at": "2026-03-13T09:00:00Z",
  "started_at": "2026-03-13T09:00:01Z",
  "finished_at": "2026-03-13T09:02:30Z",
  "status": "done",
  "result": "发现3个潜在问题",
  "log": "完整执行日志...",
  "retry_count": 0,
  "created_at": "2026-03-13T08:55:00Z"
}
```

## 六、前端页面设计

### 6.1 路由结构

```
/admin/tasks              → 任务定义列表页
/admin/tasks/new          → 创建任务
/admin/tasks/:id          → 任务详情（含执行历史）
/admin/tasks/:id/edit     → 编辑任务
/admin/runs               → 全局执行实例列表
/admin/runs/:runId        → 执行实例详情（含日志）
```

### 6.2 任务列表页 `/admin/tasks`

**筛选栏：**
- 状态筛选：全部 / 运行中(active) / 已暂停(paused)
- 类型筛选：全部 / git_review / test_gen / custom

**表格列：**
| 列 | 说明 |
|----|------|
| 任务名称 | 可点击进入详情 |
| 任务类型 | 标签样式展示 |
| 关联项目 | 仓库名称，custom 类型显示 "-" |
| Cron 表达式 | 显示表达式 + 人类可读描述（如"工作日每天9点"） |
| 状态 | active 绿色 / paused 灰色 |
| 上次执行 | 时间 + 状态标签 |
| 下次执行 | 预计时间 |
| 操作 | 暂停/恢复、手动触发、编辑、删除 |

### 6.3 任务详情页 `/admin/tasks/:id`

**上半部分 - 任务信息卡片：**
- 任务名称、类型、关联仓库、Cron 表达式、Prompt 内容、状态
- 操作按钮：编辑、暂停/恢复、手动触发、删除

**下半部分 - 执行历史列表：**
| 列 | 说明 |
|----|------|
| 执行时间 | scheduled_at |
| 开始时间 | started_at |
| 耗时 | finished_at - started_at |
| 状态 | pending 蓝色 / running 黄色 / done 绿色 / failed 红色 / cancelled 灰色 |
| 结果摘要 | result 字段截断显示 |
| 操作 | 查看日志、取消（仅 pending） |

### 6.4 执行实例详情页 `/admin/runs/:runId`

- 基本信息：所属任务、计划时间、实际时间、耗时、状态
- 结果摘要
- 完整执行日志（代码块样式，支持滚动）

### 6.5 全局执行实例页 `/admin/runs`

跨任务查看所有执行实例，支持：
- 按状态筛选：全部 / pending / running / done / failed / cancelled
- 按任务筛选：下拉选择任务名称
- 时间范围筛选
- 分页

## 七、数据库迁移策略

### 7.1 迁移步骤

```sql
-- 1. 创建新表
CREATE TABLE task_definitions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    task_type TEXT NOT NULL,
    repo_id INTEGER,
    cron_expr TEXT,
    prompt TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(repo_id) REFERENCES git_repos(id)
);

CREATE TABLE task_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id INTEGER NOT NULL,
    scheduled_at TEXT NOT NULL,
    started_at TEXT,
    finished_at TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    result TEXT,
    log TEXT,
    retry_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(task_id) REFERENCES task_definitions(id)
);

CREATE INDEX idx_task_definitions_status ON task_definitions(status);
CREATE INDEX idx_task_runs_task_id ON task_runs(task_id);
CREATE INDEX idx_task_runs_status_scheduled ON task_runs(status, scheduled_at);

-- 2. 迁移现有数据
-- 从 tasks 表中提取唯一任务定义（按 name + task_type + repo_id + prompt 去重）
INSERT INTO task_definitions (name, task_type, repo_id, cron_expr, prompt, status, created_at, updated_at)
SELECT DISTINCT name, task_type, repo_id, cron_expr, prompt, 'active',
       MIN(created_at), MAX(updated_at)
FROM tasks
GROUP BY name, task_type, repo_id, prompt;

-- 将原有执行记录迁移为 task_runs
INSERT INTO task_runs (task_id, scheduled_at, started_at, finished_at, status, result, retry_count, created_at)
SELECT td.id, t.scheduled_at, t.started_at, t.updated_at,
       CASE t.status WHEN 'done' THEN 'done' WHEN 'failed' THEN 'failed'
            WHEN 'running' THEN 'running' ELSE 'pending' END,
       t.result, t.retry_count, t.created_at
FROM tasks t
JOIN task_definitions td ON t.name = td.name AND t.task_type = td.task_type
     AND COALESCE(t.repo_id, -1) = COALESCE(td.repo_id, -1);

-- 3. 备份并删除旧表
ALTER TABLE tasks RENAME TO tasks_backup;
```

## 八、Rust 类型定义变更

### 8.1 新增枚举

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskDefinitionStatus {
    Active,  // "active"
    Paused,  // "paused"
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskRunStatus {
    Pending,    // "pending"
    Running,    // "running"
    Done,       // "done"
    Failed,     // "failed"
    Cancelled,  // "cancelled"
}
```

### 8.2 新增结构体

```rust
pub struct TaskDefinition {
    pub id: i64,
    pub name: String,
    pub task_type: TaskType,
    pub repo_id: Option<i64>,
    pub cron_expr: Option<String>,
    pub prompt: String,
    pub status: TaskDefinitionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct TaskRun {
    pub id: i64,
    pub task_id: i64,
    pub scheduled_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: TaskRunStatus,
    pub result: Option<String>,
    pub log: Option<String>,
    pub retry_count: i64,
    pub created_at: DateTime<Utc>,
}
```

## 九、前端 TypeScript 类型变更

```typescript
type TaskDefinitionStatus = 'active' | 'paused'
type TaskRunStatus = 'pending' | 'running' | 'done' | 'failed' | 'cancelled'

interface TaskDefinition {
  id: number
  name: string
  task_type: TaskType
  repo_id: number | null
  repo_name: string | null
  cron_expr: string | null
  prompt: string
  status: TaskDefinitionStatus
  last_run_at: string | null
  last_run_status: TaskRunStatus | null
  next_run_at: string | null
  total_runs: number
  created_at: string
  updated_at: string
}

interface TaskRun {
  id: number
  task_id: number
  task_name: string
  scheduled_at: string
  started_at: string | null
  finished_at: string | null
  status: TaskRunStatus
  result: string | null
  log: string | null
  retry_count: number
  created_at: string
}
```
