<template>
  <div class="empty-state">
    <p class="eyebrow">{{ eyebrow }}</p>
    <h2>{{ title }}</h2>
    <p>{{ description }}</p>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  eyebrow: string
  title: string
  description: string
}>()
</script>
