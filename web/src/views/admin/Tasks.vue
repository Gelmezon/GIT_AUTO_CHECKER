<template>
  <section class="admin-page">
    <section class="admin-card">
      <div class="admin-page-header compact">
        <div>
          <p class="eyebrow">tasks</p>
          <h3>任务列表</h3>
          <p class="admin-section-copy">列表页只负责查看和筛选，新增任务进入独立页面。</p>
        </div>
        <div class="admin-toolbar-actions">
          <RouterLink class="primary-button" to="/admin/tasks/new">新增任务</RouterLink>
          <select v-model="statusFilter" @change="reloadFromFirstPage">
            <option value="">全部状态</option>
            <option value="pending">pending</option>
            <option value="running">running</option>
            <option value="done">done</option>
            <option value="failed">failed</option>
          </select>
          <button class="ghost-button" type="button" :disabled="loading" @click="load">
            刷新
          </button>
        </div>
      </div>

      <p v-if="error" class="form-error">{{ error }}</p>
      <p v-if="notice" class="admin-notice">{{ notice }}</p>

      <div v-if="loading && items.length === 0" class="loading-panel">正在加载任务...</div>
      <div v-else-if="items.length === 0">
        <EmptyState
          eyebrow="暂无任务"
          title="当前没有任务"
          description="点击右上角“新增任务”即可创建 git_review、test_gen 或 custom 任务。"
        />
      </div>
      <template v-else>
        <div class="admin-table-wrap">
          <table class="admin-table">
            <thead>
              <tr>
                <th>任务</th>
                <th>类型</th>
                <th>仓库</th>
                <th>Cron</th>
                <th>状态</th>
                <th>计划时间</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="task in items" :key="task.id">
                <td>
                  <strong>{{ task.name }}</strong>
                  <p class="table-subline">{{ task.prompt }}</p>
                </td>
                <td>{{ task.task_type }}</td>
                <td>{{ task.repo_name || '-' }}</td>
                <td>{{ task.cron_expr || '-' }}</td>
                <td>
                  <span class="status-chip" :class="task.status">{{ task.status }}</span>
                </td>
                <td>{{ formatDate(task.scheduled_at) }}</td>
                <td>
                  <div class="actions-cell">
                    <button class="ghost-button danger-button" type="button" @click="remove(task.id)">
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
import { computed, onMounted, ref } from 'vue'

import EmptyState from '../../components/EmptyState.vue'
import * as adminApi from '../../api/admin'
import type { AdminTask, TaskStatus } from '../../types'

const items = ref<AdminTask[]>([])
const loading = ref(false)
const error = ref('')
const notice = ref('')
const page = ref(1)
const pageSize = 20
const total = ref(0)
const statusFilter = ref<TaskStatus | ''>('')

const totalPages = computed(() => Math.max(1, Math.ceil(total.value / pageSize)))

onMounted(load)

async function load() {
  loading.value = true
  error.value = ''
  try {
    const response = await adminApi.fetchTasks({
      status: statusFilter.value,
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

async function remove(id: number) {
  if (!window.confirm('确认删除这个任务吗？')) {
    return
  }
  notice.value = ''
  error.value = ''
  try {
    await adminApi.deleteTask(id)
    notice.value = '任务已删除'
    if (items.value.length === 1 && page.value > 1) {
      page.value -= 1
    }
    await load()
  } catch (err) {
    error.value = err instanceof Error ? err.message : '删除任务失败'
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

function formatDate(value: string) {
  return new Date(value).toLocaleString()
}
</script>
