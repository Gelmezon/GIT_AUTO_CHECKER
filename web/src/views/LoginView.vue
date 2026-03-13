<template>
  <main class="auth-shell">
    <section class="auth-card">
      <p class="eyebrow">git-helper</p>
      <h1>把审核结果拉回桌面</h1>
      <p class="auth-copy">
        使用消息中心查看代码审查与测试生成结果。登录后会自动进入消息总览。
      </p>

      <form class="auth-form" @submit.prevent="submit">
        <label>
          <span>邮箱地址</span>
          <input v-model.trim="email" type="email" autocomplete="email" required />
        </label>
        <label>
          <span>密码</span>
          <input v-model="password" type="password" autocomplete="current-password" required />
        </label>
        <button class="primary-button" :disabled="submitting" type="submit">
          {{ submitting ? '登录中...' : '登录' }}
        </button>
      </form>

      <p v-if="error" class="form-error">{{ error }}</p>
      <RouterLink class="text-link" to="/activate">首次使用，去激活账号</RouterLink>
    </section>
  </main>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import * as authApi from '../api/auth'
import { useAuthStore } from '../stores/auth'

const router = useRouter()
const route = useRoute()
const auth = useAuthStore()

const email = ref('')
const password = ref('')
const submitting = ref(false)
const error = ref('')

async function submit() {
  submitting.value = true
  error.value = ''
  try {
    const response = await authApi.login(email.value, password.value)
    auth.setAuth(response.token, response.user)
    const fallback = response.user.role === 'superAdmin' ? '/admin/dashboard' : '/messages'
    const redirect = typeof route.query.redirect === 'string' ? route.query.redirect : fallback
    const target =
      response.user.role === 'superAdmin'
        ? redirect.startsWith('/admin')
          ? redirect
          : fallback
        : redirect.startsWith('/admin')
          ? fallback
          : redirect
    await router.push(target)
  } catch (err) {
    error.value = err instanceof Error ? err.message : '登录失败'
  } finally {
    submitting.value = false
  }
}
</script>
