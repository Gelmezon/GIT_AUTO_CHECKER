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
import { formatDateTime } from '../utils/date'

defineProps<{
  item: MessageListItem
  selected: boolean
}>()

defineEmits<{
  select: [id: number]
}>()

function formatDate(value: string) {
  return formatDateTime(value)
}
</script>
