<template>
  <section class="admin-page" v-if="task">
    <section class="admin-card task-hero">
      <div class="admin-page-header compact">
        <div>
          <p class="eyebrow">task detail</p>
          <h3>{{ task.name }}</h3>
          <p class="admin-section-copy">{{ task.prompt }}</p>
        </div>
        <div class="admin-toolbar-actions">
          <RouterLink class="ghost-button" :to="`/admin/tasks/${task.id}/edit`">编辑</RouterLink>
          <button class="ghost-button" type="button" @click="triggerTask">手动触发</button>
          <button class="ghost-button" type="button" @click="toggleTask">
            {{ task.status === 'active' ? '暂停调度' : '恢复调度' }}
          </button>
          <button class="ghost-button danger-button" type="button" @click="removeTask">
            删除
          </button>
        </div>
      </div>

      <div class="task-meta-grid">
        <article class="task-meta-card">
          <span>任务类型</span>
          <strong>{{ TASK_TYPE_LABELS[task.task_type] }}</strong>
        </article>
        <article class="task-meta-card">
          <span>调度状态</span>
          <strong>{{ TASK_DEFINITION_STATUS_LABELS[task.status] }}</strong>
        </article>
        <article class="task-meta-card">
          <span>关联仓库</span>
          <strong>{{ task.repo_name || '-' }}</strong>
        </article>
        <article class="task-meta-card">
          <span>下次执行</span>
          <strong>{{ formatDateTime(task.next_run_at) }}</strong>
        </article>
      </div>

      <div class="task-meta-grid">
        <article class="task-meta-card">
          <span>Cron</span>
          <strong>{{ task.cron_expr || '单次任务' }}</strong>
          <small>{{ describeCron(task.cron_expr) }}</small>
        </article>
        <article class="task-meta-card">
          <span>最近执行</span>
          <strong>{{ formatDateTime(task.last_run_at) }}</strong>
          <small>{{ task.last_run_status ? TASK_RUN_STATUS_LABELS[task.last_run_status] : '暂无' }}</small>
        </article>
        <article class="task-meta-card">
          <span>累计执行</span>
          <strong>{{ task.total_runs }}</strong>
        </article>
        <article class="task-meta-card">
          <span>更新时间</span>
          <strong>{{ formatDateTime(task.updated_at) }}</strong>
        </article>
      </div>
    </section>

    <section class="admin-card">
      <div class="admin-page-header compact">
        <div>
          <p class="eyebrow">task runs</p>
          <h3>执行历史</h3>
        </div>
        <RouterLink class="ghost-button" to="/admin/runs">查看全部执行</RouterLink>
      </div>

      <p v-if="error" class="form-error">{{ error }}</p>

      <div v-if="loading && runs.length === 0" class="loading-panel">正在加载执行历史...</div>
      <div v-else-if="runs.length === 0">
        <EmptyState
          eyebrow="暂无运行"
          title="这个任务还没有执行记录"
          description="你可以先手动触发一次，或者等待调度器按计划执行。"
        />
      </div>
      <template v-else>
        <div class="admin-table-wrap">
          <table class="admin-table" aria-label="任务执行历史">
            <thead>
              <tr>
                <th>计划时间</th>
                <th>开始时间</th>
                <th>耗时</th>
                <th>状态</th>
                <th>结果摘要</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="run in runs" :key="run.id">
                <td>{{ formatDateTime(run.scheduled_at) }}</td>
                <td>{{ formatDateTime(run.started_at) }}</td>
                <td>{{ formatDuration(run.started_at, run.finished_at) }}</td>
                <td>
                  <span class="status-chip" :class="run.status">
                    {{ TASK_RUN_STATUS_LABELS[run.status] }}
                  </span>
                </td>
                <td class="truncate-cell">{{ run.result || '-' }}</td>
                <td>
                  <div class="actions-cell">
                    <RouterLink class="ghost-button" :to="`/admin/runs/${run.id}`">
                      查看日志
                    </RouterLink>
                    <button
                      v-if="run.status === 'pending'"
                      class="ghost-button danger-button"
                      type="button"
                      @click="cancelPending(run.id)"
                    >
                      取消
                    </button>
                  </div>
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <footer class="pagination-row admin-pagination">
          <button class="ghost-button" type="button" :disabled="page <= 1" @click="prevPage">
            上一页
          </button>
          <span>第 {{ page }} / {{ totalPages }} 页</span>
          <button class="ghost-button" type="button" :disabled="page >= totalPages" @click="nextPage">
            下一页
          </button>
        </footer>
      </template>
    </section>
  </section>

  <div v-else class="loading-panel">正在加载任务详情...</div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import EmptyState from '../../components/EmptyState.vue'
