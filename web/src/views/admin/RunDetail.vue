<template>
  <section class="admin-page" v-if="run">
    <section class="admin-card">
      <div class="admin-page-header compact">
        <div>
          <p class="eyebrow">run detail</p>
          <h3>{{ run.task_name }}</h3>
          <p class="admin-section-copy">单次执行的完整时间线与输出日志。</p>
        </div>
        <div class="admin-toolbar-actions">
          <RouterLink class="ghost-button" :to="`/admin/tasks/${run.task_id}`">
            返回任务
          </RouterLink>
          <button
            v-if="run.status === 'pending'"
            class="ghost-button danger-button"
            type="button"
            @click="cancelPending"
          >
            取消执行
          </button>
        </div>
      </div>

      <div class="task-meta-grid">
        <article class="task-meta-card">
          <span>状态</span>
          <strong>{{ TASK_RUN_STATUS_LABELS[run.status] }}</strong>
        </article>
        <article class="task-meta-card">
          <span>计划时间</span>
          <strong>{{ formatDateTime(run.scheduled_at) }}</strong>
        </article>
        <article class="task-meta-card">
          <span>开始时间</span>
          <strong>{{ formatDateTime(run.started_at) }}</strong>
        </article>
        <article class="task-meta-card">
          <span>耗时</span>
          <strong>{{ formatDuration(run.started_at, run.finished_at) }}</strong>
        </article>
      </div>

      <div class="task-detail-grid">
        <article class="admin-card inset-card">
          <h4>结果摘要</h4>
          <p class="detail-copy">{{ run.result || '暂无结构化结果。' }}</p>
        </article>

        <article class="admin-card inset-card">
          <h4>执行日志</h4>
          <pre class="run-log">{{ run.log || '暂无日志输出。' }}</pre>
        </article>
      </div>
    </section>
  </section>

  <div v-else class="loading-panel">正在加载执行详情...</div>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useRoute } from 'vue-router'

import * as adminApi from '../../api/admin'
import { useUiStore } from '../../stores/ui'
import type { AdminTaskRun } from '../../types'
import { formatDateTime, formatDuration } from '../../utils/date'
import { TASK_RUN_STATUS_LABELS } from '../../utils/tasks'

const route = useRoute()
const ui = useUiStore()
const run = ref<AdminTaskRun | null>(null)

onMounted(load)

async function load() {
  run.value = await adminApi.fetchRun(Number(route.params.runId))
}

async function cancelPending() {
  if (!run.value) return
  run.value = await adminApi.cancelRun(run.value.id)
  ui.toast('warning', '执行已取消')
}
</script>
