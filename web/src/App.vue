<template>
  <RouterView />
  <ConfirmDialog />
  <ToastViewport />
</template>

<script setup lang="ts">
import ConfirmDialog from './components/ConfirmDialog.vue'
import ToastViewport from './components/ToastViewport.vue'
</script>
