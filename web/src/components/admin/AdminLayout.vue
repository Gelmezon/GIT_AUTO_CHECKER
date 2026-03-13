<template>
  <main class="admin-shell">
    <Transition name="overlay-fade">
      <button
        v-if="sidebarOpen && isMobile"
        class="admin-overlay"
        type="button"
        aria-label="关闭导航"
        @click="sidebarOpen = false"
      />
    </Transition>

    <aside
      class="admin-sidebar"
      :class="{ open: sidebarOpen }"
      aria-label="管理导航"
    >
      <div class="admin-sidebar-top">
        <div>
          <p class="eyebrow">git-helper</p>
          <h1>调度中枢</h1>
          <p class="admin-copy">统一管理仓库、凭据、用户、任务定义与运行记录。</p>
        </div>
        <button
          v-if="isMobile"
          class="ghost-button admin-close-nav"
          type="button"
          @click="sidebarOpen = false"
        >
          收起
        </button>
      </div>

      <nav class="admin-nav">
        <RouterLink
          v-for="item in navItems"
          :key="item.to"
          class="admin-nav-link"
          :class="{ active: route.path.startsWith(item.to) }"
          :to="item.to"
          @click="sidebarOpen = false"
        >
          <span>{{ item.label }}</span>
          <small>{{ item.copy }}</small>
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
        <div class="admin-toolbar-main">
          <button
            v-if="isMobile"
            class="ghost-button admin-menu-button"
            type="button"
            aria-label="打开导航"
            @click="sidebarOpen = true"
          >
            菜单
          </button>
          <div>
            <p class="eyebrow">superAdmin</p>
            <h2>{{ title }}</h2>
            <p class="admin-toolbar-copy">{{ subtitle }}</p>
          </div>
        </div>
      </header>

      <RouterView v-slot="{ Component }">
        <Transition name="fade" mode="out-in">
          <component :is="Component" />
        </Transition>
      </RouterView>
    </section>
  </main>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { useAuthStore } from '../../stores/auth'

const auth = useAuthStore()
const route = useRoute()
const router = useRouter()
const sidebarOpen = ref(false)
const isMobile = ref(false)

const navItems = [
  { to: '/admin/dashboard', label: '仪表盘', copy: '实时概览' },
  { to: '/admin/tasks', label: '任务管理', copy: '定义与调度' },
  { to: '/admin/runs', label: '执行记录', copy: '运行历史' },
  { to: '/admin/repos', label: '项目管理', copy: '仓库与分支' },
  { to: '/admin/credentials', label: '凭据管理', copy: '认证配置' },
  { to: '/admin/users', label: '用户管理', copy: '账号维护' },
]

const title = computed(() => String(route.meta.title ?? '运行总览'))

const subtitle = computed(() => {
  if (route.path.startsWith('/admin/runs')) return '按任务与状态追踪执行历史，快速定位失败与阻塞。'
  if (route.path.startsWith('/admin/tasks')) return '查看任务定义、暂停恢复调度，并随时手动触发执行。'
  if (route.path.startsWith('/admin/repos')) return '维护仓库绑定、本地路径与同步认证。'
  if (route.path.startsWith('/admin/credentials')) return '为 HTTPS、SSH 与 Basic Auth 准备安全凭据。'
  if (route.path.startsWith('/admin/users')) return '管理接收审核报告的账号与激活状态。'
  return '把仓库同步、任务调度和结果分发放在同一条观察链路里。'
})

function syncViewport() {
  isMobile.value = window.innerWidth <= 980
  if (!isMobile.value) {
    sidebarOpen.value = false
  }
}

async function logout() {
  auth.logout()
  await router.push('/login')
}

onMounted(() => {
  syncViewport()
  window.addEventListener('resize', syncViewport)
})

onBeforeUnmount(() => window.removeEventListener('resize', syncViewport))
</script>
