<template>
  <div class="journal-list">
    <div class="jl-header">
      <span class="jl-title">日志列表 ({{ entries.length }})</span>
    </div>
    <div class="jl-items" v-if="entries.length > 0">
      <div
        v-for="e in entries"
        :key="e.id"
        class="jl-item"
        :class="{ active: activeId === e.id }"
        @click="$emit('select', e)"
      >
        <div class="jl-item-header">
          <span class="jl-item-title">{{ e.title }}</span>
          <span class="jl-item-date">{{ formatDate(e.created_at) }}</span>
        </div>
        <div class="jl-item-meta">
          <span v-if="e.inst_id" class="jl-tag">{{ e.inst_id }}</span>
          <span v-if="e.mode" class="jl-tag mode">{{ e.mode === 'live' ? '实盘' : '模拟' }}</span>
          <span class="jl-rating">{{ '★'.repeat(e.rating || 0) }}{{ '☆'.repeat(5 - (e.rating || 0)) }}</span>
          <span v-if="e.pnl_snapshot" class="jl-pnl" :class="pnlColor(e.pnl_snapshot)">
            {{ formatMoney(e.pnl_snapshot) }}
          </span>
        </div>
        <div class="jl-item-preview">{{ e.content?.slice(0, 120) || '无内容' }}</div>
      </div>
    </div>
    <div v-else class="empty-text">暂无日志，点击右侧新建</div>
  </div>
</template>

<script setup lang="ts">
import type { JournalEntry } from '@/types'
import { formatMoney } from '@/utils/format'
import { pnlColor } from '@/utils/color'

defineProps<{ entries: JournalEntry[]; activeId: string | null }>()
defineEmits<{ select: [entry: JournalEntry] }>()

function formatDate(ts: string): string {
  const d = new Date(ts)
  return d.toLocaleDateString('zh-CN', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })
}
</script>

<style scoped>
.journal-list {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
.jl-header {
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border);
}
.jl-title { font-size: 13px; font-weight: 600; }
.jl-items {
  flex: 1;
  overflow-y: auto;
}
.jl-item {
  padding: 10px 12px;
  border-bottom: 1px solid var(--color-border);
  cursor: pointer;
  transition: background 0.1s;
}
.jl-item:hover { background: var(--color-bg-hover); }
.jl-item.active { background: var(--color-bg-active); border-left: 2px solid var(--color-accent); }
.jl-item-header {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  margin-bottom: 4px;
}
.jl-item-title { font-size: 13px; font-weight: 600; }
.jl-item-date { font-size: 11px; color: var(--color-text-tertiary); }
.jl-item-meta { display: flex; gap: 6px; align-items: center; margin-bottom: 4px; }
.jl-tag {
  padding: 0 5px;
  background: var(--color-bg-primary);
  border: 1px solid var(--color-border);
  border-radius: 3px;
  font-size: 10px;
  color: var(--color-text-secondary);
}
.jl-tag.mode { color: var(--color-accent); }
.jl-rating { font-size: 11px; color: #ff9800; }
.jl-pnl { font-size: 12px; font-weight: 500; }
.jl-item-preview {
  font-size: 12px;
  color: var(--color-text-tertiary);
  line-height: 1.4;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.positive { color: var(--color-positive); }
.negative { color: var(--color-negative); }
.empty-text {
  padding: 24px;
  text-align: center;
  color: var(--color-text-tertiary);
  font-size: 13px;
}
</style>
