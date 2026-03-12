<template>
  <button
    type="button"
    class="message-card"
    :class="{ selected, unread: !item.is_read }"
    @click="$emit('select', item.id)"
  >
    <div class="message-card-row">
      <p>{{ item.title }}</p>
      <span>{{ formatDate(item.created_at) }}</span>
    </div>
    <p class="message-card-meta">
      <span v-if="item.repo_name">{{ item.repo_name }}</span>
      <span v-if="item.commit_range">{{ item.commit_range }}</span>
    </p>
    <p class="message-card-summary">{{ item.summary }}</p>
  </button>
</template>

<script setup lang="ts">
import type { MessageListItem } from '../types'

defineProps<{
  item: MessageListItem
  selected: boolean
}>()

defineEmits<{
  select: [id: number]
}>()

function formatDate(value: string) {
  return new Intl.DateTimeFormat('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(value))
}
</script>
