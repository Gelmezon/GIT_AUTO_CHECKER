export interface User {
  id: number
  email: string
  display_name: string
  avatar_url: string | null
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
