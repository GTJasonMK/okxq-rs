<template>
  <section class="lp-panel">
    <div class="lp-header">
      <div>
        <span class="lp-title">仓位</span>
        <span class="lp-subtitle">{{ modeLabel }} · OKX 当前持仓 {{ sortedPositions.length }} · 历史 {{ historyRows.length }}</span>
      </div>
      <span class="lp-source">交易所持仓为准</span>
    </div>

    <section class="lp-section">
      <div class="lp-section-title">当前持仓</div>
      <div class="lp-wrap">
        <table v-if="sortedPositions.length > 0">
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
            </tr>
          </thead>
          <tbody>
            <tr v-for="position in sortedPositions" :key="positionKey(position)">
              <td class="symbol-cell">{{ position.inst_id || '--' }}</td>
              <td>
                <span class="side-badge" :class="positionSideClass(position.pos_side)">
                  {{ positionSideLabel(position.pos_side) }}
                </span>
              </td>
              <td class="num">{{ formatLiveAbsoluteQuantity(position.pos) }}</td>
              <td class="num">{{ formatPrice(position.avg_px) }}</td>
              <td class="num">{{ formatPrice(position.mark_px) }}</td>
              <td class="num">{{ formatLeverage(position.lever) }}</td>
              <td class="num">{{ formatMoney(position.margin) }}</td>
              <td class="num" :class="livePnlClass(position.upl)">
                <span class="pnl-main">{{ formatMoney(position.upl) }}</span>
                <span class="pct">{{ formatPercent(position.upl_ratio) }}</span>
              </td>
            </tr>
          </tbody>
        </table>
        <div v-else class="empty-state">
          <strong>当前 OKX 账户暂无合约持仓</strong>
          <span>这里直接读取交易所当前持仓，不依赖策略是否正在运行。</span>
          <em>如果刚刚成交但这里为空，请等待下一次私有账户刷新。</em>
        </div>
      </div>
    </section>

    <section class="lp-section">
      <div class="lp-section-title">历史仓位</div>
      <div
        ref="historyViewport"
        class="lp-wrap history-wrap"
        @scroll="syncHistoryViewport"
      >
        <table v-if="historyRows.length > 0">
          <thead>
            <tr>
              <th>时间</th>
              <th>品种</th>
              <th>方向</th>
              <th>动作</th>
              <th class="num">价格</th>
              <th class="num">数量</th>
              <th>状态</th>
              <th>动作</th>
            </tr>
          </thead>
          <tbody>
            <tr v-if="historyWindow.beforeHeight > 0" class="lp-virtual-spacer-row">
              <td
                :colspan="HISTORY_TABLE_COLUMN_COUNT"
                class="lp-virtual-spacer-cell"
                :style="{ height: `${historyWindow.beforeHeight}px` }"
              ></td>
            </tr>
            <tr v-for="row in visibleHistoryRows" :key="row.key">
              <td class="time-cell" :title="formatDateTime(row.timestamp)">{{ formatDateTime(row.timestamp) }}</td>
              <td class="symbol-cell">{{ row.instId || '--' }}</td>
              <td>
                <span class="side-badge" :class="positionSideClass(row.positionSide)">
                  {{ positionSideLabel(row.positionSide) }}
                </span>
              </td>
              <td>
                <span class="action-badge" :class="row.actionClass">{{ row.actionLabel }}</span>
              </td>
              <td class="num">{{ formatPrice(row.price) }}</td>
              <td class="num">{{ formatLiveAbsoluteQuantity(row.quantity) }}</td>
              <td>
                <span class="status-badge" :class="row.statusClass">{{ row.statusLabel }}</span>
              </td>
              <td class="action-cell">
                <div>{{ row.actionName }}</div>
                <div v-if="row.note" class="note">{{ row.note }}</div>
              </td>
            </tr>
            <tr v-if="historyWindow.afterHeight > 0" class="lp-virtual-spacer-row">
              <td
                :colspan="HISTORY_TABLE_COLUMN_COUNT"
                class="lp-virtual-spacer-cell"
                :style="{ height: `${historyWindow.afterHeight}px` }"
              ></td>
            </tr>
          </tbody>
        </table>
        <div v-else class="empty-state">
          <strong>当前范围暂无历史仓位记录</strong>
          <span>策略产生开仓或平仓记录后，会在这里按最新时间展示仓位变动。</span>
          <em>风控拦截和纯监控记录不会被当成历史仓位。</em>
        </div>
      </div>
    </section>
  </section>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { LiveOrder, Position, TradingMode } from '@/types'
import { formatMoney, formatPercent, formatPrice } from '@/utils/format'
import { compareOrdersByLatest, finiteNumber, orderTimestamp } from '@/utils/liveStrategyCore'
import {
  formatLiveAbsoluteQuantity,
  formatLiveDateTime as formatDateTime,
  livePnlClass,
} from '@/utils/liveStrategyDisplay/format'
import {
  isContractLiveOrder,
  isLiveOrderBlocked,
  isLiveOrderExit,
  isLiveOrderPending,
  isLivePositionEntryAction,
  liveOrderHistoryActionLabel,
  liveOrderHistoryStatusClass,
  liveOrderPositionSide,
  orderStatusLabel,
  type LiveOrderPositionSide,
} from '@/utils/liveStrategyDisplay/orders'
import { liveActionLabel } from '@/utils/liveOrderActions'

