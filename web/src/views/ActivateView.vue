<template>
  <main class="auth-shell">
    <section class="auth-card auth-card-offset">
      <p class="eyebrow">账户激活</p>
      <h1>首次登录前，先设置密码</h1>
      <p class="auth-copy">
        管理员或系统已经创建了账号后，可以在这里完成激活并直接进入消息中心。
      </p>

      <form class="auth-form" @submit.prevent="submit">
        <label>
          <span>邮箱地址</span>
          <input v-model.trim="email" type="email" autocomplete="email" required />
        </label>
        <label>
          <span>新密码</span>
          <input v-model="password" type="password" autocomplete="new-password" required />
        </label>
        <button class="primary-button" :disabled="submitting" type="submit">
          {{ submitting ? '激活中...' : '激活账号' }}
        </button>
      </form>

      <p v-if="error" class="form-error">{{ error }}</p>
      <RouterLink class="text-link" to="/login">返回登录</RouterLink>
    </section>
  </main>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'

import * as authApi from '../api/auth'
import { useAuthStore } from '../stores/auth'

const router = useRouter()
const auth = useAuthStore()

const email = ref('')
const password = ref('')
const submitting = ref(false)
const error = ref('')

async function submit() {
  submitting.value = true
  error.value = ''
  try {
    const response = await authApi.activate(email.value, password.value)
    auth.setAuth(response.token, response.user)
    await router.push('/messages')
  } catch (err) {
    error.value = err instanceof Error ? err.message : '激活失败'
  } finally {
    submitting.value = false
  }
}
</script>
