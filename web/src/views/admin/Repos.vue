<template>
  <section class="admin-page">
    <section class="admin-card">
      <div class="admin-page-header compact">
        <div>
          <p class="eyebrow">repos</p>
          <h3>{{ editingId ? '编辑项目' : '新增项目' }}</h3>
        </div>
        <button v-if="editingId" class="ghost-button" type="button" @click="resetForm">
          取消编辑
        </button>
      </div>

      <form class="admin-form-grid" @submit.prevent="submit">
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
          <span>审查 Cron</span>
          <input v-model.trim="form.review_cron" placeholder="0 9 * * 1-5" />
        </label>
        <label class="checkbox-field">
          <input v-model="form.enabled" type="checkbox" />
          <span>启用项目</span>
        </label>
        <button class="primary-button" :disabled="saving" type="submit">
          {{ saving ? '提交中...' : editingId ? '保存修改' : '创建项目' }}
        </button>
      </form>

      <p v-if="error" class="form-error">{{ error }}</p>
      <p v-if="notice" class="admin-notice">{{ notice }}</p>
    </section>

    <section class="admin-card">
      <div class="admin-page-header compact">
        <div>
          <p class="eyebrow">list</p>
          <h3>项目列表</h3>
        </div>
        <button class="ghost-button" type="button" :disabled="loading" @click="load">
          刷新
        </button>
      </div>

      <div v-if="loading && items.length === 0" class="loading-panel">正在加载项目...</div>
      <div v-else-if="items.length === 0">
        <EmptyState
          eyebrow="暂无项目"
          title="还没有 Git 项目"
          description="先创建一个仓库配置，后续任务和手动同步都基于它运行。"
        />
      </div>
      <div v-else class="admin-table-wrap">
        <table class="admin-table">
          <thead>
            <tr>
              <th>名称</th>
              <th>仓库地址</th>
              <th>分支</th>
              <th>Cron</th>
              <th>状态</th>
              <th>最近更新</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="repo in items" :key="repo.id">
              <td>{{ repo.name }}</td>
              <td class="truncate-cell">{{ repo.repo_url }}</td>
              <td>{{ repo.branch }}</td>
              <td>{{ repo.review_cron || '-' }}</td>
              <td>
                <span class="status-chip" :class="repo.enabled ? 'done' : 'failed'">
                  {{ repo.enabled ? 'enabled' : 'disabled' }}
                </span>
              </td>
              <td>{{ formatDate(repo.updated_at) }}</td>
              <td>
                <div class="actions-cell">
                  <button class="ghost-button" type="button" @click="startEdit(repo)">编辑</button>
                  <button class="ghost-button" type="button" @click="triggerSync(repo.id)">
                    同步
                  </button>
                  <button
                    class="ghost-button danger-button"
                    type="button"
                    @click="remove(repo.id)"
                  >
                    删除
                  </button>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>
  </section>
</template>

<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'

import EmptyState from '../../components/EmptyState.vue'
import * as adminApi from '../../api/admin'
import type { AdminRepo } from '../../types'

interface RepoFormState {
  name: string
  repo_url: string
  branch: string
  local_path: string
  review_cron: string
  enabled: boolean
}

const items = ref<AdminRepo[]>([])
const loading = ref(false)
const saving = ref(false)
const error = ref('')
const notice = ref('')
const editingId = ref<number | null>(null)
const form = reactive<RepoFormState>(createEmptyForm())

onMounted(load)

async function load() {
  loading.value = true
  error.value = ''
  try {
    items.value = await adminApi.fetchRepos()
  } catch (err) {
    error.value = err instanceof Error ? err.message : '加载项目失败'
  } finally {
    loading.value = false
  }
}

async function submit() {
  saving.value = true
  error.value = ''
  notice.value = ''
  try {
    const payload = {
      ...form,
      review_cron: form.review_cron.trim() || null,
    }
    if (editingId.value) {
      await adminApi.updateRepo(editingId.value, payload)
      notice.value = '项目已更新'
    } else {
      await adminApi.createRepo(payload)
      notice.value = '项目已创建'
    }
    resetForm()
    await load()
  } catch (err) {
    error.value = err instanceof Error ? err.message : '保存项目失败'
  } finally {
    saving.value = false
  }
}

function startEdit(repo: AdminRepo) {
  editingId.value = repo.id
  form.name = repo.name
  form.repo_url = repo.repo_url
  form.branch = repo.branch
  form.local_path = repo.local_path
  form.review_cron = repo.review_cron ?? ''
  form.enabled = repo.enabled
}

function resetForm() {
  editingId.value = null
  Object.assign(form, createEmptyForm())
}

async function triggerSync(id: number) {
  notice.value = ''
  error.value = ''
  try {
    const result = await adminApi.syncRepo(id)
    notice.value = `仓库已${result.action === 'cloned' ? '克隆' : '同步'}，当前分支 ${result.branch}`
    await load()
  } catch (err) {
    error.value = err instanceof Error ? err.message : '同步项目失败'
  }
}

async function remove(id: number) {
  if (!window.confirm('确认删除这个项目吗？相关任务也会一起删除。')) {
    return
  }
  error.value = ''
  notice.value = ''
  try {
    await adminApi.deleteRepo(id)
    if (editingId.value === id) {
      resetForm()
    }
    notice.value = '项目已删除'
    await load()
  } catch (err) {
    error.value = err instanceof Error ? err.message : '删除项目失败'
  }
}

function createEmptyForm(): RepoFormState {
  return {
    name: '',
    repo_url: '',
    branch: 'main',
    local_path: '',
    review_cron: '',
    enabled: true,
  }
}

function formatDate(value: string) {
  return new Date(value).toLocaleString()
}
</script>