type HistoryRow = {
  key: string
  timestamp: number
  instId: string
  positionSide: LiveOrderPositionSide
  actionLabel: string
  actionClass: string
  price: number | null
  quantity: number | null
  statusLabel: string
  statusClass: string
  actionName: string
  note: string
}

const HISTORY_ROW_ESTIMATED_HEIGHT = 40
const HISTORY_OVERSCAN_ROWS = 8
const HISTORY_DEFAULT_VIEWPORT_HEIGHT = 280
const HISTORY_VIRTUALIZE_THRESHOLD = 80
const HISTORY_TABLE_COLUMN_COUNT = 8

const props = defineProps<{
  mode: TradingMode
  positions: Position[]
  historyOrders: LiveOrder[]
}>()
const historyViewport = ref<HTMLElement | null>(null)
const historyScrollTop = ref(0)
const historyViewportHeight = ref(HISTORY_DEFAULT_VIEWPORT_HEIGHT)

const modeLabel = computed(() => props.mode === 'live' ? '实盘' : '模拟盘')
const sortedPositions = computed(() =>
  [...props.positions]
    .filter(isOpenPosition)
    .sort(comparePositions)
)
const historyRows = computed<HistoryRow[]>(() =>
  [...props.historyOrders]
    .filter(isPositionHistoryOrder)
    .sort(compareOrdersByLatest)
    .map(historyRowFromOrder)
)
const historyWindow = computed(() => {
  const total = historyRows.value.length
  if (total === 0 || total <= HISTORY_VIRTUALIZE_THRESHOLD) {
    return { start: 0, end: total, beforeHeight: 0, afterHeight: 0 }
  }
  const rowHeight = HISTORY_ROW_ESTIMATED_HEIGHT
  const visibleRows = Math.max(1, Math.ceil(historyViewportHeight.value / rowHeight))
  const firstVisible = Math.floor(historyScrollTop.value / rowHeight)
  const start = Math.max(0, firstVisible - HISTORY_OVERSCAN_ROWS)
  const end = Math.min(total, firstVisible + visibleRows + HISTORY_OVERSCAN_ROWS)
  return {
    start,
    end,
    beforeHeight: start * rowHeight,
    afterHeight: Math.max(0, (total - end) * rowHeight),
  }
})
const visibleHistoryRows = computed(() =>
  historyRows.value.slice(historyWindow.value.start, historyWindow.value.end)
)

function syncHistoryViewport() {
  const viewport = historyViewport.value
  if (!viewport) return
  historyScrollTop.value = Math.max(0, viewport.scrollTop)
  historyViewportHeight.value = viewport.clientHeight || HISTORY_DEFAULT_VIEWPORT_HEIGHT
}

function clampHistoryScroll() {
  const viewport = historyViewport.value
  if (!viewport) return
  const maxScrollTop = Math.max(
    0,
    historyRows.value.length * HISTORY_ROW_ESTIMATED_HEIGHT - historyViewportHeight.value,
  )
  if (viewport.scrollTop > maxScrollTop) {
    viewport.scrollTop = maxScrollTop
  }
  syncHistoryViewport()
}

function isOpenPosition(position: Position): boolean {
  const pos = finiteNumber(position.pos)
  return pos !== null && Math.abs(pos) > 0
}

function comparePositions(left: Position, right: Position): number {
  return Math.abs(right.upl ?? 0) - Math.abs(left.upl ?? 0)
    || (left.inst_id || '').localeCompare(right.inst_id || '')
    || (left.pos_side || '').localeCompare(right.pos_side || '')
}

function isPositionHistoryOrder(order: LiveOrder): boolean {
  if (isLiveOrderBlocked(order)) return false
  if (!order.success && !isLiveOrderPending(order)) return false
  if (isLiveOrderExit(order)) return true
  return isContractLiveOrder(order) && isLivePositionEntryAction(order.action)
}

function historyRowFromOrder(order: LiveOrder): HistoryRow {
  const exit = isLiveOrderExit(order)
  const positionSide = liveOrderPositionSide(order, exit)
  const actionLabel = liveOrderHistoryActionLabel(order, exit, positionSide)
  return {
    key: order.ord_id || `${order.id}-${orderTimestamp(order)}`,
    timestamp: orderTimestamp(order),
    instId: order.inst_id || order.symbol,
    positionSide,
    actionLabel,
    actionClass: exit ? 'exit' : positionSide,
    price: finiteNumber(order.avg_fill_price)
      ?? finiteNumber(order.px)
      ?? firstFiniteNumber(order.arrival_mid_px, order.arrival_ask_px, order.arrival_bid_px),
    quantity: finiteNumber(order.filled_size) ?? finiteNumber(order.sz),
    statusLabel: orderStatusLabel(order.status),
    statusClass: liveOrderHistoryStatusClass(order),
    actionName: liveActionLabel(order.action),
    note: order.error_message || '',
  }
}

