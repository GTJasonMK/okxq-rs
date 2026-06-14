import { onMounted, ref } from 'vue'
import * as api from '@/api/assistant'
import { useAssistantStore } from '@/stores/assistantStore'
import type { ChatMessage, ChatSession } from '@/types'
import { describeError } from '@/utils/logger'

export function useAssistantView() {
  const store = useAssistantStore()
  const error = ref<string | null>(null)

  async function loadSessions() {
    try {
      error.value = null
      store.sessions = await api.fetchSessions() as never
    } catch (e) {
      error.value = describeError(e)
    }
  }

  async function selectSession(session: ChatSession) {
    store.activeSessionId = session.id
    try {
      error.value = null
      const detail = await api.fetchSession(session.id)
      store.messages = detail.messages as never
    } catch (e) {
      error.value = describeError(e)
    }
  }

  async function startNewSession() {
    try {
      const session = await api.createSession({ title: '新会话' }) as unknown as ChatSession
      await loadSessions()
      await selectSession(session)
    } catch (e) {
      error.value = describeError(e)
    }
  }

  async function sendMessage(text: string) {
    if (!store.activeSessionId) {
      try {
        await startNewSession()
      } catch {
        return
      }
    }
    if (!store.activeSessionId) return

    const localId = `local-${Date.now()}`
    const userMsg = {
      id: localId,
      message_id: localId,
      session_id: store.activeSessionId,
      role: 'user',
      content: text,
      created_at: new Date().toISOString(),
    } satisfies ChatMessage
    store.messages = [...store.messages as never[], userMsg]
    store.loading = true
    try {
      error.value = null
      const reply = await api.chat(store.activeSessionId, text)
      store.messages = (reply.messages.length > 0
        ? reply.messages
        : [...store.messages as never[], reply.message]) as never
      await loadSessions()
    } catch (e) {
      error.value = describeError(e)
    } finally {
      store.loading = false
    }
  }

  onMounted(() => {
    void loadSessions()
  })

  return {
    store,
    error,
    selectSession,
    startNewSession,
    sendMessage,
  }
}
