<template>
  <div class="view-assistant">
    <h2 class="va-title">AI 助手</h2>
    <div v-if="error" class="va-error">{{ error }}</div>
    <div class="va-grid">
      <SessionList
        :sessions="store.sessions as never"
        :active-id="store.activeSessionId"
        @select="selectSession"
        @new="startNewSession"
      />
      <div class="va-chat-area">
        <ChatPanel :messages="store.messages as never" :loading="store.loading" />
        <ChatInput :disabled="store.loading" @send="sendMessage" />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useAssistantView } from '@/composables/useAssistantView'
import SessionList from '@/components/assistant/SessionList.vue'
import ChatPanel from '@/components/assistant/ChatPanel.vue'
import ChatInput from '@/components/assistant/ChatInput.vue'

const {
  store,
  error,
  selectSession,
  startNewSession,
  sendMessage,
} = useAssistantView()
</script>

<style scoped>
.view-assistant { display: flex; flex-direction: column; height: 100%; gap: 8px; }
.va-title { font-size: 16px; font-weight: 600; margin: 0; }
.va-error {
  padding: 8px 10px;
  border: 1px solid rgba(239,83,80,0.35);
  border-radius: 6px;
  background: rgba(239,83,80,0.08);
  color: var(--color-negative);
  font-size: 12px;
}
.va-grid {
  flex: 1;
  display: grid;
  grid-template-columns: 240px 1fr;
  gap: 8px;
  min-height: 0;
}
.va-chat-area {
  display: flex;
  flex-direction: column;
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
}
</style>
