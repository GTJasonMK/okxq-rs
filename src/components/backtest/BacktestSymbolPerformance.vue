<template>
  <div class="symbol-performance">
    <div class="sp-header">
      <span class="sp-title">币种收益 ({{ rows.length }})</span>
      <span v-if="truncated" class="sp-subtitle warning">
        基于最近 {{ displayedEvents }} / 共 {{ totalEvents }} 条事件统计
      </span>
      <span v-else class="sp-subtitle">已实现盈亏 · 按平仓事件聚合</span>
    </div>
    <div v-if="rows.length > 0" class="sp-summary">
      <div>
        <span>总盈亏</span>
        <strong :class="pnlColor(totalPnl)">{{ formatMoney(totalPnl) }}</strong>
      </div>
      <div>
        <span>盈利币种</span>
        <strong>{{ winningSymbols }}</strong>
      </div>
      <div>
        <span>亏损币种</span>
        <strong>{{ losingSymbols }}</strong>
      </div>
      <div>
        <span>总交易</span>
        <strong>{{ totalClosedTrades }}</strong>
      </div>
    </div>
    <div class="sp-table-wrap">
      <table v-if="rows.length > 0">
        <thead>
          <tr>
            <th>币种</th>
            <th class="num">交易</th>
            <th class="num">胜率</th>
            <th class="num">已实现盈亏</th>
            <th class="num">资金贡献</th>
            <th class="num">成交收益率</th>
            <th class="num">多单</th>
            <th class="num">空单</th>
            <th class="num">手续费</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="row in rows" :key="row.symbol">
            <td>
              <div class="sp-symbol">
                <strong>{{ row.baseSymbol }}</strong>
                <span>{{ row.symbol }}</span>
              </div>
            </td>
            <td class="num">{{ row.closedTrades }}</td>
            <td class="num">{{ formatPct(row.winRatePct) }}</td>
            <td class="num" :class="pnlColor(row.realizedPnl)">{{ formatMoney(row.realizedPnl) }}</td>
            <td class="num" :class="pnlColor(row.capitalContributionPct)">
              {{ formatPct(row.capitalContributionPct) }}
            </td>
            <td class="num" :class="pnlColor(row.turnoverReturnPct)">
              {{ formatPct(row.turnoverReturnPct) }}
            </td>
            <td class="num" :class="pnlColor(row.longPnl)">{{ formatMoney(row.longPnl) }}</td>
            <td class="num" :class="pnlColor(row.shortPnl)">{{ formatMoney(row.shortPnl) }}</td>
            <td class="num">{{ formatMoney(row.commission) }}</td>
          </tr>
        </tbody>
      </table>
      <div v-else class="empty-text">暂无已平仓收益</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { pnlColor } from '@/utils/color'
import { formatMoney } from '@/utils/format'
import type { BacktestSymbolPerformanceRow } from '@/utils/backtestSymbolPerformance'

const props = defineProps<{
  rows: BacktestSymbolPerformanceRow[]
  totalEvents?: number
  displayedEvents?: number
  truncated?: boolean
}>()

const totalPnl = computed(() => props.rows.reduce((sum, row) => sum + row.realizedPnl, 0))
const winningSymbols = computed(() => props.rows.filter(row => row.realizedPnl > 0).length)
const losingSymbols = computed(() => props.rows.filter(row => row.realizedPnl < 0).length)
const totalClosedTrades = computed(() => props.rows.reduce((sum, row) => sum + row.closedTrades, 0))
const totalEvents = computed(() => props.totalEvents ?? props.displayedEvents ?? 0)
const displayedEvents = computed(() => props.displayedEvents ?? 0)
const truncated = computed(() => props.truncated || totalEvents.value > displayedEvents.value)

function formatPct(value: number) {
  if (!Number.isFinite(value)) return '--'
  return `${value.toFixed(2)}%`
}
</script>

<style scoped>
.symbol-performance {
  min-width: 0;
  overflow: hidden;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: var(--color-bg-secondary);
}
.sp-header {
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
  gap: 8px;
  min-width: 0;
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border);
}
.sp-title {
  flex: 0 0 auto;
  font-size: 13px;
  font-weight: 600;
}
.sp-subtitle {
  min-width: 0;
  color: var(--color-text-tertiary);
  font-size: 11px;
}
.sp-subtitle.warning {
  color: var(--color-warning);
}
.sp-summary {
  display: grid;
  grid-template-columns: repeat(4, minmax(92px, 1fr));
  gap: 1px;
  border-bottom: 1px solid var(--color-border);
  background: var(--color-border);
}
.sp-summary > div {
  min-width: 0;
  padding: 7px 10px;
  background: var(--color-bg-secondary);
}
.sp-summary span {
  display: block;
  margin-bottom: 2px;
  color: var(--color-text-tertiary);
  font-size: 11px;
  line-height: 1.2;
}
.sp-summary strong {
  display: block;
  color: var(--color-text-primary);
  font-size: 13px;
  font-variant-numeric: tabular-nums;
  line-height: 1.2;
  overflow-wrap: anywhere;
}
.sp-summary strong.positive {
  color: var(--color-positive);
}
.sp-summary strong.negative {
  color: var(--color-negative);
}
.sp-table-wrap {
  max-height: 420px;
  overflow: auto;
}
table {
  width: 100%;
  min-width: 860px;
  border-collapse: collapse;
  font-size: 12px;
}
th {
  position: sticky;
  top: 0;
  z-index: 1;
  padding: 6px 10px;
  background: var(--color-bg-secondary);
  color: var(--color-text-tertiary);
  font-size: 11px;
  font-weight: 500;
  text-align: left;
  white-space: nowrap;
}
th.num {
  text-align: right;
}
td {
  padding: 6px 10px;
  border-top: 1px solid var(--color-border);
  white-space: nowrap;
}
td.num {
  text-align: right;
  font-variant-numeric: tabular-nums;
}
.sp-symbol {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
}
.sp-symbol strong {
  color: var(--color-text-primary);
  font-size: 12px;
  line-height: 1.2;
}
.sp-symbol span {
  color: var(--color-text-tertiary);
  font-size: 10px;
  line-height: 1.2;
}
.positive {
  color: var(--color-positive);
}
.negative {
  color: var(--color-negative);
}
.empty-text {
  padding: 24px;
  color: var(--color-text-tertiary);
  font-size: 13px;
  text-align: center;
}
@media (max-width: 720px) {
  .sp-summary {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
</style>
