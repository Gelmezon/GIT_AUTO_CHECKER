import { defineStore } from 'pinia'
import { ref } from 'vue'

export type ToastType = 'success' | 'error' | 'warning' | 'info'

export interface ToastItem {
  id: number
  type: ToastType
  title: string
  message?: string
}

interface ConfirmOptions {
  title: string
  description: string
  confirmText?: string
  cancelText?: string
  tone?: 'danger' | 'default'
}

interface ConfirmState extends ConfirmOptions {
  resolve: (value: boolean) => void
}

let toastId = 0

export const useUiStore = defineStore('ui', () => {
  const toasts = ref<ToastItem[]>([])
  const confirmation = ref<ConfirmState | null>(null)

  function toast(type: ToastType, title: string, message?: string, duration = 3200) {
    const id = ++toastId
    toasts.value.push({ id, type, title, message })
    window.setTimeout(() => dismissToast(id), duration)
  }

  function dismissToast(id: number) {
    toasts.value = toasts.value.filter((item) => item.id !== id)
  }

  function confirm(options: ConfirmOptions) {
    return new Promise<boolean>((resolve) => {
      confirmation.value = { ...options, resolve }
    })
  }

  function resolveConfirmation(value: boolean) {
    const pending = confirmation.value
    confirmation.value = null
    pending?.resolve(value)
  }

  return {
    toasts,
    confirmation,
    toast,
    dismissToast,
    confirm,
    resolveConfirmation,
  }
})
