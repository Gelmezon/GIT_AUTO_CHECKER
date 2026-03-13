<template>
  <section class="admin-page">
    <section class="admin-card">
      <div class="admin-page-header compact">
        <div>
          <p class="eyebrow">users</p>
          <h3>{{ editingId ? '编辑用户' : '新增用户' }}</h3>
        </div>
        <button v-if="editingId" class="ghost-button" type="button" @click="resetForm">
          取消编辑
        </button>
      </div>

      <form class="admin-form-grid" @submit.prevent="submit">
        <label>
          <span>邮箱</span>
          <input v-model.trim="form.email" type="email" required />
        </label>
        <label>
          <span>显示名称</span>
          <input v-model.trim="form.display_name" required />
        </label>
        <button class="primary-button" :disabled="saving" type="submit">
          {{ saving ? '提交中...' : editingId ? '保存修改' : '创建用户' }}
        </button>
      </form>

      <p class="admin-hint">新建用户后，用户可通过“激活账号”页面自行设置密码。</p>
      <p v-if="error" class="form-error">{{ error }}</p>
      <p v-if="notice" class="admin-notice">{{ notice }}</p>
    </section>

    <section class="admin-card">
      <div class="admin-page-header compact">
        <div>
          <p class="eyebrow">list</p>
          <h3>用户列表</h3>
        </div>
        <button class="ghost-button" type="button" :disabled="loading" @click="load">
          刷新
        </button>
      </div>

      <div v-if="loading && items.length === 0" class="loading-panel">正在加载用户...</div>
      <div v-else-if="items.length === 0">
        <EmptyState
          eyebrow="暂无用户"
          title="还没有用户数据"
          description="你可以手动创建用户，也可以等调度任务根据 Git 提交人自动发现。"
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
              <td>{{ formatDate(user.created_at) }}</td>
              <td>
                <div class="actions-cell">
                  <button class="ghost-button" type="button" @click="startEdit(user)">编辑</button>
                  <button
                    class="ghost-button danger-button"
                    type="button"
                    @click="remove(user.id)"
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
import type { AdminUser } from '../../types'

interface UserFormState {
  email: string
  display_name: string
}

const items = ref<AdminUser[]>([])
const loading = ref(false)
const saving = ref(false)
const error = ref('')
const notice = ref('')
const editingId = ref<number | null>(null)
const form = reactive<UserFormState>(createEmptyForm())

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

async function submit() {
  saving.value = true
  error.value = ''
  notice.value = ''
  try {
    if (editingId.value) {
      await adminApi.updateUser(editingId.value, { ...form })
      notice.value = '用户已更新'
    } else {
      await adminApi.createUser({ ...form })
      notice.value = '用户已创建'
    }
    resetForm()
    await load()
  } catch (err) {
    error.value = err instanceof Error ? err.message : '保存用户失败'
  } finally {
    saving.value = false
  }
}

function startEdit(user: AdminUser) {
  editingId.value = user.id
  form.email = user.email
  form.display_name = user.display_name
}

function resetForm() {
  editingId.value = null
  Object.assign(form, createEmptyForm())
}

async function remove(id: number) {
  if (!window.confirm('确认删除这个用户吗？该用户的消息也会被一并删除。')) {
    return
  }
  error.value = ''
  notice.value = ''
  try {
    await adminApi.deleteUser(id)
    if (editingId.value === id) {
      resetForm()
    }
    notice.value = '用户已删除'
    await load()
  } catch (err) {
    error.value = err instanceof Error ? err.message : '删除用户失败'
  }
}

function createEmptyForm(): UserFormState {
  return {
    email: '',
    display_name: '',
  }
}

function formatDate(value: string) {
  return new Date(value).toLocaleString()
}
</script>
