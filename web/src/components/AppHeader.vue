<template>
  <header class="app-header">
    <div>
      <p class="eyebrow">git-helper</p>
      <h1>消息中心</h1>
    </div>
    <div class="header-actions">
      <div class="user-pill">
        <span>{{ initials }}</span>
        <div>
          <strong>{{ user.display_name }}</strong>
          <small>{{ user.email }}</small>
        </div>
      </div>
      <button class="ghost-button" type="button" @click="$emit('logout')">
        退出
      </button>
    </div>
  </header>
</template>

<script setup lang="ts">
import { computed } from 'vue'

import type { User } from '../types'

const props = defineProps<{
  user: User
}>()

defineEmits<{
  logout: []
}>()

const initials = computed(() =>
  props.user.display_name
    .split(/\s+/)
    .slice(0, 2)
    .map((segment) => segment[0]?.toUpperCase() ?? '')
    .join(''),
)
</script>
