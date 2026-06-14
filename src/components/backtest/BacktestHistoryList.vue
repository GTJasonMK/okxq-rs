<template>
  <section class="vb-history">
    <div class="vb-history-title">历史回测</div>
    <div class="vb-history-body">
      <div
        v-for="result in history"
        :key="result.result_id"
        class="vb-history-item"
        :class="{ active: activeResultId === result.result_id }"
        @click="emit('select', result)"
      >
        <span class="hb-name">{{ result.strategy_name || result.strategy_id }}</span>
        <span class="hb-symbol">{{ result.symbol }}</span>
        <span class="hb-return" :class="pnlColor(result.total_return_pct)">
          {{ result.total_return_pct?.toFixed(1) }}%
        </span>
        <span v-if="historyIntegrityBadge(result)" class="hb-integrity" :class="historyIntegrityClass(result)">
          {{ historyIntegrityBadge(result) }}
        </span>
        <button
          type="button"
          class="hb-delete"
          :disabled="deletingResultId === result.result_id"
          @click.stop="emit('delete', result)"
        >
          {{ deletingResultId === result.result_id ? '删除中' : '删除' }}
        </button>
      </div>
      <div v-if="history.length === 0" class="empty-text">暂无回测记录</div>
    </div>
  </section>
</template>

<script setup lang="ts">
import type { BacktestResult } from '@/types'
import { pnlColor } from '@/utils/color'

defineProps<{
  activeResultId?: string
  deletingResultId: string | null
  history: BacktestResult[]
}>()

const emit = defineEmits<{
  delete: [result: BacktestResult]
  select: [result: BacktestResult]
}>()

function historyIntegrityBadge(result: BacktestResult) {
  const status = resultIntegrityStatus(result)
  if (status === 'invalid') return '异常'
  if (status === 'warning') return '复核'
  if (status === 'unverified') return '旧'
  return ''
}

function historyIntegrityClass(result: BacktestResult) {
  const status = resultIntegrityStatus(result)
  if (status === 'invalid') return 'invalid'
  if (status === 'warning') return 'warning'
  if (status === 'unverified') return 'unverified'
  return ''
}

function resultIntegrityStatus(result: BacktestResult) {
  const status = result.backtest_result_integrity?.status
  return typeof status === 'string' ? status.trim() : ''
}
</script>

<style scoped>
.vb-history {
  display: flex;
  flex-direction: column;
  min-height: 0;
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
}
.vb-history-title {
  flex: 0 0 auto;
  padding: 8px 12px;
  font-size: 13px;
  font-weight: 600;
  border-bottom: 1px solid var(--color-border);
}
.vb-history-body {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
}
.vb-history-item {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto auto auto auto;
  gap: 6px;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border);
  cursor: pointer;
  font-size: 12px;
  transition: background 0.1s;
}
.vb-history-item:hover { background: var(--color-bg-hover); }
.vb-history-item.active { background: var(--color-bg-active); }
.hb-name { font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.hb-symbol { font-size: 10px; color: var(--color-text-tertiary); }
.hb-return { font-weight: 600; }
.hb-integrity {
  border: 1px solid var(--color-border);
  border-radius: 999px;
  font-size: 10px;
  line-height: 1.2;
  padding: 1px 5px;
}
.hb-integrity.invalid {
  border-color: rgba(229, 62, 62, 0.34);
  color: var(--color-negative);
}
.hb-integrity.warning,
.hb-integrity.unverified {
  border-color: rgba(183, 121, 31, 0.34);
  color: #b7791f;
}
.hb-delete {
  min-width: 46px;
  padding: 3px 7px;
  border: 1px solid rgba(239, 83, 80, 0.38);
  border-radius: 4px;
  background: transparent;
  color: var(--color-negative);
  font-size: 11px;
  line-height: 1.3;
  cursor: pointer;
}
.hb-delete:hover:not(:disabled) {
  background: rgba(239, 83, 80, 0.12);
}
.hb-delete:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
.empty-text {
  padding: 16px;
  text-align: center;
  color: var(--color-text-tertiary);
  font-size: 12px;
}
</style>
