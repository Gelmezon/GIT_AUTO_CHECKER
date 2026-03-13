<template>
  <main class="admin-shell">
    <aside class="admin-sidebar">
      <div>
        <p class="eyebrow">git-helper</p>
        <h1>管理后台</h1>
        <p class="admin-copy">superAdmin 可以直接通过页面管理仓库、用户和任务。</p>
      </div>

      <nav class="admin-nav">
        <RouterLink
          v-for="item in navItems"
          :key="item.to"
          class="admin-nav-link"
          :class="{ active: route.path === item.to }"
          :to="item.to"
        >
          {{ item.label }}
        </RouterLink>
      </nav>

      <div v-if="auth.user" class="admin-user-card">
        <strong>{{ auth.user.display_name }}</strong>
        <small>{{ auth.user.email }}</small>
      </div>

      <button class="ghost-button admin-logout" type="button" @click="logout">
        退出登录
      </button>
    </aside>

    <section class="admin-main">
      <header class="admin-toolbar">
        <div>
          <p class="eyebrow">superAdmin</p>
          <h2>{{ title }}</h2>
        </div>
        <p class="admin-toolbar-copy">把原本需要 CLI 的运维动作集中到浏览器里完成。</p>
      </header>

      <RouterView />
    </section>
  </main>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { useAuthStore } from '../../stores/auth'

const auth = useAuthStore()
const route = useRoute()
const router = useRouter()

const navItems = [
  { to: '/admin/dashboard', label: '仪表盘' },
  { to: '/admin/repos', label: '项目管理' },
  { to: '/admin/users', label: '用户管理' },
  { to: '/admin/tasks', label: '任务管理' },
]

const title = computed(() => {
  switch (route.path) {
    case '/admin/repos':
      return '项目管理'
    case '/admin/users':
      return '用户管理'
    case '/admin/tasks':
      return '任务管理'
    default:
      return '运行总览'
  }
})

async function logout() {
  auth.logout()
  await router.push('/login')
}
</script>
