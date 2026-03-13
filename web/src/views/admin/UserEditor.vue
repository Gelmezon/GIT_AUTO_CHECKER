<template>
  <section class="admin-page">
    <section class="admin-card">
      <div class="admin-page-header compact">
        <div>
          <p class="eyebrow">users</p>
          <h3>{{ isEdit ? '编辑用户' : '新增用户' }}</h3>
          <p class="admin-section-copy">
            {{ isEdit ? '更新用户邮箱和显示名称。' : '创建后用户可在激活页自行设置密码。' }}
          </p>
        </div>
        <RouterLink class="ghost-button" to="/admin/users">返回列表</RouterLink>
      </div>

      <div v-if="loading" class="loading-panel">正在加载用户...</div>
      <form v-else class="admin-form-grid" @submit.prevent="submit">
        <label>
          <span>邮箱</span>
          <input v-model.trim="form.email" type="email" required />
        </label>
        <label>
          <span>显示名称</span>
          <input v-model.trim="form.display_name" required />
        </label>
        <button class="primary-button" :disabled="saving" type="submit">
          {{ saving ? '提交中...' : isEdit ? '保存修改' : '创建用户' }}
        </button>
      </form>

      <p v-if="error" class="form-error">{{ error }}</p>
    </section>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import * as adminApi from '../../api/admin'

interface UserFormState {
  email: string
  display_name: string
}

const route = useRoute()
const router = useRouter()
const isEdit = computed(() => Boolean(route.params.id))
const loading = ref(false)
const saving = ref(false)
const error = ref('')
const form = reactive<UserFormState>({
  email: '',
  display_name: '',
})

onMounted(async () => {
  if (!isEdit.value) return
  const userId = Number(route.params.id)
  if (!Number.isFinite(userId)) {
    await router.replace('/admin/users')
    return
  }

  loading.value = true
  try {
    const user = await adminApi.fetchUser(userId)
    form.email = user.email
    form.display_name = user.display_name
  } catch (err) {
    error.value = err instanceof Error ? err.message : '加载用户失败'
  } finally {
    loading.value = false
  }
})

async function submit() {
  saving.value = true
  error.value = ''
  try {
    if (isEdit.value) {
      await adminApi.updateUser(Number(route.params.id), { ...form })
    } else {
      await adminApi.createUser({ ...form })
    }
    await router.push('/admin/users')
  } catch (err) {
    error.value = err instanceof Error ? err.message : '保存用户失败'
  } finally {
    saving.value = false
  }
}
</script>
