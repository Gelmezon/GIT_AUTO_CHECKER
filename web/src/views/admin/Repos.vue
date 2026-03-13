<template>
  <section class="admin-page">
    <section class="admin-card">
      <div class="admin-page-header compact">
        <div>
          <p class="eyebrow">repos</p>
          <h3>项目列表</h3>
          <p class="admin-section-copy">默认只显示列表，新增和编辑会进入独立页面。</p>
        </div>
        <div class="admin-toolbar-actions">
          <RouterLink class="primary-button" to="/admin/repos/new">新增项目</RouterLink>
          <button class="ghost-button" type="button" :disabled="loading" @click="load">
            刷新
          </button>
        </div>
      </div>

      <p v-if="error" class="form-error">{{ error }}</p>
      <p v-if="notice" class="admin-notice">{{ notice }}</p>

      <div v-if="loading && items.length === 0" class="loading-panel">正在加载项目...</div>
      <div v-else-if="items.length === 0">
        <EmptyState
          eyebrow="暂无项目"
          title="还没有 Git 项目"
          description="点击右上角“新增项目”后再填写仓库地址、分支和本地路径。"
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
                  <RouterLink class="ghost-button" :to="`/admin/repos/${repo.id}/edit`">
                    编辑
                  </RouterLink>
                  <button class="ghost-button" type="button" @click="triggerSync(repo.id)">
                    同步
                  </button>
                  <button class="ghost-button danger-button" type="button" @click="remove(repo.id)">
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
import { onMounted, ref } from 'vue'

import EmptyState from '../../components/EmptyState.vue'
import * as adminApi from '../../api/admin'
import type { AdminRepo } from '../../types'

const items = ref<AdminRepo[]>([])
const loading = ref(false)
const error = ref('')
const notice = ref('')

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
  notice.value = ''
  error.value = ''
  try {
    await adminApi.deleteRepo(id)
    notice.value = '项目已删除'
    await load()
  } catch (err) {
    error.value = err instanceof Error ? err.message : '删除项目失败'
  }
}

function formatDate(value: string) {
  return new Date(value).toLocaleString()
}
</script>
