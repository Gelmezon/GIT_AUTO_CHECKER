import { computed, ref } from 'vue'
import { defineStore } from 'pinia'

import * as messageApi from '../api/messages'
import type { MessageDetail, MessageListItem } from '../types'

export const useMessagesStore = defineStore('messages', () => {
  const items = ref<MessageListItem[]>([])
  const detail = ref<MessageDetail | null>(null)
  const total = ref(0)
  const unreadCount = ref(0)
  const page = ref(1)
  const pageSize = ref(20)
  const unreadOnly = ref(false)
  const loadingList = ref(false)
  const loadingDetail = ref(false)

  const totalPages = computed(() => Math.max(1, Math.ceil(total.value / pageSize.value)))

  async function loadList() {
    loadingList.value = true
    try {
      const response = await messageApi.fetchMessages({
        unread: unreadOnly.value,
        page: page.value,
        page_size: pageSize.value,
      })
      items.value = response.items
      total.value = response.total
      unreadCount.value = response.unread_count
    } finally {
      loadingList.value = false
    }
  }

  async function loadDetail(id: number) {
    loadingDetail.value = true
    try {
      const wasUnread = items.value.find((item) => item.id === id)?.is_read === false
      detail.value = await messageApi.fetchMessage(id)
      items.value = items.value.map((item) =>
        item.id === id ? { ...item, is_read: true } : item,
      )
      if (wasUnread) {
        unreadCount.value = Math.max(0, unreadCount.value - 1)
      }
    } finally {
      loadingDetail.value = false
    }
  }

  async function markAllRead() {
    await messageApi.markAllMessagesRead()
    items.value = items.value.map((item) => ({ ...item, is_read: true }))
    unreadCount.value = 0
    if (detail.value) {
      detail.value = { ...detail.value, is_read: true }
    }
  }

  return {
    items,
    detail,
    total,
    unreadCount,
    page,
    pageSize,
    unreadOnly,
    totalPages,
    loadingList,
    loadingDetail,
    loadList,
    loadDetail,
    markAllRead,
  }
})
