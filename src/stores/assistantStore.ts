import { defineStore } from 'pinia'
import { ref } from 'vue'

export const useAssistantStore = defineStore('assistant', () => {
  const sessions = ref<unknown[]>([])
  const activeSessionId = ref<string | null>(null)
  const messages = ref<unknown[]>([])
  const capabilities = ref<unknown[]>([])
  const loading = ref(false)

  return { sessions, activeSessionId, messages, capabilities, loading }
})
