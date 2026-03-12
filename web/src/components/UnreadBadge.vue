<template>
  <span v-if="count > 0" class="unread-badge">
    {{ count > 99 ? '99+' : count }}
  </span>
</template>

<script setup lang="ts">
defineProps<{
  count: number
}>()
</script>