function positionKey(position: Position): string {
  return `${position.inst_id}:${position.pos_side}`
}

function positionSideLabel(side: string): string {
  if (side === 'long') return '多'
  if (side === 'short') return '空'
  return '--'
}

function positionSideClass(side: string): string {
  if (side === 'long' || side === 'short') return side
  return 'flat'
}

function formatLeverage(value: number | null): string {
  const parsed = finiteNumber(value)
  return parsed === null ? '--' : `${parsed}x`
}

function firstFiniteNumber(...values: Array<number | null>): number | null {
  for (const value of values) {
    const parsed = finiteNumber(value)
    if (parsed !== null) return parsed
  }
  return null
}

onMounted(() => {
  void nextTick(syncHistoryViewport)
  window.addEventListener('resize', syncHistoryViewport)
})

onBeforeUnmount(() => {
  window.removeEventListener('resize', syncHistoryViewport)
})

watch(() => historyRows.value.length, () => {
  void nextTick(clampHistoryScroll)
})
</script>

<style scoped>
.lp-panel {
  display: flex;
  flex-direction: column;
  min-height: 100%;
  overflow: hidden;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: var(--color-bg-secondary);
}
.lp-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border);
}
.lp-title {
  display: block;
  font-size: 13px;
  font-weight: 600;
}
.lp-subtitle,
.lp-source {
  color: var(--color-text-tertiary);
  font-size: 11px;
}
.lp-source {
  flex: 0 0 auto;
  margin-top: 1px;
}
.lp-section {
  min-height: 0;
  border-top: 1px solid var(--color-border);
}
.lp-section:first-of-type { border-top: none; }
.lp-section-title {
  padding: 7px 12px;
  border-bottom: 1px solid var(--color-border);
  background: rgba(148,163,184,0.05);
  color: var(--color-text-secondary);
  font-size: 12px;
  font-weight: 600;
}
.lp-wrap {
  overflow: auto;
  min-height: 0;
  max-height: 220px;
}
.history-wrap {
  max-height: 280px;
}
table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
}
th {
  padding: 6px 10px;
  color: var(--color-text-tertiary);
  font-size: 11px;
  font-weight: 500;
  text-align: left;
  white-space: nowrap;
}
th.num,
td.num {
  text-align: right;
}
td {
  padding: 6px 10px;
  border-top: 1px solid var(--color-border);
  white-space: nowrap;
}
td.num {
  font-variant-numeric: tabular-nums;
}
.lp-virtual-spacer-cell {
  height: 0;
  padding: 0;
  border-top: 0;
}
.symbol-cell {
  font-weight: 600;
}
.time-cell {
  color: var(--color-text-secondary);
  font-size: 11px;
}
.pnl-main {
  display: block;
  line-height: 1.35;
}
.pct {
  display: block;
  margin-top: 1px;
  color: var(--color-text-tertiary);
  font-size: 10px;
  line-height: 1.25;
}
.side-badge,
.action-badge,
.status-badge {
  display: inline-block;
  padding: 1px 6px;
  border-radius: 3px;
  font-size: 11px;
  font-weight: 600;
}
.side-badge.long,
.action-badge.long {
  background: rgba(38,166,154,0.15);
  color: var(--color-positive);
}
.side-badge.short,
.action-badge.short {
  background: rgba(239,83,80,0.15);
  color: var(--color-negative);
}
.side-badge.flat {
  background: rgba(148,163,184,0.12);
  color: var(--color-text-tertiary);
}
.action-badge.exit {
  background: rgba(255,183,77,0.16);
  color: var(--color-warning);
}
.status-badge.filled,
.status-badge.closed {
  background: rgba(38,166,154,0.12);
  color: var(--color-positive);
}
.status-badge.pending {
  background: rgba(41,98,255,0.13);
  color: var(--color-accent);
}
.status-badge.failed {
  background: rgba(239,83,80,0.12);
  color: var(--color-negative);
}
.status-badge.neutral {
  background: rgba(148,163,184,0.12);
  color: var(--color-text-secondary);
}
.action-cell {
  min-width: 160px;
  white-space: normal;
}
.note {
  margin-top: 2px;
  color: var(--color-text-tertiary);
  font-size: 11px;
}
.positive { color: var(--color-positive); }
.negative { color: var(--color-negative); }
.flat { color: var(--color-text-tertiary); }
.empty-state {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 22px 12px;
  color: var(--color-text-tertiary);
  font-size: 12px;
  text-align: center;
}
.empty-state strong {
  color: var(--color-text-secondary);
  font-size: 13px;
}
.empty-state em {
  font-style: normal;
  line-height: 1.45;
}
</style>
