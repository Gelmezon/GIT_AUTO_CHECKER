<template>
  <Teleport to="body">
    <div class="toast-stack" aria-live="polite" aria-atomic="true">
      <TransitionGroup name="toast">
        <article
          v-for="toast in ui.toasts"
          :key="toast.id"
          class="toast-card"
          :class="toast.type"
        >
          <div>
            <strong>{{ toast.title }}</strong>
            <p v-if="toast.message">{{ toast.message }}</p>
          </div>
          <button class="ghost-button toast-close" type="button" @click="ui.dismissToast(toast.id)">
            关闭
          </button>
        </article>
      </TransitionGroup>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { useUiStore } from '../stores/ui'

const ui = useUiStore()
</script>
