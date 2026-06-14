<template>
  <div class="position-table">
    <div class="pt-header">
      <span class="pt-title">持仓 ({{ positions.length }})</span>
      <span class="pt-note">平仓使用市价 reduce-only</span>
    </div>
    <div class="table-wrap">
      <table v-if="positions.length > 0">
        <thead>
          <tr>
            <th>品种</th>
            <th>方向</th>
            <th class="num">数量</th>
            <th class="num">均价</th>
            <th class="num">标记价</th>
            <th class="num">杠杆</th>
            <th class="num">保证金</th>
            <th class="num">未实现盈亏</th>
            <th class="action-head">操作</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="p in positions" :key="p.inst_id + p.pos_side">
            <td class="symbol-cell">{{ p.inst_id }}</td>
            <td>
              <span class="side-badge" :class="positionSideClass(p)">
                {{ positionSideLabel(p) }}
              </span>
            </td>
            <td class="num">{{ formatPositionSize(p.pos) }}</td>
            <td class="num">{{ formatPrice(p.avg_px) }}</td>
            <td class="num">{{ formatPrice(p.mark_px) }}</td>
            <td class="num">{{ formatLeverage(p.lever) }}</td>
            <td class="num">{{ formatMoney(p.margin) }}</td>
            <td class="num" :class="pnlColor(p.upl)">
              {{ formatMoney(p.upl) }}
              <span class="upl-pct">({{ formatPercentSimple(p.upl_ratio) }})</span>
            </td>
            <td class="action-cell">
              <template v-if="confirmingKey === positionKey(p)">
                <button
                  class="position-action danger"
                  type="button"
                  :disabled="isClosing(p)"
                  @click="confirmClose(p)"
                >
                  {{ isClosing(p) ? '提交中' : '确认' }}
                </button>
                <button
                  class="position-action"
                  type="button"
                  :disabled="isClosing(p)"
                  @click="confirmingKey = ''"
                >
                  取消
                </button>
              </template>
              <button
                v-else
                class="position-action danger"
                type="button"
                :disabled="!canClose(p) || isClosing(p)"
                :title="closeTitle(p)"
                @click="requestClose(p)"
              >
                {{ isClosing(p) ? '平仓中' : '平仓' }}
              </button>
            </td>
          </tr>
        </tbody>
      </table>
      <div v-else class="empty-text">暂无持仓</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import type { Position } from '@/types'
import { formatPrice, formatMoney } from '@/utils/format'
import { pnlColor } from '@/utils/color'

const props = withDefaults(defineProps<{
  positions: Position[]
  modeLocked?: boolean
  closingPositionKeys?: Set<string>
}>(), {
  modeLocked: false,
  closingPositionKeys: () => new Set<string>(),
})
const emit = defineEmits<{ close: [position: Position] }>()
const confirmingKey = ref('')

function formatPercentSimple(ratio: number | null): string {
  if (!isFiniteNumber(ratio)) return '--'
  return `${(ratio * 100).toFixed(2)}%`
}

function formatPositionSize(value: number | null): string {
  if (value === null) return '--'
  const size = Math.abs(value)
  if (!Number.isFinite(size)) return '--'
  return String(size)
}

function formatLeverage(value: number | null): string {
  return isFiniteNumber(value) ? `${value}x` : '--'
}

function positionSideClass(position: Position): string {
  if (position.pos_side === 'long' || position.pos_side === 'short') return position.pos_side
  return 'unknown'
}

function positionSideLabel(position: Position): string {
  if (position.pos_side === 'long') return '多'
  if (position.pos_side === 'short') return '空'
  return '--'
}

function positionKey(position: Position): string {
  return `${position.inst_id}:${position.pos_side}`
}

function isClosing(position: Position): boolean {
  return props.closingPositionKeys.has(positionKey(position))
}

function canClose(position: Position): boolean {
  return !props.modeLocked &&
    position.inst_type === 'SWAP' &&
    (position.pos_side === 'long' || position.pos_side === 'short') &&
    isFiniteNumber(position.pos) &&
    Math.abs(position.pos) > 0
}

function closeTitle(position: Position): string {
  if (props.modeLocked) return '查看模式与默认交易模式不一致，平仓已锁定'
  if (position.inst_type !== 'SWAP') return '仅支持永续合约持仓平仓'
  if (position.pos_side !== 'long' && position.pos_side !== 'short') return '持仓方向无效'
  if (!isFiniteNumber(position.pos) || Math.abs(position.pos) <= 0) return '持仓数量无效'
  return position.pos_side === 'short' ? '市价买入平空' : '市价卖出平多'
}

function requestClose(position: Position) {
  if (!canClose(position)) return
  confirmingKey.value = positionKey(position)
}

function confirmClose(position: Position) {
  if (!canClose(position)) return
  confirmingKey.value = ''
  emit('close', position)
}

function isFiniteNumber(value: number | null): value is number {
  return typeof value === 'number' && Number.isFinite(value)
}
</script>

<style scoped>
.position-table {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  min-height: 0;
}
.pt-header {
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.pt-title { font-size: 13px; font-weight: 600; }
.pt-note {
  color: var(--color-text-tertiary);
  font-size: 11px;
  white-space: nowrap;
}
.table-wrap {
  overflow: auto;
  min-height: 0;
  max-height: 260px;
}
table { width: 100%; border-collapse: collapse; font-size: 12px; }
th {
  text-align: left;
  padding: 6px 10px;
  color: var(--color-text-tertiary);
  font-weight: 500;
  font-size: 11px;
  white-space: nowrap;
}
th.num { text-align: right; }
th.action-head { text-align: right; }
td {
  padding: 5px 10px;
  border-top: 1px solid var(--color-border);
  white-space: nowrap;
}
td.num { text-align: right; font-variant-numeric: tabular-nums; }
.action-cell {
  text-align: right;
  min-width: 106px;
}
.symbol-cell { font-weight: 600; }
.side-badge {
  display: inline-block;
  padding: 1px 6px;
  border-radius: 3px;
  font-size: 11px;
  font-weight: 500;
}
.side-badge.long { background: rgba(38,166,154,0.15); color: var(--color-positive); }
.side-badge.short { background: rgba(239,83,80,0.15); color: var(--color-negative); }
.upl-pct { font-size: 11px; color: var(--color-text-tertiary); }
.positive { color: var(--color-positive); }
.negative { color: var(--color-negative); }
.position-action {
  margin-left: 4px;
  padding: 2px 7px;
  border: 1px solid var(--color-border);
  border-radius: 3px;
  background: var(--color-bg-primary);
  color: var(--color-text-secondary);
  font-size: 11px;
  cursor: pointer;
}
.position-action:hover {
  border-color: var(--color-accent);
  color: var(--color-text-primary);
}
.position-action.danger {
  border-color: rgba(239,83,80,0.35);
  color: var(--color-negative);
}
.position-action.danger:hover {
  border-color: var(--color-negative);
  background: rgba(239,83,80,0.08);
}
.position-action:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}
.empty-text {
  padding: 24px;
  text-align: center;
  color: var(--color-text-tertiary);
  font-size: 13px;
}
</style>
