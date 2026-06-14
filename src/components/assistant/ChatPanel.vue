<template>
  <div class="chat-panel" ref="panelRef">
    <div class="cp-messages">
      <div v-if="messages.length === 0 && !loading" class="cp-empty">
        AI 助手已就绪，请输入您的问题
      </div>
      <div v-for="m in messages" :key="m.id" class="cp-msg" :class="m.role">
        <div class="cp-msg-role">{{ roleLabel(m.role) }}</div>
        <div class="cp-msg-content" v-text="m.content"></div>
        <div class="cp-msg-time">{{ fmtTime(m.created_at) }}</div>
      </div>
      <div v-if="loading" class="cp-msg assistant">
        <div class="cp-msg-content typing">思考中<span class="dots">...</span></div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, nextTick } from 'vue'
import type { ChatMessage } from '@/types'

const props = defineProps<{ messages: ChatMessage[]; loading: boolean }>()
const panelRef = ref<HTMLDivElement>()

function roleLabel(r: string): string {
  return { user: '用户', assistant: 'AI', system: '系统' }[r] || r
}

function fmtTime(ts: string): string {
  return new Date(ts).toLocaleTimeString('zh-CN', { hour12: false })
}

watch(() => props.messages.length, async () => {
  await nextTick()
  if (panelRef.value) {
    panelRef.value.scrollTop = panelRef.value.scrollHeight
  }
})
</script>

<style scoped>
.chat-panel {
  flex: 1;
  overflow-y: auto;
  min-height: 0;
}
.cp-empty {
  padding: 40px 20px;
  text-align: center;
  color: var(--color-text-tertiary);
  font-size: 14px;
}
.cp-msg { padding: 10px 14px; border-bottom: 1px solid var(--color-border); }
.cp-msg.user { background: rgba(41,98,255,0.05); }
.cp-msg.assistant { background: var(--color-bg-secondary); }
.cp-msg-role { font-size: 10px; font-weight: 600; color: var(--color-text-tertiary); margin-bottom: 4px; }
.cp-msg-content { font-size: 13px; line-height: 1.5; white-space: pre-wrap; }
.cp-msg-content.typing { color: var(--color-text-tertiary); font-style: italic; }
.cp-msg-time { font-size: 10px; color: var(--color-text-tertiary); margin-top: 4px; }
.dots { animation: blink 1s steps(1) infinite; }
@keyframes blink { 50% { opacity: 0; } }
</style>
