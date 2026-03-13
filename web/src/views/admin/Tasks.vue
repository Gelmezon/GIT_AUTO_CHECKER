<template>
  <section class="admin-page">
    <section class="admin-card">
      <div class="admin-page-header compact">
        <div>
          <p class="eyebrow">tasks</p>
          <h3>任务定义</h3>
          <p class="admin-section-copy">按任务类型、调度状态和最近执行情况管理整套自动化流程。</p>
        </div>
        <div class="admin-toolbar-actions">
          <RouterLink class="primary-button" to="/admin/tasks/new">新增任务</RouterLink>
          <select v-model="statusFilter" aria-label="按状态筛选" @change="reloadFromFirstPage">
            <option value="">全部状态</option>
            <option value="active">运行中</option>
            <option value="paused">已暂停</option>
          </select>
          <select v-model="typeFilter" aria-label="按类型筛选" @change="reloadFromFirstPage">
            <option value="">全部类型</option>
            <option value="git_review">代码审查</option>
            <option value="test_gen">测试生成</option>
            <option value="custom">自定义</option>
          </select>
          <button class="ghost-button" type="button" :disabled="loading" @click="load">
            刷新
          </button>
        </div>
      </div>

      <p v-if="error" class="form-error">{{ error }}</p>

      <div v-if="loading && items.length === 0" class="loading-panel">正在加载任务...</div>
      <div v-else-if="items.length === 0">
        <EmptyState
          eyebrow="暂无任务"
          title="还没有任务定义"
          description="点击右上角“新增任务”后，我们就可以开始编排代码审查或测试生成任务。"
        />
      </div>
      <template v-else>
        <div class="admin-table-wrap">
          <table class="admin-table" aria-label="任务定义列表">
            <thead>
              <tr>
                <th>任务</th>
                <th>类型</th>
                <th>仓库</th>
                <th>Cron</th>
                <th>状态</th>
                <th>上次执行</th>
                <th>下次执行</th>
                <th>总次数</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="task in items" :key="task.id">
                <td>
                  <RouterLink class="table-title-link" :to="`/admin/tasks/${task.id}`">
                    {{ task.name }}
                  </RouterLink>
                  <p class="table-subline">{{ task.prompt }}</p>
                </td>
                <td>{{ TASK_TYPE_LABELS[task.task_type] }}</td>
                <td>{{ task.repo_name || '-' }}</td>
                <td>
                  <strong>{{ task.cron_expr || '单次任务' }}</strong>
                  <p class="table-subline">{{ describeCron(task.cron_expr) }}</p>
                </td>
                <td>
                  <span class="status-chip" :class="task.status">
                    {{ TASK_DEFINITION_STATUS_LABELS[task.status] }}
                  </span>
                </td>
                <td>
                  <template v-if="task.last_run_at">
                    <span>{{ formatDateTime(task.last_run_at) }}</span>
                    <p class="table-subline">
                      {{ task.last_run_status ? TASK_RUN_STATUS_LABELS[task.last_run_status] : '-' }}
                    </p>
                  </template>
                  <span v-else>-</span>
                </td>
                <td>
                  <template v-if="task.next_run_at">
                    <span>{{ formatDateTime(task.next_run_at) }}</span>
                    <p class="table-subline">{{ formatRelativeTime(task.next_run_at) }}</p>
                  </template>
                  <span v-else>-</span>
                </td>
                <td>{{ task.total_runs }}</td>
                <td>
                  <div class="actions-cell">
                    <RouterLink class="ghost-button" :to="`/admin/tasks/${task.id}`">
                      详情
                    </RouterLink>
                    <RouterLink class="ghost-button" :to="`/admin/tasks/${task.id}/edit`">
                      编辑
                    </RouterLink>
                    <button class="ghost-button" type="button" @click="trigger(task.id)">
                      触发
                    </button>
                    <button
                      class="ghost-button"
                      type="button"
                      @click="toggleTask(task)"
                    >
                      {{ task.status === 'active' ? '暂停' : '恢复' }}
                    </button>
                    <button
                      class="ghost-button danger-button"
                      type="button"
                      @click="remove(task)"
                    >
                      删除
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
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'

import EmptyState from '../../components/EmptyState.vue'
import * as adminApi from '../../api/admin'
import { useUiStore } from '../../stores/ui'
import type { AdminTask, TaskDefinitionStatus, TaskType } from '../../types'
import { formatDateTime, formatRelativeTime } from '../../utils/date'
import {
  describeCron,
  TASK_DEFINITION_STATUS_LABELS,
  TASK_RUN_STATUS_LABELS,
  TASK_TYPE_LABELS,
} from '../../utils/tasks'

const ui = useUiStore()
const items = ref<AdminTask[]>([])
const loading = ref(false)
const error = ref('')
const page = ref(1)
const pageSize = 12
const total = ref(0)
const statusFilter = ref<TaskDefinitionStatus | ''>('')
const typeFilter = ref<TaskType | ''>('')

let refreshTimer: number | undefined

const totalPages = computed(() => Math.max(1, Math.ceil(total.value / pageSize)))

onMounted(() => {
  void load()
  refreshTimer = window.setInterval(() => {
    void load(true)
  }, 30_000)
})

onBeforeUnmount(() => {
  if (refreshTimer) {
    window.clearInterval(refreshTimer)
  }
})

async function load(silent = false) {
  if (!silent) {
    loading.value = true
  }
  error.value = ''
  try {
    const response = await adminApi.fetchTasks({
      status: statusFilter.value,
      task_type: typeFilter.value,
      page: page.value,
      page_size: pageSize,
    })
    items.value = response.items
    total.value = response.total
  } catch (err) {
    error.value = err instanceof Error ? err.message : '加载任务失败'
  } finally {
    loading.value = false
  }
}

async function trigger(id: number) {
  try {
    await adminApi.triggerTask(id)
    ui.toast('success', '任务已触发', '新的执行实例已经进入队列。')
    await load(true)
  } catch (err) {
    error.value = err instanceof Error ? err.message : '触发任务失败'
    ui.toast('error', '触发失败', error.value)
  }
}

async function toggleTask(task: AdminTask) {
  try {
    if (task.status === 'active') {
      await adminApi.pauseTask(task.id)
      ui.toast('warning', '任务已暂停', `已停止继续调度「${task.name}」`)
    } else {
      await adminApi.resumeTask(task.id)
      ui.toast('success', '任务已恢复', `「${task.name}」会重新参与调度`)
    }
    await load(true)
  } catch (err) {
    error.value = err instanceof Error ? err.message : '更新任务状态失败'
    ui.toast('error', '状态更新失败', error.value)
  }
}

async function remove(task: AdminTask) {
  const confirmed = await ui.confirm({
    title: '删除任务定义？',
    description: `删除后将移除「${task.name}」以及关联的执行记录，这个操作无法撤销。`,
    confirmText: '确认删除',
    cancelText: '先保留',
    tone: 'danger',
  })
  if (!confirmed) return

  try {
    await adminApi.deleteTask(task.id)
    ui.toast('success', '任务已删除', `「${task.name}」已从调度列表移除。`)
    if (items.value.length === 1 && page.value > 1) {
      page.value -= 1
    }
    await load(true)
  } catch (err) {
    error.value = err instanceof Error ? err.message : '删除任务失败'
    ui.toast('error', '删除失败', error.value)
  }
}

async function reloadFromFirstPage() {
  page.value = 1
  await load()
}

async function prevPage() {
  if (page.value <= 1) return
  page.value -= 1
  await load()
}

async function nextPage() {
  if (page.value >= totalPages.value) return
  page.value += 1
  await load()
}
</script>
