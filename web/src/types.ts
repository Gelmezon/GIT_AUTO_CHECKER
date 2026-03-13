export type UserRole = 'superAdmin' | 'user'
export type TaskType = 'git_review' | 'test_gen' | 'custom'
export type TaskDefinitionStatus = 'active' | 'paused'
export type TaskRunStatus = 'pending' | 'running' | 'done' | 'failed' | 'cancelled'
export type TaskStatus = TaskRunStatus
export type GitPlatform = 'github' | 'gitee' | 'gitlab' | 'other'
export type GitAuthType = 'token' | 'ssh' | 'basic'

export interface User {
  id: number
  email: string
  display_name: string
  avatar_url: string | null
  role: UserRole
}

export interface AuthResponse {
  token: string
  user: User
}

export interface MessageListItem {
  id: number
  title: string
  repo_name: string | null
  summary: string
  commit_range: string | null
  is_read: boolean
  created_at: string
}

export interface MessageDetail {
  id: number
  title: string
  repo_name: string | null
  content: string
  report_path: string | null
  commit_range: string | null
  is_read: boolean
  created_at: string
}

export interface MessageListResponse {
  total: number
  unread_count: number
  page: number
  page_size: number
  items: MessageListItem[]
}

export interface AdminRepo {
  id: number
  name: string
  repo_url: string
  branch: string
  local_path: string
  review_cron: string | null
  credential_id: number | null
  credential_name: string | null
  last_commit: string | null
  enabled: boolean
  created_at: string
  updated_at: string
}

export interface AdminCredential {
  id: number
  name: string
  platform: GitPlatform
  auth_type: GitAuthType
  username: string | null
  ssh_key_path: string | null
  has_token: boolean
  has_password: boolean
  created_at: string
  updated_at: string
}

export interface AdminUser {
  id: number
  email: string
  display_name: string
  avatar_url: string | null
  activated_at: string | null
  created_at: string
  updated_at: string
}

export interface AdminTask {
  id: number
  name: string
  task_type: TaskType
  repo_id: number | null
  repo_name: string | null
  prompt: string
  cron_expr: string | null
  status: TaskDefinitionStatus
  last_run_at: string | null
  last_run_status: TaskRunStatus | null
  next_run_at: string | null
  total_runs: number
  created_at: string
  updated_at: string
}

export interface AdminTaskRun {
  id: number
  task_id: number
  task_name: string
  repo_name: string | null
  scheduled_at: string
  started_at: string | null
  finished_at: string | null
  status: TaskRunStatus
  result: string | null
  log: string | null
  retry_count: number
  created_at: string
}

export interface AdminDashboard {
  repo_count: number
  task_count: number
  user_count: number
  today_executed_count: number
  recent_runs: AdminTaskRun[]
}

export interface AdminTaskListResponse {
  total: number
  page: number
  page_size: number
  items: AdminTask[]
}

export interface AdminTaskRunListResponse {
  total: number
  page: number
  page_size: number
  items: AdminTaskRun[]
}

export interface RepoSyncResponse {
  action: string
  branch: string
  updated: boolean
  head: string | null
}
