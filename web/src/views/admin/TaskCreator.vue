<template>
  <section class="admin-page">
    <section class="admin-card">
      <div class="admin-page-header compact">
        <div>
          <p class="eyebrow">tasks</p>
          <h3>新增任务</h3>
          <p class="admin-section-copy">支持 `git_review`、`test_gen` 和 `custom` 三种任务。</p>
        </div>
        <RouterLink class="ghost-button" to="/admin/tasks">返回列表</RouterLink>
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
          <input v-model.trim="form.cron_expr" placeholder="例如 0 */1 * * 1-5" />
        </label>
        <label>
          <span>计划时间</span>
          <input v-model.trim="form.scheduled_at" placeholder="可留空，由系统自动计算或立即执行" />
        </label>
        <label class="admin-form-full">
          <span>Prompt</span>
          <textarea v-model.trim="form.prompt" rows="8" required />
        </label>
        <button class="primary-button" :disabled="saving" type="submit">
          {{ saving ? '创建中...' : '创建任务' }}
        </button>
      </form>

      <p class="admin-hint">
        Cron 现在支持标准 5 段写法，例如 `0 */1 * * 1-5` 表示工作日每小时执行一次。
      </p>
      <p v-if="error" class="form-error">{{ error }}</p>
    </section>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { useRouter } from 'vue-router'

import * as adminApi from '../../api/admin'
import type { AdminRepo, TaskType } from '../../types'

interface TaskFormState {
  name: string
  task_type: TaskType
  repo_id: string
  prompt: string
  cron_expr: string
  scheduled_at: string
}

const router = useRouter()
const repos = ref<AdminRepo[]>([])
const saving = ref(false)
const error = ref('')
const form = reactive<TaskFormState>({
  name: '',
  task_type: 'git_review',
  repo_id: '',
  prompt: '',
  cron_expr: '',
  scheduled_at: '',
})

const requiresRepo = computed(() => form.task_type !== 'custom')

onMounted(loadRepos)

async function loadRepos() {
  try {
    repos.value = await adminApi.fetchRepos()
  } catch (err) {
    error.value = err instanceof Error ? err.message : '加载仓库失败'
  }
}

async function submit() {
  saving.value = true
  error.value = ''
  try {
    await adminApi.createTask({
      name: form.name,
      task_type: form.task_type,
      repo_id: requiresRepo.value && form.repo_id ? Number(form.repo_id) : null,
      prompt: form.prompt,
      cron_expr: form.cron_expr.trim() || null,
      scheduled_at: form.scheduled_at.trim() || null,
    })
    await router.push('/admin/tasks')
  } catch (err) {
    error.value = err instanceof Error ? err.message : '创建任务失败'
  } finally {
    saving.value = false
  }
}
</script>
