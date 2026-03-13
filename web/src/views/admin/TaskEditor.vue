<template>
  <section class="admin-page">
    <section class="admin-card">
      <div class="admin-page-header compact">
        <div>
          <p class="eyebrow">tasks</p>
          <h3>{{ isEditing ? '编辑任务' : '新增任务' }}</h3>
          <p class="admin-section-copy">支持代码审查、测试生成与自定义任务，任务创建后会自动生成首个执行实例。</p>
        </div>
        <RouterLink class="ghost-button" to="/admin/tasks">返回列表</RouterLink>
      </div>

      <form v-if="ready" class="admin-form-grid" @submit.prevent="submit">
        <label>
          <span>任务名称</span>
          <input v-model.trim="form.name" required placeholder="例如：工作日代码审查" />
        </label>

        <label>
          <span>任务类型</span>
          <select v-model="form.task_type">
            <option value="git_review">代码审查</option>
            <option value="test_gen">测试生成</option>
            <option value="custom">自定义</option>
          </select>
        </label>

        <label>
          <span>关联仓库</span>
          <select v-model="repoIdValue" :disabled="!needsRepo">
            <option value="">不关联仓库</option>
            <option v-for="repo in repos" :key="repo.id" :value="String(repo.id)">
              {{ repo.name }}
            </option>
          </select>
        </label>

        <label>
          <span>Cron 表达式</span>
          <input
            v-model.trim="form.cron_expr"
            placeholder="例如 0 9 * * 1-5"
          />
        </label>

        <label>
          <span>首次计划时间</span>
          <input v-model="form.scheduled_at" type="datetime-local" />
        </label>

        <label>
          <span>预估说明</span>
          <input
            :value="describeCron(form.cron_expr || null)"
            disabled
          />
        </label>

        <label class="admin-form-full">
          <span>Prompt</span>
          <textarea v-model.trim="form.prompt" rows="10" required />
          <small class="field-counter">{{ form.prompt.length }} 字</small>
        </label>

        <div class="admin-inline-actions admin-form-full">
          <button class="primary-button" :disabled="saving" type="submit">
            {{ saving ? '保存中...' : isEditing ? '保存变更' : '创建任务' }}
          </button>
          <RouterLink class="ghost-button" to="/admin/tasks">取消</RouterLink>
        </div>
      </form>

      <div v-else class="loading-panel">正在准备任务表单...</div>
      <p class="admin-hint">Cron 使用标准 5 段写法；如果留空，则任务只执行一次。</p>
      <p v-if="error" class="form-error">{{ error }}</p>
    </section>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import * as adminApi from '../../api/admin'
import { useUiStore } from '../../stores/ui'
import type { AdminRepo, TaskType } from '../../types'
import { dateTimeLocalToIso, toDateTimeLocalValue } from '../../utils/date'
import { describeCron, requiresRepo } from '../../utils/tasks'

interface TaskFormState {
  name: string
  task_type: TaskType
  repo_id: number | null
  prompt: string
  cron_expr: string
  scheduled_at: string
}

const route = useRoute()
const router = useRouter()
const ui = useUiStore()
const repos = ref<AdminRepo[]>([])
const ready = ref(false)
const saving = ref(false)
const error = ref('')
const form = reactive<TaskFormState>({
  name: '',
  task_type: 'git_review',
  repo_id: null,
  prompt: '',
  cron_expr: '',
  scheduled_at: '',
})

const isEditing = computed(() => Boolean(route.params.id))
const needsRepo = computed(() => requiresRepo(form.task_type))
const repoIdValue = computed({
  get: () => (form.repo_id ? String(form.repo_id) : ''),
  set: (value: string) => {
    form.repo_id = value ? Number(value) : null
  },
})

onMounted(async () => {
  try {
    repos.value = await adminApi.fetchRepos()
    if (isEditing.value) {
      const task = await adminApi.fetchTask(Number(route.params.id))
      form.name = task.name
      form.task_type = task.task_type
      form.repo_id = task.repo_id
      form.prompt = task.prompt
      form.cron_expr = task.cron_expr ?? ''
      form.scheduled_at = toDateTimeLocalValue(task.next_run_at ?? task.last_run_at)
    }
    ready.value = true
  } catch (err) {
    error.value = err instanceof Error ? err.message : '加载任务失败'
  }
})

async function submit() {
  if (needsRepo.value && !form.repo_id) {
    error.value = '当前任务类型必须绑定仓库'
    return
  }

  saving.value = true
  error.value = ''
  const payload = {
    name: form.name,
    task_type: form.task_type,
    repo_id: needsRepo.value ? form.repo_id : null,
    prompt: form.prompt,
    cron_expr: form.cron_expr.trim() || null,
    scheduled_at: dateTimeLocalToIso(form.scheduled_at),
  }

  try {
    if (isEditing.value) {
      await adminApi.updateTask(Number(route.params.id), payload)
      ui.toast('success', '任务已更新', '新的调度信息已经保存。')
      await router.push(`/admin/tasks/${route.params.id}`)
    } else {
      const task = await adminApi.createTask(payload)
      ui.toast('success', '任务已创建', '首个执行实例已经进入调度队列。')
      await router.push(`/admin/tasks/${task.id}`)
    }
  } catch (err) {
    error.value = err instanceof Error ? err.message : '保存任务失败'
    ui.toast('error', '保存失败', error.value)
  } finally {
    saving.value = false
  }
}
</script>
