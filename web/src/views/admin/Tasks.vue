<template>
  <section class="admin-page">
    <section class="admin-card">
      <div class="admin-page-header compact">
        <div>
          <p class="eyebrow">tasks</p>
          <h3>新增任务</h3>
        </div>
      </div>

      <form class="admin-form-grid" @submit.prevent="submit">
        <label>
          <span>任务名称</span>
          <input v-model.trim="form.name" required />
        </label>
        <label>
          <span>任务类型</span>
          <select v-model="form.task_type">
            <option value="git_review">git_review</option>
            <option value="test_gen">test_gen</option>
            <option value="custom">custom</option>
          </select>
        </label>
        <label>
          <span>关联项目</span>
          <select v-model="form.repo_id" :disabled="!requiresRepo">
            <option value="">不关联项目</option>
            <option v-for="repo in repos" :key="repo.id" :value="String(repo.id)">
              {{ repo.name }}
            </option>
          </select>
        </label>
        <label>
          <span>Cron 表达式</span>
          <input v-model.trim="form.cron_expr" placeholder="留空则立即执行一次" />
        </label>
        <label>
          <span>计划时间</span>
          <input
            v-model.trim="form.scheduled_at"
            placeholder="RFC3339，可留空由系统自动计算"
          />
        </label>
        <label class="admin-form-full">
          <span>Prompt</span>
          <textarea v-model.trim="form.prompt" rows="6" required />
        </label>
        <button class="primary-button" :disabled="saving" type="submit">
          {{ saving ? '创建中...' : '创建任务' }}
        </button>
      </form>

      <p v-if="error" class="form-error">{{ error }}</p>
      <p v-if="notice" class="admin-notice">{{ notice }}</p>
    </section>

    <section class="admin-card">
      <div class="admin-page-header compact">
        <div>
          <p class="eyebrow">list</p>
          <h3>任务列表</h3>
        </div>
        <div class="admin-toolbar-actions">
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

      <div v-if="loading && items.length === 0" class="loading-panel">正在加载任务...</div>
      <div v-else-if="items.length === 0">
        <EmptyState
          eyebrow="暂无任务"
          title="当前没有任务"
          description="你可以创建定时审查、测试生成或自定义任务。"
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
                    <button
                      class="ghost-button danger-button"
                      type="button"
                      @click="remove(task.id)"
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
import { computed, onMounted, reactive, ref } from 'vue'

import EmptyState from '../../components/EmptyState.vue'
import * as adminApi from '../../api/admin'
import type { AdminRepo, AdminTask, TaskStatus, TaskType } from '../../types'

interface TaskFormState {
  name: string
  task_type: TaskType
  repo_id: string
  prompt: string
  cron_expr: string
  scheduled_at: string
}

const repos = ref<AdminRepo[]>([])
const items = ref<AdminTask[]>([])
const loading = ref(false)
const saving = ref(false)
const error = ref('')
const notice = ref('')
const page = ref(1)
const pageSize = 20
const total = ref(0)
const statusFilter = ref<TaskStatus | ''>('')
const form = reactive<TaskFormState>(createEmptyForm())

const totalPages = computed(() => Math.max(1, Math.ceil(total.value / pageSize)))
const requiresRepo = computed(() => form.task_type !== 'custom')

onMounted(async () => {
  await loadRepos()
  await load()
})

async function loadRepos() {
  repos.value = await adminApi.fetchRepos()
}

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

async function submit() {
  saving.value = true
  error.value = ''
  notice.value = ''
  try {
    await adminApi.createTask({
      name: form.name,
      task_type: form.task_type,
      repo_id: requiresRepo.value && form.repo_id ? Number(form.repo_id) : null,
      prompt: form.prompt,
      cron_expr: form.cron_expr.trim() || null,
      scheduled_at: form.scheduled_at.trim() || null,
    })
    notice.value = '任务已创建'
    Object.assign(form, createEmptyForm())
    page.value = 1
    await load()
  } catch (err) {
    error.value = err instanceof Error ? err.message : '创建任务失败'
  } finally {
    saving.value = false
  }
}

async function remove(id: number) {
  if (!window.confirm('确认删除这个任务吗？')) {
    return
  }
  error.value = ''
  notice.value = ''
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

function createEmptyForm(): TaskFormState {
  return {
    name: '',
    task_type: 'git_review',
    repo_id: '',
    prompt: '',
    cron_expr: '',
    scheduled_at: '',
  }
}

function formatDate(value: string) {
  return new Date(value).toLocaleString()
}
</script>
