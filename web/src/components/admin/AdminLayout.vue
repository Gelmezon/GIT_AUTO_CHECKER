<template>
  <main class="admin-shell">
    <aside class="admin-sidebar">
      <div>
        <p class="eyebrow">git-helper</p>
        <h1>管理后台</h1>
        <p class="admin-copy">superAdmin 可以在这里集中管理仓库、用户和任务。</p>
      </div>

      <nav class="admin-nav">
        <RouterLink
          v-for="item in navItems"
          :key="item.to"
          class="admin-nav-link"
          :class="{ active: route.path.startsWith(item.to) }"
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
        <p class="admin-toolbar-copy">把原本需要命令行完成的管理动作收拢到浏览器里。</p>
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
  if (route.path.startsWith('/admin/repos/new')) return '新增项目'
  if (/^\/admin\/repos\/\d+\/edit$/.test(route.path)) return '编辑项目'
  if (route.path.startsWith('/admin/repos')) return '项目管理'
  if (route.path.startsWith('/admin/users/new')) return '新增用户'
  if (/^\/admin\/users\/\d+\/edit$/.test(route.path)) return '编辑用户'
  if (route.path.startsWith('/admin/users')) return '用户管理'
  if (route.path.startsWith('/admin/tasks/new')) return '新增任务'
  if (route.path.startsWith('/admin/tasks')) return '任务管理'
  return '运行总览'
})

async function logout() {
  auth.logout()
  await router.push('/login')
}
</script>
