import type {
  AdminCredential,
  AdminDashboard,
  AdminRepo,
  AdminTask,
  AdminTaskListResponse,
  AdminTaskRun,
  AdminTaskRunListResponse,
  AdminUser,
  GitAuthType,
  GitPlatform,
  RepoSyncResponse,
  TaskDefinitionStatus,
  TaskRunStatus,
  TaskType,
} from '../types'
import { request } from './client'

export interface RepoPayload {
  name: string
  repo_url: string
  branch: string
  local_path: string
  review_cron: string | null
  credential_id: number | null
  enabled: boolean
}

export interface CredentialPayload {
  name: string
  platform: GitPlatform
  auth_type: GitAuthType
  token: string | null
  username: string | null
  password: string | null
  ssh_key_path: string | null
}

export interface UserPayload {
  email: string
  display_name: string
}

export interface TaskPayload {
  name: string
  task_type: TaskType
  repo_id: number | null
  prompt: string
  cron_expr: string | null
  scheduled_at: string | null
}

export interface TaskListParams {
  status?: TaskDefinitionStatus | ''
  task_type?: TaskType | ''
  page?: number
  page_size?: number
}

export interface TaskRunListParams {
  status?: TaskRunStatus | ''
  task_id?: number | null
  page?: number
  page_size?: number
}

export function fetchDashboard() {
  return request<AdminDashboard>('/api/admin/dashboard')
}

export function fetchCredentials() {
  return request<AdminCredential[]>('/api/admin/credentials')
}

export function fetchCredential(id: number) {
  return request<AdminCredential>(`/api/admin/credentials/${id}`)
}

export function createCredential(payload: CredentialPayload) {
  return request<AdminCredential>('/api/admin/credentials', {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}

export function updateCredential(id: number, payload: CredentialPayload) {
  return request<AdminCredential>(`/api/admin/credentials/${id}`, {
    method: 'PUT',
    body: JSON.stringify(payload),
  })
}

export function deleteCredential(id: number) {
  return request<void>(`/api/admin/credentials/${id}`, {
    method: 'DELETE',
  })
}

export function fetchRepos() {
  return request<AdminRepo[]>('/api/admin/repos')
}

export function fetchRepo(id: number) {
  return request<AdminRepo>(`/api/admin/repos/${id}`)
}

export function createRepo(payload: RepoPayload) {
  return request<AdminRepo>('/api/admin/repos', {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}

export function updateRepo(id: number, payload: RepoPayload) {
  return request<AdminRepo>(`/api/admin/repos/${id}`, {
    method: 'PUT',
    body: JSON.stringify(payload),
  })
}

export function deleteRepo(id: number) {
  return request<void>(`/api/admin/repos/${id}`, {
    method: 'DELETE',
  })
}

export function syncRepo(id: number) {
  return request<RepoSyncResponse>(`/api/admin/repos/${id}/sync`, {
    method: 'POST',
  })
}

export function fetchUsers() {
  return request<AdminUser[]>('/api/admin/users')
}

export function fetchUser(id: number) {
  return request<AdminUser>(`/api/admin/users/${id}`)
}

export function createUser(payload: UserPayload) {
  return request<AdminUser>('/api/admin/users', {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}

export function updateUser(id: number, payload: UserPayload) {
  return request<AdminUser>(`/api/admin/users/${id}`, {
    method: 'PUT',
    body: JSON.stringify(payload),
  })
}

export function deleteUser(id: number) {
  return request<void>(`/api/admin/users/${id}`, {
    method: 'DELETE',
  })
}

export function fetchTasks(params: TaskListParams = {}) {
  const search = new URLSearchParams()
  if (params.status) {
    search.set('status', params.status)
  }
  if (params.task_type) {
    search.set('task_type', params.task_type)
  }
  if (params.page) {
    search.set('page', String(params.page))
  }
  if (params.page_size) {
    search.set('page_size', String(params.page_size))
  }
  const query = search.toString()
  return request<AdminTaskListResponse>(`/api/admin/tasks${query ? `?${query}` : ''}`)
}

export function createTask(payload: TaskPayload) {
  return request<AdminTask>('/api/admin/tasks', {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}

export function fetchTask(id: number) {
  return request<AdminTask>(`/api/admin/tasks/${id}`)
}

export function updateTask(id: number, payload: TaskPayload) {
  return request<AdminTask>(`/api/admin/tasks/${id}`, {
    method: 'PUT',
    body: JSON.stringify(payload),
  })
}

export function pauseTask(id: number) {
  return request<AdminTask>(`/api/admin/tasks/${id}/pause`, {
    method: 'POST',
  })
}

export function resumeTask(id: number) {
  return request<AdminTask>(`/api/admin/tasks/${id}/resume`, {
    method: 'POST',
  })
}

export function triggerTask(id: number) {
  return request<AdminTaskRun>(`/api/admin/tasks/${id}/trigger`, {
    method: 'POST',
  })
}

export function fetchTaskRuns(id: number, page = 1, page_size = 20) {
  const search = new URLSearchParams({
    page: String(page),
    page_size: String(page_size),
  })
  return request<AdminTaskRunListResponse>(`/api/admin/tasks/${id}/runs?${search.toString()}`)
}

export function fetchRuns(params: TaskRunListParams = {}) {
  const search = new URLSearchParams()
  if (params.status) {
    search.set('status', params.status)
  }
  if (params.task_id) {
    search.set('task_id', String(params.task_id))
  }
  if (params.page) {
    search.set('page', String(params.page))
  }
  if (params.page_size) {
    search.set('page_size', String(params.page_size))
  }
  const query = search.toString()
  return request<AdminTaskRunListResponse>(`/api/admin/runs${query ? `?${query}` : ''}`)
}

export function fetchRun(id: number) {
  return request<AdminTaskRun>(`/api/admin/runs/${id}`)
}

export function cancelRun(id: number) {
  return request<AdminTaskRun>(`/api/admin/runs/${id}/cancel`, {
    method: 'POST',
  })
}

export function deleteTask(id: number) {
  return request<void>(`/api/admin/tasks/${id}`, {
    method: 'DELETE',
  })
}
