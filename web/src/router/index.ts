import { createRouter, createWebHistory } from 'vue-router'

import ActivateView from '../views/ActivateView.vue'
import LoginView from '../views/LoginView.vue'
import MessagesView from '../views/MessagesView.vue'
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
  ],
})

router.beforeEach((to) => {
  const auth = useAuthStore()
  if (to.meta.requiresAuth && !auth.isAuthenticated) {
    return {
      path: '/login',
      query: { redirect: to.fullPath },
    }
  }
  if (to.meta.guestOnly && auth.isAuthenticated) {
    return '/messages'
  }
  return true
})

export default router
