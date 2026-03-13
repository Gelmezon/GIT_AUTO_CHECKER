import type {
  AdminDashboard,
  AdminRepo,
  AdminTask,
  AdminTaskListResponse,
  AdminUser,
  RepoSyncResponse,
  TaskStatus,
  TaskType,
} from '../types'
import { request } from './client'

export interface RepoPayload {
  name: string
  repo_url: string
  branch: string
  local_path: string
  review_cron: string | null
  enabled: boolean
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
  status?: TaskStatus | ''
  page?: number
  page_size?: number
}

export function fetchDashboard() {
  return request<AdminDashboard>('/api/admin/dashboard')
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

export function deleteTask(id: number) {
  return request<void>(`/api/admin/tasks/${id}`, {
    method: 'DELETE',
  })
}
