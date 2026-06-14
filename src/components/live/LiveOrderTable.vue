<template>
  <div class="lo-table" :class="{ 'charts-only': !showTable, 'table-only': !showCharts }">
    <div class="lo-header">
      <span class="lo-title">策略订单 ({{ orders.length }})</span>
      <div class="lo-summary" aria-label="订单摘要">
        <span>最新优先</span>
        <span>开仓 {{ orderSummary.entries }}</span>
        <span>平仓 {{ orderSummary.exits }}</span>
        <span>拦截 {{ orderSummary.blocked }}</span>
        <span>失败 {{ orderSummary.failed }}</span>
      </div>
    </div>
    <div v-if="showCharts && displayOrders.length > 0" class="lo-charts">
      <section class="lo-chart-card">
        <div class="lo-chart-head">
          <span>动作分布</span>
          <strong>{{ displayOrders.length }} 条</strong>
        </div>
        <div class="lo-mix-track" role="img" :aria-label="orderMixLabel">
          <i
            v-for="segment in orderMixSegments"
            :key="segment.key"
            :class="segment.key"
            :style="{ width: segment.width }"
            :title="`${segment.label} ${segment.count}`"
          ></i>
        </div>
        <div class="lo-mix-legend">
          <span v-for="segment in orderMixSegments" :key="segment.key">
            <i :class="segment.key"></i>{{ segment.label }} {{ segment.count }}
          </span>
          <span v-if="orderSummary.failed > 0"><i class="failed"></i>失败 {{ orderSummary.failed }}</span>
        </div>
      </section>
      <section class="lo-chart-card">
        <div class="lo-chart-head">
          <span>最近动作</span>
          <strong>{{ orderTimelineBars.length }} 笔</strong>
        </div>
        <div class="lo-timeline" role="img" aria-label="最近订单动作序列">
          <i
            v-for="bar in orderTimelineBars"
            :key="bar.key"
            :class="bar.kind"
            :style="{ height: bar.height }"
            :title="bar.title"
          ></i>
        </div>
      </section>
    </div>
    <div
      v-if="showTable"
      ref="orderViewport"
      class="lo-wrap"
      @scroll="syncOrderViewport"
    >
      <table v-if="displayOrders.length > 0">
        <thead>
          <tr>
            <th>订单ID</th>
            <th>品种</th>
            <th>动作</th>
            <th class="num">均价</th>
            <th class="num">成交/委托</th>
            <th>动作/备注</th>
            <th>状态</th>
            <th>动作时间</th>
          </tr>
        </thead>
        <tbody>
          <tr v-if="tableWindow.beforeHeight > 0" class="lo-virtual-spacer-row">
            <td
              :colspan="ORDER_TABLE_COLUMN_COUNT"
              class="lo-virtual-spacer-cell"
              :style="{ height: `${tableWindow.beforeHeight}px` }"
            ></td>
          </tr>
          <tr v-for="o in visibleTableOrders" :key="orderKey(o)">
            <td class="id-cell">{{ orderLabel(o) }}</td>
            <td>{{ o.inst_id }}</td>
            <td>
              <span class="action-badge" :class="orderActionClass(o)">{{ orderActionLabel(o) }}</span>
            </td>
            <td class="num price-cell">
              <span>{{ formatPrice(orderDisplayPrice(o)) }}</span>
              <div v-if="orderPriceNote(o)" class="note">{{ orderPriceNote(o) }}</div>
            </td>
            <td class="num">{{ orderDisplaySize(o) }}</td>
            <td class="action-cell">
              <div>{{ liveActionLabel(o.action) }}</div>
              <div v-if="o.error_message" class="note">{{ o.error_message }}</div>
            </td>
            <td>
              <span class="status" :class="o.status">{{ orderStatusLabel(o.status) }}</span>
            </td>
            <td class="time-cell" :title="formatTradeTime(orderTimestamp(o))">{{ formatTradeTime(orderTimestamp(o)) }}</td>
          </tr>
          <tr v-if="tableWindow.afterHeight > 0" class="lo-virtual-spacer-row">
            <td
              :colspan="ORDER_TABLE_COLUMN_COUNT"
              class="lo-virtual-spacer-cell"
              :style="{ height: `${tableWindow.afterHeight}px` }"
            ></td>
          </tr>
        </tbody>
      </table>
      <div v-else class="empty-state">
        <strong>当前范围暂无策略订单</strong>
        <span>策略生成入场、平仓、挂单或风控拦截动作后，都会按最新动作时间记录在这里。</span>
        <em>如果策略已经运行但仍为空，请先查看决策页是否返回动作，或确认上方范围是否切到了当前 run。</em>
      </div>
    </div>
    <div v-else-if="displayOrders.length === 0" class="empty-state">
      <strong>当前范围暂无策略订单</strong>
      <span>策略生成入场、平仓、挂单或风控拦截动作后，订单分布会显示在这里。</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { LiveOrder } from '@/types'
