import type { MessageDetail, MessageListResponse } from '../types'
import { request } from './client'

export function fetchMessages(params: {
  unread?: boolean
  page?: number
  page_size?: number
}) {
  const search = new URLSearchParams()
  if (params.unread) {
    search.set('unread', 'true')
  }
  search.set('page', String(params.page ?? 1))
  search.set('page_size', String(params.page_size ?? 20))
  return request<MessageListResponse>(`/api/messages?${search.toString()}`)
}

export function fetchMessage(id: number) {
  return request<MessageDetail>(`/api/messages/${id}`)
}

export function markMessageRead(id: number) {
  return request<void>(`/api/messages/${id}/read`, {
    method: 'PUT',
  })
}

export function markAllMessagesRead() {
  return request<{ updated: number }>('/api/messages/read-all', {
    method: 'PUT',
  })
}

export function fetchUnreadCount() {
  return request<{ unread_count: number }>('/api/messages/unread-count')
}