import * as adminApi from '../../api/admin'
import { useUiStore } from '../../stores/ui'
import type { AdminTask, AdminTaskRun } from '../../types'
import { formatDateTime, formatDuration } from '../../utils/date'
import {
  describeCron,
  TASK_DEFINITION_STATUS_LABELS,
  TASK_RUN_STATUS_LABELS,
  TASK_TYPE_LABELS,
} from '../../utils/tasks'

const route = useRoute()
const router = useRouter()
const ui = useUiStore()
const task = ref<AdminTask | null>(null)
const runs = ref<AdminTaskRun[]>([])
const loading = ref(false)
const error = ref('')
const page = ref(1)
const pageSize = 12
const total = ref(0)

const totalPages = computed(() => Math.max(1, Math.ceil(total.value / pageSize)))
const taskId = computed(() => Number(route.params.id))

onMounted(async () => {
  await Promise.all([loadTask(), loadRuns()])
})

async function loadTask() {
  task.value = await adminApi.fetchTask(taskId.value)
}

async function loadRuns() {
  loading.value = true
  error.value = ''
  try {
    const response = await adminApi.fetchTaskRuns(taskId.value, page.value, pageSize)
    runs.value = response.items
    total.value = response.total
  } catch (err) {
    error.value = err instanceof Error ? err.message : '加载执行历史失败'
  } finally {
    loading.value = false
  }
}

async function triggerTask() {
  try {
    await adminApi.triggerTask(taskId.value)
    ui.toast('success', '任务已触发', '新的执行实例已经排队。')
    await Promise.all([loadTask(), loadRuns()])
  } catch (err) {
    error.value = err instanceof Error ? err.message : '触发任务失败'
    ui.toast('error', '触发失败', error.value)
  }
}

async function toggleTask() {
  if (!task.value) return
  try {
    task.value =
      task.value.status === 'active'
        ? await adminApi.pauseTask(taskId.value)
        : await adminApi.resumeTask(taskId.value)
    ui.toast(
      task.value.status === 'active' ? 'success' : 'warning',
      task.value.status === 'active' ? '任务已恢复' : '任务已暂停',
    )
    await loadRuns()
  } catch (err) {
    error.value = err instanceof Error ? err.message : '更新任务状态失败'
  }
}

async function removeTask() {
  if (!task.value) return
  const confirmed = await ui.confirm({
    title: '删除当前任务？',
    description: `删除后会一并移除「${task.value.name}」的执行记录。`,
    confirmText: '确认删除',
    cancelText: '先不删',
    tone: 'danger',
  })
  if (!confirmed) return

  try {
    await adminApi.deleteTask(taskId.value)
    ui.toast('success', '任务已删除')
    await router.push('/admin/tasks')
  } catch (err) {
    error.value = err instanceof Error ? err.message : '删除任务失败'
    ui.toast('error', '删除失败', error.value)
  }
}

async function cancelPending(runId: number) {
  try {
    await adminApi.cancelRun(runId)
    ui.toast('warning', '执行已取消')
    await Promise.all([loadTask(), loadRuns()])
  } catch (err) {
    error.value = err instanceof Error ? err.message : '取消执行失败'
  }
}

async function prevPage() {
  if (page.value <= 1) return
  page.value -= 1
  await loadRuns()
}

async function nextPage() {
  if (page.value >= totalPages.value) return
  page.value += 1
  await loadRuns()
}
</script>
