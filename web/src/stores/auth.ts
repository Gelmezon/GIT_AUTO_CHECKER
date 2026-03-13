import { computed, ref } from 'vue'
import { defineStore } from 'pinia'

import type { User } from '../types'
import * as authApi from '../api/auth'

export const useAuthStore = defineStore('auth', () => {
  const token = ref(localStorage.getItem('token') ?? '')
  const user = ref<User | null>(null)
  const loading = ref(false)

  const isAuthenticated = computed(() => Boolean(token.value))
  const isSuperAdmin = computed(() => user.value?.role === 'superAdmin')
  const homePath = computed(() => (isSuperAdmin.value ? '/admin/dashboard' : '/messages'))

  function setAuth(nextToken: string, nextUser: User) {
    token.value = nextToken
    user.value = nextUser
    localStorage.setItem('token', nextToken)
  }

  async function hydrate() {
    if (!token.value || user.value || loading.value) {
      return
    }
    loading.value = true
    try {
      user.value = await authApi.fetchMe()
    } finally {
      loading.value = false
    }
  }

  function logout() {
    token.value = ''
    user.value = null
    localStorage.removeItem('token')
  }

  return {
    token,
    user,
    loading,
    isAuthenticated,
    isSuperAdmin,
    homePath,
    setAuth,
    hydrate,
    logout,
  }
})