import { formatPrice } from '@/utils/format'
import { compareOrdersByLatest, orderTimestamp } from '@/utils/liveStrategyCore'
import { liveActionLabel } from '@/utils/liveOrderActions'
import {
  formatLiveDateTime as formatTradeTime,
  formatLiveQuantity as formatSize,
} from '@/utils/liveStrategyDisplay/format'
import {
  isContractLiveOrder,
  isFailedLiveOrder,
  isLiveOrderBlocked,
  isLiveOrderExit,
  isLivePositionEntryAction,
  isLiveRiskOrder,
  liveOrderEntryActionLabel,
  liveOrderExitActionLabel,
  liveOrderSpotSideLabel,
  orderStatusLabel,
  orderStatusText,
} from '@/utils/liveStrategyDisplay/orders'

const ORDER_ROW_ESTIMATED_HEIGHT = 34
const ORDER_OVERSCAN_ROWS = 8
const ORDER_DEFAULT_VIEWPORT_HEIGHT = 360
const ORDER_VIRTUALIZE_THRESHOLD = 80
const ORDER_TABLE_COLUMN_COUNT = 8

const props = withDefaults(defineProps<{
  orders: LiveOrder[]
  showCharts?: boolean
  showTable?: boolean
}>(), {
  showCharts: true,
  showTable: true,
})

const displayOrders = computed(() => [...props.orders].sort(compareOrdersByLatest))
const orderViewport = ref<HTMLElement | null>(null)
const orderScrollTop = ref(0)
const orderViewportHeight = ref(ORDER_DEFAULT_VIEWPORT_HEIGHT)

const tableWindow = computed(() => {
  const total = displayOrders.value.length
  if (total === 0 || total <= ORDER_VIRTUALIZE_THRESHOLD) {
    return { start: 0, end: total, beforeHeight: 0, afterHeight: 0 }
  }
  const rowHeight = ORDER_ROW_ESTIMATED_HEIGHT
  const visibleRows = Math.max(1, Math.ceil(orderViewportHeight.value / rowHeight))
  const firstVisible = Math.floor(orderScrollTop.value / rowHeight)
  const start = Math.max(0, firstVisible - ORDER_OVERSCAN_ROWS)
  const end = Math.min(total, firstVisible + visibleRows + ORDER_OVERSCAN_ROWS)
  return {
    start,
    end,
    beforeHeight: start * rowHeight,
    afterHeight: Math.max(0, (total - end) * rowHeight),
  }
})
const visibleTableOrders = computed(() => (
  displayOrders.value.slice(tableWindow.value.start, tableWindow.value.end)
))

const orderSummary = computed(() => displayOrders.value.reduce((summary, order) => {
  if (isLiveOrderBlocked(order)) summary.blocked += 1
  else if (isLiveOrderExit(order)) summary.exits += 1
  else if (isEntryOrder(order)) summary.entries += 1
  if (isFailedLiveOrder(order)) summary.failed += 1
  return summary
}, { entries: 0, exits: 0, blocked: 0, failed: 0 }))
const orderMixSegments = computed(() => {
  const other = Math.max(
    0,
    displayOrders.value.length - orderSummary.value.entries - orderSummary.value.exits - orderSummary.value.blocked,
  )
  const segments = [
    { key: 'entries', label: '开仓', count: orderSummary.value.entries },
    { key: 'exits', label: '平仓', count: orderSummary.value.exits },
    { key: 'blocked', label: '拦截', count: orderSummary.value.blocked },
    { key: 'other', label: '其他', count: other },
  ]
  const total = Math.max(displayOrders.value.length, 1)
  return segments
    .filter(segment => segment.count > 0)
    .map(segment => ({
      ...segment,
      width: `${Math.max(3, (segment.count / total) * 100)}%`,
    }))
})
const orderMixLabel = computed(() =>
  `订单动作分布，开仓 ${orderSummary.value.entries}，平仓 ${orderSummary.value.exits}，拦截 ${orderSummary.value.blocked}，失败 ${orderSummary.value.failed}`
)
const orderTimelineBars = computed(() =>
  displayOrders.value.slice(0, 24).reverse().map((order, index, rows) => ({
    key: `${orderKey(order)}-${index}`,
    kind: timelineKind(order),
    height: `${Math.max(30, Math.round(((index + 1) / Math.max(rows.length, 1)) * 100))}%`,
    title: `${formatTradeTime(orderTimestamp(order))} · ${orderActionLabel(order)} · ${orderStatusLabel(order.status)}`,
  }))
)

