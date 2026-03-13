import { createRouter, createWebHistory } from 'vue-router'

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
      component: () => import('../views/LoginView.vue'),
      meta: { guestOnly: true, title: '登录' },
    },
    {
      path: '/activate',
      name: 'activate',
      component: () => import('../views/ActivateView.vue'),
      meta: { guestOnly: true, title: '激活账号' },
    },
    {
      path: '/messages/:id?',
      name: 'messages',
      component: () => import('../views/MessagesView.vue'),
      meta: { requiresAuth: true, title: '消息中心' },
    },
    {
      path: '/admin',
      component: () => import('../components/admin/AdminLayout.vue'),
      meta: { requiresAuth: true, role: 'superAdmin', title: '运行总览' },
      children: [
        {
          path: '',
          redirect: '/admin/dashboard',
        },
        {
          path: 'dashboard',
          name: 'admin-dashboard',
          component: () => import('../views/admin/Dashboard.vue'),
          meta: { title: '运行总览' },
        },
        {
          path: 'repos',
          name: 'admin-repos',
          component: () => import('../views/admin/Repos.vue'),
          meta: { title: '项目管理' },
        },
        {
          path: 'repos/new',
          name: 'admin-repos-new',
          component: () => import('../views/admin/RepoEditor.vue'),
          meta: { title: '新增项目' },
        },
        {
          path: 'repos/:id/edit',
          name: 'admin-repos-edit',
          component: () => import('../views/admin/RepoEditor.vue'),
          meta: { title: '编辑项目' },
        },
        {
          path: 'credentials',
          name: 'admin-credentials',
          component: () => import('../views/admin/Credentials.vue'),
          meta: { title: '凭据管理' },
        },
        {
          path: 'credentials/new',
          name: 'admin-credentials-new',
          component: () => import('../views/admin/CredentialEditor.vue'),
          meta: { title: '新增凭据' },
        },
        {
          path: 'credentials/:id/edit',
          name: 'admin-credentials-edit',
          component: () => import('../views/admin/CredentialEditor.vue'),
          meta: { title: '编辑凭据' },
        },
        {
          path: 'users',
          name: 'admin-users',
          component: () => import('../views/admin/Users.vue'),
          meta: { title: '用户管理' },
        },
        {
          path: 'users/new',
          name: 'admin-users-new',
          component: () => import('../views/admin/UserEditor.vue'),
          meta: { title: '新增用户' },
        },
        {
          path: 'users/:id/edit',
          name: 'admin-users-edit',
          component: () => import('../views/admin/UserEditor.vue'),
          meta: { title: '编辑用户' },
        },
        {
          path: 'tasks',
          name: 'admin-tasks',
          component: () => import('../views/admin/Tasks.vue'),
          meta: { title: '任务管理' },
        },
        {
          path: 'tasks/new',
          name: 'admin-tasks-new',
          component: () => import('../views/admin/TaskEditor.vue'),
          meta: { title: '新增任务' },
        },
        {
          path: 'tasks/:id',
          name: 'admin-task-detail',
          component: () => import('../views/admin/TaskDetail.vue'),
          meta: { title: '任务详情' },
        },
        {
          path: 'tasks/:id/edit',
          name: 'admin-task-edit',
          component: () => import('../views/admin/TaskEditor.vue'),
          meta: { title: '编辑任务' },
        },
        {
          path: 'runs',
          name: 'admin-runs',
          component: () => import('../views/admin/Runs.vue'),
          meta: { title: '执行记录' },
        },
        {
          path: 'runs/:runId',
          name: 'admin-run-detail',
          component: () => import('../views/admin/RunDetail.vue'),
          meta: { title: '执行详情' },
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
