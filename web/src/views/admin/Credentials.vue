<template>
  <section class="admin-page">
    <section class="admin-card">
      <div class="admin-page-header compact">
        <div>
          <p class="eyebrow">credentials</p>
          <h3>凭据列表</h3>
          <p class="admin-section-copy">为私有仓库准备 HTTPS Token、SSH Key 或 Basic Auth 凭据。</p>
        </div>
        <div class="admin-toolbar-actions">
          <RouterLink class="primary-button" to="/admin/credentials/new">新增凭据</RouterLink>
          <button class="ghost-button" type="button" :disabled="loading" @click="load">
            刷新
          </button>
        </div>
      </div>

      <p v-if="error" class="form-error">{{ error }}</p>
      <div v-if="loading && items.length === 0" class="loading-panel">正在加载凭据...</div>
      <div v-else-if="items.length === 0">
        <EmptyState
          eyebrow="暂无凭据"
          title="还没有 Git 认证凭据"
          description="点击右上角“新增凭据”后，可以为私有仓库配置 token、ssh 或 basic 认证。"
        />
      </div>
      <div v-else class="admin-table-wrap">
        <table class="admin-table">
          <thead>
            <tr>
              <th>名称</th>
              <th>平台</th>
              <th>类型</th>
              <th>用户名</th>
              <th>SSH Key</th>
              <th>密文字段</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="credential in items" :key="credential.id">
              <td>{{ credential.name }}</td>
              <td>{{ credential.platform }}</td>
              <td>{{ credential.auth_type }}</td>
              <td>{{ credential.username || '-' }}</td>
              <td class="truncate-cell">{{ credential.ssh_key_path || '-' }}</td>
              <td>
                <span>{{ credential.has_token ? 'token ' : '' }}</span>
                <span>{{ credential.has_password ? 'password' : '' }}</span>
              </td>
              <td>
                <div class="actions-cell">
                  <RouterLink class="ghost-button" :to="`/admin/credentials/${credential.id}/edit`">
                    编辑
                  </RouterLink>
                  <button
                    class="ghost-button danger-button"
                    type="button"
                    @click="remove(credential.id)"
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
import { onMounted, ref } from 'vue'

import EmptyState from '../../components/EmptyState.vue'
import * as adminApi from '../../api/admin'
import { useUiStore } from '../../stores/ui'
import type { AdminCredential } from '../../types'

const ui = useUiStore()
const items = ref<AdminCredential[]>([])
const loading = ref(false)
const error = ref('')

onMounted(load)

async function load() {
  loading.value = true
  error.value = ''
  try {
    items.value = await adminApi.fetchCredentials()
  } catch (err) {
    error.value = err instanceof Error ? err.message : '加载凭据失败'
  } finally {
    loading.value = false
  }
}

async function remove(id: number) {
  const confirmed = await ui.confirm({
    title: '删除凭据？',
    description: '已绑定仓库会自动解除关联，但凭据本身会被彻底移除。',
    confirmText: '确认删除',
    cancelText: '先保留',
    tone: 'danger',
  })
  if (!confirmed) {
    return
  }
  error.value = ''
  try {
    await adminApi.deleteCredential(id)
    ui.toast('success', '凭据已删除')
    await load()
  } catch (err) {
    error.value = err instanceof Error ? err.message : '删除凭据失败'
  }
}
</script>
