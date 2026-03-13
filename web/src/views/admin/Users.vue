<template>
  <section class="admin-page">
    <section class="admin-card">
      <div class="admin-page-header compact">
        <div>
          <p class="eyebrow">users</p>
          <h3>用户列表</h3>
          <p class="admin-section-copy">默认只展示用户列表，新增和编辑会跳转到独立页面。</p>
        </div>
        <div class="admin-toolbar-actions">
          <RouterLink class="primary-button" to="/admin/users/new">新增用户</RouterLink>
          <button class="ghost-button" type="button" :disabled="loading" @click="load">
            刷新
          </button>
        </div>
      </div>

      <p v-if="error" class="form-error">{{ error }}</p>
      <div v-if="loading && items.length === 0" class="loading-panel">正在加载用户...</div>
      <div v-else-if="items.length === 0">
        <EmptyState
          eyebrow="暂无用户"
          title="还没有用户数据"
          description="点击右上角“新增用户”即可手动创建账号，或等待系统根据 Git 提交人自动发现。"
        />
      </div>
      <div v-else class="admin-table-wrap">
        <table class="admin-table">
          <thead>
            <tr>
              <th>邮箱</th>
              <th>显示名称</th>
              <th>状态</th>
              <th>创建时间</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="user in items" :key="user.id">
              <td>{{ user.email }}</td>
              <td>{{ user.display_name }}</td>
              <td>
                <span class="status-chip" :class="user.activated_at ? 'done' : 'pending'">
                  {{ user.activated_at ? 'active' : 'inactive' }}
                </span>
              </td>
              <td>{{ formatDateTime(user.created_at) }}</td>
              <td>
                <div class="actions-cell">
                  <RouterLink class="ghost-button" :to="`/admin/users/${user.id}/edit`">
                    编辑
                  </RouterLink>
                  <button class="ghost-button danger-button" type="button" @click="remove(user.id)">
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
import { useUiStore } from '../../stores/ui'
import type { AdminUser } from '../../types'
import { formatDateTime } from '../../utils/date'

const ui = useUiStore()
const items = ref<AdminUser[]>([])
const loading = ref(false)
const error = ref('')

onMounted(load)

async function load() {
  loading.value = true
  error.value = ''
  try {
    items.value = await adminApi.fetchUsers()
  } catch (err) {
    error.value = err instanceof Error ? err.message : '加载用户失败'
  } finally {
    loading.value = false
  }
}

async function remove(id: number) {
  const confirmed = await ui.confirm({
    title: '删除用户？',
    description: '删除后，该用户的所有消息也会被一并清理。',
    confirmText: '确认删除',
    cancelText: '先保留',
    tone: 'danger',
  })
  if (!confirmed) {
    return
  }
  error.value = ''
  try {
    await adminApi.deleteUser(id)
    ui.toast('success', '用户已删除')
    await load()
  } catch (err) {
    error.value = err instanceof Error ? err.message : '删除用户失败'
  }
}
</script>
