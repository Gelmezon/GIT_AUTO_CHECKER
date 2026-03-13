<template>
  <section class="admin-page">
    <section class="admin-card">
      <div class="admin-page-header compact">
        <div>
          <p class="eyebrow">repos</p>
          <h3>{{ isEdit ? '编辑项目' : '新增项目' }}</h3>
          <p class="admin-section-copy">
            {{ isEdit ? '修改已有仓库配置。' : '填写仓库地址、分支和本地路径后保存。' }}
          </p>
        </div>
        <RouterLink class="ghost-button" to="/admin/repos">返回列表</RouterLink>
      </div>

      <div v-if="loading" class="loading-panel">正在加载项目...</div>
      <form v-else class="admin-form-grid" @submit.prevent="submit">
        <label>
          <span>项目名称</span>
          <input v-model.trim="form.name" required />
        </label>
        <label>
          <span>仓库地址</span>
          <input v-model.trim="form.repo_url" required />
        </label>
        <label>
          <span>分支</span>
          <input v-model.trim="form.branch" required />
        </label>
        <label>
          <span>本地路径</span>
          <input v-model.trim="form.local_path" required />
        </label>
        <label>
          <span>Cron 表达式</span>
          <input v-model.trim="form.review_cron" placeholder="例如 0 9 * * 1-5" />
        </label>
        <label class="checkbox-field">
          <span>启用状态</span>
          <div>
            <input v-model="form.enabled" type="checkbox" />
            <span>启用项目</span>
          </div>
        </label>
        <button class="primary-button" :disabled="saving" type="submit">
          {{ saving ? '提交中...' : isEdit ? '保存修改' : '创建项目' }}
        </button>
      </form>

      <p class="admin-hint">支持标准 5 段 Cron，例如 `0 */1 * * 1-5`。</p>
      <p v-if="error" class="form-error">{{ error }}</p>
    </section>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import * as adminApi from '../../api/admin'

interface RepoFormState {
  name: string
  repo_url: string
  branch: string
  local_path: string
  review_cron: string
  enabled: boolean
}

const route = useRoute()
const router = useRouter()
const isEdit = computed(() => Boolean(route.params.id))
const loading = ref(false)
const saving = ref(false)
const error = ref('')
const form = reactive<RepoFormState>({
  name: '',
  repo_url: '',
  branch: 'main',
  local_path: '',
  review_cron: '',
  enabled: true,
})

onMounted(async () => {
  if (!isEdit.value) return
  const repoId = Number(route.params.id)
  if (!Number.isFinite(repoId)) {
    await router.replace('/admin/repos')
    return
  }

  loading.value = true
  try {
    const repo = await adminApi.fetchRepo(repoId)
    form.name = repo.name
    form.repo_url = repo.repo_url
    form.branch = repo.branch
    form.local_path = repo.local_path
    form.review_cron = repo.review_cron ?? ''
    form.enabled = repo.enabled
  } catch (err) {
    error.value = err instanceof Error ? err.message : '加载项目失败'
  } finally {
    loading.value = false
  }
})

async function submit() {
  saving.value = true
  error.value = ''
  try {
    const payload = {
      ...form,
      review_cron: form.review_cron.trim() || null,
    }
    if (isEdit.value) {
      await adminApi.updateRepo(Number(route.params.id), payload)
    } else {
      await adminApi.createRepo(payload)
    }
    await router.push('/admin/repos')
  } catch (err) {
    error.value = err instanceof Error ? err.message : '保存项目失败'
  } finally {
    saving.value = false
  }
}
</script>