function syncOrderViewport() {
  const viewport = orderViewport.value
  if (!viewport) return
  orderScrollTop.value = Math.max(0, viewport.scrollTop)
  orderViewportHeight.value = viewport.clientHeight || ORDER_DEFAULT_VIEWPORT_HEIGHT
}

function clampOrderScroll() {
  const viewport = orderViewport.value
  if (!viewport) return
  const maxScrollTop = Math.max(
    0,
    displayOrders.value.length * ORDER_ROW_ESTIMATED_HEIGHT - orderViewportHeight.value,
  )
  if (viewport.scrollTop > maxScrollTop) {
    viewport.scrollTop = maxScrollTop
  }
  syncOrderViewport()
}

function orderActionLabel(order: LiveOrder): string {
  if (isLiveOrderBlocked(order)) return isLiveRiskOrder(order) ? '风控拦截' : '监控'
  if (isLiveOrderExit(order)) return liveOrderExitActionLabel(order)
  if (isEntryOrder(order)) return liveOrderEntryActionLabel(order)
  const actionLabel = liveActionLabel(order.action)
  if (actionLabel !== '--') return actionLabel
  if (isContractLiveOrder(order)) return order.side === 'sell' ? '做空' : order.side === 'buy' ? '做多' : '--'
  return liveOrderSpotSideLabel(order.side)
}

function orderActionClass(order: LiveOrder): string {
  if (isLiveOrderBlocked(order)) return isLiveRiskOrder(order) ? 'risk' : 'blocked'
  if (isLiveOrderExit(order)) return 'exit'
  if (order.side === 'sell') return 'short'
  if (order.side === 'buy') return 'long'
  return 'neutral'
}

function timelineKind(order: LiveOrder): string {
  if (isFailedLiveOrder(order)) return 'failed'
  if (isLiveOrderBlocked(order)) return 'blocked'
  if (isLiveOrderExit(order)) return 'exits'
  if (isEntryOrder(order)) return 'entries'
  return 'other'
}

function isEntryOrder(order: LiveOrder): boolean {
  const status = orderStatusText(order)
  return isLivePositionEntryAction(order.action)
    || status.includes('filled')
    || status === 'live'
    || status === 'open'
    || status === 'pending'
    || status === 'submitted'
    || status === 'submit_unknown'
    || status === 'submitting'
}

function orderKey(order: LiveOrder): string {
  return order.ord_id || `local-${order.id}`
}

function orderLabel(order: LiveOrder): string {
  if (order.ord_id) return order.ord_id.slice(0, 12)
  return `#${order.id}`
}

function orderDisplayPrice(order: LiveOrder): number | null {
  return order.avg_fill_price ?? order.px ?? positiveReferencePrice(order)
}

function orderPriceNote(order: LiveOrder): string {
  if (order.avg_fill_price !== null || order.px !== null) return ''
  if (positiveReferencePrice(order) === null) return ''
  return order.reference_price_missing
    ? '参考价 entry fallback'
    : referenceSourceLabel(order.reference_price_source)
}

function positiveReferencePrice(order: LiveOrder): number | null {
  const price = order.reference_price
  return typeof price === 'number' && Number.isFinite(price) && price > 0 ? price : null
}

function referenceSourceLabel(source: string): string {
  const normalized = source.trim()
  if (normalized === 'historical_last_close') return '参考价 历史收盘'
  if (normalized === 'strategy_action_price') return '参考价 策略动作'
  if (!normalized) return '参考价'
  return `参考价 ${normalized}`
}

function orderDisplaySize(order: LiveOrder): string {
  if (typeof order.filled_size === 'number' && Number.isFinite(order.filled_size)) {
    const submitted = formatSize(order.sz)
    const filled = formatSize(order.filled_size)
    return submitted === '--' ? filled : `${filled} / ${submitted}`
  }
  return formatSize(order.sz)
}

onMounted(() => {
  void nextTick(syncOrderViewport)
  window.addEventListener('resize', syncOrderViewport)
})

onBeforeUnmount(() => {
  window.removeEventListener('resize', syncOrderViewport)
})

watch(() => displayOrders.value.length, () => {
  void nextTick(clampOrderScroll)
})
</script>

