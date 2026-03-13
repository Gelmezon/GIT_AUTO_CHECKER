import { createRouter, createWebHistory } from 'vue-router'

import AdminLayout from '../components/admin/AdminLayout.vue'
import ActivateView from '../views/ActivateView.vue'
import LoginView from '../views/LoginView.vue'
import MessagesView from '../views/MessagesView.vue'
import AdminDashboardView from '../views/admin/Dashboard.vue'
import AdminReposView from '../views/admin/Repos.vue'
import AdminTasksView from '../views/admin/Tasks.vue'
import AdminUsersView from '../views/admin/Users.vue'
import { useAuthStore } from '../stores/auth'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      redirect: '/messages',
    },
    {
      path: '/login',
      name: 'login',
      component: LoginView,
      meta: { guestOnly: true },
    },
    {
      path: '/activate',
      name: 'activate',
      component: ActivateView,
      meta: { guestOnly: true },
    },
    {
      path: '/messages/:id?',
      name: 'messages',
      component: MessagesView,
      meta: { requiresAuth: true },
    },
    {
      path: '/admin',
      component: AdminLayout,
      meta: { requiresAuth: true, role: 'superAdmin' },
      children: [
        {
          path: '',
          redirect: '/admin/dashboard',
        },
        {
          path: 'dashboard',
          name: 'admin-dashboard',
          component: AdminDashboardView,
        },
        {
          path: 'repos',
          name: 'admin-repos',
          component: AdminReposView,
        },
        {
          path: 'users',
          name: 'admin-users',
          component: AdminUsersView,
        },
        {
          path: 'tasks',
          name: 'admin-tasks',
          component: AdminTasksView,
        },
      ],
    },
  ],
})

router.beforeEach(async (to) => {
  const auth = useAuthStore()
  if (auth.token && !auth.user && !auth.loading) {
    try {
      await auth.hydrate()
    } catch {
      auth.logout()
    }
  }

  if (to.meta.requiresAuth && !auth.isAuthenticated) {
    return {
      path: '/login',
      query: { redirect: to.fullPath },
    }
  }
  if (to.meta.guestOnly && auth.isAuthenticated) {
    return auth.homePath
  }
  if (to.meta.role && auth.user?.role !== to.meta.role) {
    return auth.homePath
  }
  if (auth.user?.role === 'superAdmin' && to.path.startsWith('/messages')) {
    return '/admin/dashboard'
  }
  return true
})

export default router
