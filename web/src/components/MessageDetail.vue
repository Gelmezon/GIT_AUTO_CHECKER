<template>
  <section v-if="message" class="detail-panel">
    <div class="detail-header">
      <div>
        <p class="eyebrow">{{ message.repo_name || '任务消息' }}</p>
        <h2>{{ message.title }}</h2>
      </div>
      <div class="detail-actions">
        <button class="ghost-button" type="button" @click="$emit('mark-all-read')">
          全部已读
        </button>
      </div>
    </div>

    <div class="detail-meta">
      <span v-if="message.commit_range">{{ message.commit_range }}</span>
      <span>{{ formatDate(message.created_at) }}</span>
      <span v-if="message.report_path">报告路径: {{ message.report_path }}</span>
    </div>

    <article class="detail-markdown" v-html="rendered" />
  </section>
  <EmptyState
    v-else
    eyebrow="空白面板"
    title="选择一条消息"
    description="左侧列表会展示审核报告和测试生成结果。"
  />
</template>

<script setup lang="ts">
import MarkdownIt from 'markdown-it'
import { computed } from 'vue'

import type { MessageDetail as MessageDetailType } from '../types'
import { formatDateTime } from '../utils/date'
import EmptyState from './EmptyState.vue'

const props = defineProps<{
  message: MessageDetailType | null
}>()

defineEmits<{
  'mark-all-read': []
}>()

const markdown = new MarkdownIt({
  html: false,
  linkify: true,
  typographer: true,
})

const rendered = computed(() => markdown.render(props.message?.content ?? ''))

function formatDate(value: string) {
  return formatDateTime(value)
}
</script>
