<template>
  <Teleport to="body">
    <Transition name="overlay-fade">
      <div
        v-if="ui.confirmation"
        class="dialog-backdrop"
        role="presentation"
        @click.self="ui.resolveConfirmation(false)"
      >
        <div
          class="confirm-dialog"
          role="alertdialog"
          aria-modal="true"
          :aria-labelledby="titleId"
          :aria-describedby="descriptionId"
        >
          <p class="eyebrow">confirm</p>
          <h2 :id="titleId">{{ ui.confirmation.title }}</h2>
          <p :id="descriptionId" class="confirm-copy">{{ ui.confirmation.description }}</p>

          <div class="confirm-actions">
            <button class="ghost-button" type="button" @click="ui.resolveConfirmation(false)">
              {{ ui.confirmation.cancelText ?? '取消' }}
            </button>
            <button
              class="primary-button"
              :class="{ 'danger-fill': ui.confirmation.tone === 'danger' }"
              type="button"
              @click="ui.resolveConfirmation(true)"
            >
              {{ ui.confirmation.confirmText ?? '确认' }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted } from 'vue'

import { useUiStore } from '../stores/ui'

const ui = useUiStore()
const titleId = computed(() => `confirm-title-${ui.confirmation ? 'active' : 'idle'}`)
const descriptionId = computed(() => `confirm-description-${ui.confirmation ? 'active' : 'idle'}`)

function onKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape' && ui.confirmation) {
    ui.resolveConfirmation(false)
  }
}

onMounted(() => window.addEventListener('keydown', onKeydown))
onBeforeUnmount(() => window.removeEventListener('keydown', onKeydown))
</script>
