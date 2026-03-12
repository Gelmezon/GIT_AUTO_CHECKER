<template>
  <main class="messages-shell">
    <AppHeader v-if="auth.user" :user="auth.user" @logout="logout" />

    <section class="messages-layout">
      <aside class="messages-sidebar">
        <div class="sidebar-header">
          <div class="tab-row">
            <button
              class="tab-button"
              :class="{ active: !store.unreadOnly }"
              type="button"
              @click="switchUnread(false)"
            >
              全部
            </button>
            <button
              class="tab-button"
              :class="{ active: store.unreadOnly }"
              type="button"
              @click="switchUnread(true)"
            >
              未读
              <UnreadBadge :count="store.unreadCount" />
            </button>
          </div>
          <button class="ghost-button" type="button" @click="refresh">
            刷新
          </button>
        </div>

        <div v-if="store.loadingList" class="loading-panel">正在加载消息...</div>
        <template v-else-if="store.items.length > 0">
          <MessageCard
            v-for="item in store.items"
            :key="item.id"
            :item="item"
            :selected="item.id === selectedId"
            @select="openMessage"
          />
        </template>
        <EmptyState
          v-else
          eyebrow="暂无消息"
          title="当前筛选下没有结果"
          description="等下一次调度执行完成，审核报告会自动同步到这里。"
        />

        <footer class="pagination-row">
          <button class="ghost-button" type="button" :disabled="store.page <= 1" @click="prevPage">
            上一页
          </button>
          <span>第 {{ store.page }} / {{ store.totalPages }} 页</span>
          <button
            class="ghost-button"
            type="button"
            :disabled="store.page >= store.totalPages"
            @click="nextPage"
          >
            下一页
          </button>
        </footer>
      </aside>

      <section class="messages-detail">
        <div v-if="store.loadingDetail" class="loading-panel">正在加载详情...</div>
        <MessageDetail
          v-else
          :message="store.detail"
          @mark-all-read="markAllRead"
        />
      </section>
    </section>
  </main>
</template>

<script setup lang="ts">
import { computed, onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import AppHeader from '../components/AppHeader.vue'
import EmptyState from '../components/EmptyState.vue'
import MessageCard from '../components/MessageCard.vue'
import MessageDetail from '../components/MessageDetail.vue'
import UnreadBadge from '../components/UnreadBadge.vue'
import { useAuthStore } from '../stores/auth'
import { useMessagesStore } from '../stores/messages'

const auth = useAuthStore()
const store = useMessagesStore()
const route = useRoute()
const router = useRouter()

const selectedId = computed(() => {
  const raw = route.params.id
  return raw ? Number(raw) : null
})

onMounted(async () => {
  await auth.hydrate()
  store.unreadOnly = route.query.unread === 'true'
  store.page = Number(route.query.page ?? 1)
  await store.loadList()
  if (selectedId.value) {
    await store.loadDetail(selectedId.value)
  }
})

watch(
  () => route.params.id,
  async (value) => {
    if (!value) {
      store.detail = null
      return
    }
    await store.loadDetail(Number(value))
  },
)

async function openMessage(id: number) {
  await router.push({
    path: `/messages/${id}`,
    query: {
      unread: store.unreadOnly ? 'true' : undefined,
      page: String(store.page),
    },
  })
}

async function refresh() {
  await store.loadList()
  if (selectedId.value) {
    await store.loadDetail(selectedId.value)
  }
}

async function switchUnread(value: boolean) {
  store.unreadOnly = value
  store.page = 1
  await router.push({
    path: selectedId.value ? `/messages/${selectedId.value}` : '/messages',
    query: {
      unread: value ? 'true' : undefined,
      page: '1',
    },
  })
  await store.loadList()
}

async function prevPage() {
  if (store.page <= 1) return
  store.page -= 1
  await syncPage()
}

async function nextPage() {
  if (store.page >= store.totalPages) return
  store.page += 1
  await syncPage()
}

async function syncPage() {
  await router.push({
    path: selectedId.value ? `/messages/${selectedId.value}` : '/messages',
    query: {
      unread: store.unreadOnly ? 'true' : undefined,
      page: String(store.page),
    },
  })
  await store.loadList()
}

async function markAllRead() {
  await store.markAllRead()
}

async function logout() {
  auth.logout()
  await router.push('/login')
}
</script>