<style scoped>
.lo-table {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
}
.lo-header {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border);
}
.lo-title { font-size: 13px; font-weight: 600; }
.lo-summary {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  color: var(--color-text-tertiary);
  font-size: 11px;
}
.lo-summary span {
  padding: 2px 6px;
  border: 1px solid rgba(148,163,184,0.2);
  border-radius: 999px;
  background: rgba(148,163,184,0.06);
}
.lo-charts {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(220px, 0.72fr);
  gap: 8px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--color-border);
}
.lo-chart-card {
  min-width: 0;
  padding: 8px;
  border: 1px solid rgba(148,163,184,0.18);
  border-radius: 6px;
  background: rgba(148,163,184,0.045);
}
.lo-chart-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 7px;
  font-size: 11px;
}
.lo-chart-head span {
  color: var(--color-text-tertiary);
}
.lo-chart-head strong {
  color: var(--color-text-secondary);
  font-variant-numeric: tabular-nums;
}
.lo-mix-track {
  display: flex;
  height: 12px;
  overflow: hidden;
  border-radius: 999px;
  background: rgba(148,163,184,0.12);
}
.lo-mix-track i,
.lo-mix-legend i,
.lo-timeline i {
  background: var(--color-text-secondary);
}
.lo-mix-track i + i {
  border-left: 1px solid rgba(15, 17, 23, 0.8);
}
.lo-mix-legend {
  display: flex;
  flex-wrap: wrap;
  gap: 7px;
  margin-top: 8px;
  color: var(--color-text-tertiary);
  font-size: 10px;
}
.lo-mix-legend span {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}
.lo-mix-legend i {
  width: 8px;
  height: 8px;
  border-radius: 2px;
}
.lo-timeline {
  display: flex;
  align-items: end;
  gap: 3px;
  height: 48px;
  padding: 4px 0 0;
}
.lo-timeline i {
  flex: 1 1 0;
  min-width: 3px;
  border-radius: 3px 3px 0 0;
  opacity: 0.9;
}
.lo-mix-track .entries,
.lo-mix-legend .entries,
.lo-timeline .entries {
  background: rgba(255, 183, 77, 0.86);
}
.lo-mix-track .exits,
.lo-mix-legend .exits,
.lo-timeline .exits {
  background: rgba(38,166,154,0.86);
}
.lo-mix-track .blocked,
.lo-mix-legend .blocked,
.lo-timeline .blocked {
  background: rgba(246,200,93,0.86);
}
.lo-mix-track .failed,
.lo-mix-legend .failed,
.lo-timeline .failed {
  background: rgba(239,83,80,0.9);
}
.lo-mix-track .other,
.lo-mix-legend .other,
.lo-timeline .other {
  background: rgba(148,163,184,0.58);
}
.lo-wrap {
  max-height: 360px;
  overflow: auto;
}
table {
  width: 100%;
  min-width: 920px;
  border-collapse: collapse;
  font-size: 12px;
}
th {
  position: sticky;
  top: 0;
  z-index: 1;
  background: var(--color-bg-secondary);
  text-align: left;
  padding: 6px 8px;
  color: var(--color-text-tertiary);
  font-weight: 500;
  font-size: 11px;
  white-space: nowrap;
}
th.num { text-align: right; }
td { padding: 4px 8px; border-top: 1px solid var(--color-border); white-space: nowrap; }
td.num { text-align: right; font-variant-numeric: tabular-nums; }
.lo-virtual-spacer-cell {
  height: 0;
  padding: 0;
  border-top: 0;
}
.id-cell { font-family: monospace; font-size: 11px; }
.action-cell {
  max-width: 320px;
  white-space: normal;
}
.note {
  margin-top: 1px;
  color: var(--color-text-tertiary);
  font-size: 10px;
  line-height: 1.35;
}
.action-badge {
  display: inline-block;
  min-width: 38px;
  padding: 2px 6px;
  border-radius: 3px;
  font-size: 10px;
  font-weight: 500;
  text-align: center;
}
.action-badge.long { background: rgba(38,166,154,0.15); color: var(--color-positive); }
.action-badge.short { background: rgba(255,152,0,0.16); color: #ffb74d; }
.action-badge.exit { background: rgba(148,163,184,0.16); color: var(--color-text-secondary); }
.action-badge.risk { background: rgba(239,83,80,0.15); color: var(--color-negative); }
.action-badge.blocked { background: rgba(246,200,93,0.14); color: #f6c85d; }
.action-badge.neutral { background: rgba(148,163,184,0.14); color: var(--color-text-secondary); }
.status.filled { color: var(--color-positive); }
.status.live,
.status.blocked { color: var(--color-accent); }
.status.risk_blocked { color: var(--color-negative); }
.time-cell { font-size: 11px; color: var(--color-text-secondary); }
.empty-state {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 24px;
  text-align: center;
}
.empty-state strong {
  color: var(--color-text-primary);
  font-size: 13px;
  font-weight: 700;
}
.empty-state span {
  color: var(--color-text-secondary);
  font-size: 12px;
  line-height: 1.45;
}
.empty-state em {
  color: var(--color-text-tertiary);
  font-size: 11px;
  font-style: normal;
  line-height: 1.45;
}
@media (max-width: 980px) {
  .lo-charts {
    grid-template-columns: 1fr;
  }
}
</style>
