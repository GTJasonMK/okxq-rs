<template>
  <div class="session-list">
    <div class="sl-header">
      <span class="sl-title">会话</span>
      <button class="icon-btn" @click="$emit('new')">+ 新建</button>
    </div>
    <div class="sl-items" v-if="sessions.length > 0">
      <div
        v-for="s in sessions"
        :key="s.id"
        class="sl-item"
        :class="{ active: activeId === s.id }"
        @click="$emit('select', s)"
      >
        <div class="sl-item-name">{{ s.title || '未命名会话' }}</div>
        <div class="sl-item-date">{{ fmtDate(s.created_at) }}</div>
      </div>
    </div>
    <div v-else class="empty-text">暂无会话</div>
  </div>
</template>

<script setup lang="ts">
import type { ChatSession } from '@/types'

defineProps<{ sessions: ChatSession[]; activeId: string | null }>()
defineEmits<{ select: [s: ChatSession]; new: [] }>()

function fmtDate(ts: string): string {
  return new Date(ts).toLocaleDateString('zh-CN', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })
}
</script>

<style scoped>
.session-list {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.sl-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border);
}
.sl-title { font-size: 13px; font-weight: 600; }
.icon-btn {
  background: none;
  border: 1px solid var(--color-border);
  padding: 2px 8px;
  border-radius: 3px;
  color: var(--color-text-secondary);
  font-size: 11px;
  cursor: pointer;
}
.icon-btn:hover { background: var(--color-bg-hover); }
.sl-items { flex: 1; overflow-y: auto; }
.sl-item {
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border);
  cursor: pointer;
}
.sl-item:hover { background: var(--color-bg-hover); }
.sl-item.active { background: var(--color-bg-active); border-left: 2px solid var(--color-accent); }
.sl-item-name { font-size: 12px; font-weight: 500; margin-bottom: 2px; }
.sl-item-date { font-size: 10px; color: var(--color-text-tertiary); }
.empty-text {
  padding: 24px;
  text-align: center;
  color: var(--color-text-tertiary);
  font-size: 13px;
}
</style>
