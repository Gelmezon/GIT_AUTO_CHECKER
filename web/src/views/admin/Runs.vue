<template>
  <section class="admin-page">
    <section class="admin-card">
      <div class="admin-page-header compact">
        <div>
          <p class="eyebrow">runs</p>
          <h3>全局执行记录</h3>
          <p class="admin-section-copy">跨任务查看执行状态、耗时和结果，适合排查失败与积压。</p>
        </div>
        <div class="admin-toolbar-actions">
          <select v-model="statusFilter" aria-label="按执行状态筛选" @change="reloadFromFirstPage">
            <option value="">全部状态</option>
            <option value="pending">待执行</option>
            <option value="running">执行中</option>
            <option value="done">已完成</option>
            <option value="failed">失败</option>
            <option value="cancelled">已取消</option>
          </select>
          <select v-model="taskFilter" aria-label="按任务筛选" @change="reloadFromFirstPage">
            <option value="">全部任务</option>
            <option v-for="task in taskOptions" :key="task.id" :value="String(task.id)">
              {{ task.name }}
            </option>
          </select>
          <button class="ghost-button" type="button" :disabled="loading" @click="load">
            刷新
          </button>
        </div>
      </div>

      <p v-if="error" class="form-error">{{ error }}</p>
      <div v-if="loading && runs.length === 0" class="loading-panel">正在加载执行记录...</div>

      <div v-else-if="runs.length === 0">
        <EmptyState
          eyebrow="暂无执行"
          title="还没有运行记录"
          description="任务执行后，这里会集中展示所有实例。"
        />
      </div>
      <template v-else>
        <div class="admin-table-wrap">
          <table class="admin-table" aria-label="执行记录列表">
            <thead>
              <tr>
                <th>任务</th>
                <th>仓库</th>
                <th>状态</th>
                <th>计划时间</th>
                <th>开始时间</th>
                <th>耗时</th>
                <th>结果</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="run in runs" :key="run.id">
                <td>
                  <RouterLink class="table-title-link" :to="`/admin/tasks/${run.task_id}`">
                    {{ run.task_name }}
                  </RouterLink>
                </td>
                <td>{{ run.repo_name || '-' }}</td>
                <td>
                  <span class="status-chip" :class="run.status">
                    {{ TASK_RUN_STATUS_LABELS[run.status] }}
                  </span>
                </td>
                <td>{{ formatDateTime(run.scheduled_at) }}</td>
                <td>{{ formatDateTime(run.started_at) }}</td>
                <td>{{ formatDuration(run.started_at, run.finished_at) }}</td>
                <td class="truncate-cell">{{ run.result || '-' }}</td>
                <td>
                  <div class="actions-cell">
                    <RouterLink class="ghost-button" :to="`/admin/runs/${run.id}`">
                      详情
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
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'

import EmptyState from '../../components/EmptyState.vue'
import * as adminApi from '../../api/admin'
import { useUiStore } from '../../stores/ui'
import type { AdminTask, AdminTaskRun, TaskRunStatus } from '../../types'
import { formatDateTime, formatDuration } from '../../utils/date'
import { TASK_RUN_STATUS_LABELS } from '../../utils/tasks'

const ui = useUiStore()
const taskOptions = ref<AdminTask[]>([])
const runs = ref<AdminTaskRun[]>([])
const loading = ref(false)
const error = ref('')
const statusFilter = ref<TaskRunStatus | ''>('')
const taskFilter = ref('')
const page = ref(1)
const pageSize = 20
const total = ref(0)

const totalPages = computed(() => Math.max(1, Math.ceil(total.value / pageSize)))

onMounted(async () => {
  const response = await adminApi.fetchTasks({ page: 1, page_size: 200 })
  taskOptions.value = response.items
  await load()
})

async function load() {
  loading.value = true
  error.value = ''
  try {
    const response = await adminApi.fetchRuns({
      status: statusFilter.value,
      task_id: taskFilter.value ? Number(taskFilter.value) : null,
      page: page.value,
      page_size: pageSize,
    })
    runs.value = response.items
    total.value = response.total
  } catch (err) {
    error.value = err instanceof Error ? err.message : '加载执行记录失败'
  } finally {
    loading.value = false
  }
}

async function cancelPending(runId: number) {
  try {
    await adminApi.cancelRun(runId)
    ui.toast('warning', '执行已取消')
    await load()
  } catch (err) {
    error.value = err instanceof Error ? err.message : '取消执行失败'
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
