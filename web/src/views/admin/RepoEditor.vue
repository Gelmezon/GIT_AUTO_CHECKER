<template>
  <section class="admin-page">
    <section class="admin-card">
      <div class="admin-page-header compact">
        <div>
          <p class="eyebrow">repos</p>
          <h3>{{ isEdit ? '编辑项目' : '新增项目' }}</h3>
          <p class="admin-section-copy">
            为私有仓库选择认证凭据后，后续 `git clone` / `git pull` 会自动注入认证。
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
          <span>认证凭据</span>
          <select v-model="form.credential_id">
            <option value="">公开仓库 / 不使用凭据</option>
            <option v-for="credential in credentials" :key="credential.id" :value="String(credential.id)">
              {{ credential.name }} ({{ credential.auth_type }} / {{ credential.platform }})
            </option>
          </select>
        </label>
        <label>
          <span>审查 Cron</span>
          <input v-model.trim="form.review_cron" placeholder="例如 0 9 * * 1-5" />
        </label>
        <label class="checkbox-field">
          <span>启用状态</span>
          <div>
            <input v-model="form.enabled" type="checkbox" />
            <span>启用项目</span>
          </div>
        </label>
        <div class="admin-inline-actions">
          <button class="primary-button" :disabled="saving" type="submit">
            {{ saving ? '提交中...' : isEdit ? '保存修改' : '创建项目' }}
          </button>
          <RouterLink class="ghost-button" to="/admin/credentials/new">先去新增凭据</RouterLink>
        </div>
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
import type { AdminCredential } from '../../types'

interface RepoFormState {
  name: string
  repo_url: string
  branch: string
  local_path: string
  credential_id: string
  review_cron: string
  enabled: boolean
}

const route = useRoute()
const router = useRouter()
const isEdit = computed(() => Boolean(route.params.id))
const loading = ref(false)
const saving = ref(false)
const error = ref('')
const credentials = ref<AdminCredential[]>([])
const form = reactive<RepoFormState>({
  name: '',
  repo_url: '',
  branch: 'main',
  local_path: '',
  credential_id: '',
  review_cron: '',
  enabled: true,
})

onMounted(async () => {
  loading.value = true
  try {
    credentials.value = await adminApi.fetchCredentials()

    if (!isEdit.value) {
      return
    }

    const repoId = Number(route.params.id)
    if (!Number.isFinite(repoId)) {
      await router.replace('/admin/repos')
      return
    }

    const repo = await adminApi.fetchRepo(repoId)
    form.name = repo.name
    form.repo_url = repo.repo_url
    form.branch = repo.branch
    form.local_path = repo.local_path
    form.credential_id = repo.credential_id ? String(repo.credential_id) : ''
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
      name: form.name,
      repo_url: form.repo_url,
      branch: form.branch,
      local_path: form.local_path,
      credential_id: form.credential_id ? Number(form.credential_id) : null,
      review_cron: form.review_cron.trim() || null,
      enabled: form.enabled,
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
