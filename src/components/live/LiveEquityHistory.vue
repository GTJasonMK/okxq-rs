<template>
  <div class="le-panel">
    <div class="le-header">
      <span class="le-title">{{ panelTitle }}</span>
      <span class="le-run" :title="`${runLabel} · ${latestSnapshotText}`">
        {{ runLabel }} · {{ latestSnapshotText }}
      </span>
    </div>

    <div class="le-section">
      <div class="le-section-title">按天汇总</div>
      <div class="le-wrap">
        <table v-if="dailyRows.length > 0">
          <thead>
            <tr>
              <th>日期</th>
              <th class="num">收盘权益</th>
              <th class="num">今日</th>
              <th class="num">总收益</th>
              <th class="num">快照</th>
              <th>区间</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="row in dailyRows" :key="row.trading_day">
              <td>{{ row.trading_day || '--' }}</td>
              <td class="num">{{ formatMoney(row.last_equity) }}</td>
              <td class="num pnl-cell" :class="pnlDisplayClass(rowPnlAvailable(row), row.today_pnl)">
                <template v-if="rowPnlAvailable(row)">
                  <span class="pnl-main">{{ formatPnl(row.today_pnl) }}</span>
                  <span class="pct">{{ formatPctPoint(row.today_pnl_pct) }}</span>
                </template>
                <span v-else class="pnl-main">--</span>
              </td>
              <td class="num pnl-cell" :class="pnlDisplayClass(rowPnlAvailable(row), row.total_pnl)">
                <template v-if="rowPnlAvailable(row)">
                  <span class="pnl-main">{{ formatPnl(row.total_pnl) }}</span>
                  <span class="pct">{{ formatPctPoint(row.total_pnl_pct) }}</span>
                </template>
                <span v-else class="pnl-main">--</span>
              </td>
              <td class="num">{{ row.snapshot_count }}</td>
              <td class="time-cell">{{ formatTime(row.start_timestamp) }} - {{ formatTime(row.end_timestamp) }}</td>
            </tr>
          </tbody>
        </table>
        <div v-else class="empty-state">
          <strong>暂无按天权益汇总</strong>
          <span>策略运行并同步 OKX 账户权益后会显示按天汇总。</span>
          <em>当前请以交易所账户、持仓、成交和账单同步结果为准。</em>
        </div>
      </div>
    </div>

    <div class="le-section">
      <div class="le-section-title">最近快照</div>
      <div class="le-wrap">
        <table v-if="snapshotRows.length > 0">
          <thead v-if="isAccountEquityHistory">
            <tr>
              <th>时间</th>
              <th class="num">账户权益</th>
              <th class="num">未实现</th>
              <th class="num">今日</th>
            </tr>
          </thead>
          <thead v-else>
            <tr>
              <th>时间</th>
              <th class="num">价格</th>
              <th>持仓</th>
              <th class="num">权益</th>
              <th class="num">未实现</th>
              <th class="num">今日</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="row in snapshotRows"
              :key="`${row.run_id}-${equitySnapshotTimestamp(row)}-${row.id}`"
            >
              <td class="time-cell">{{ formatDateTime(equitySnapshotTimestamp(row)) }}</td>
              <template v-if="!isAccountEquityHistory">
                <td class="num">{{ snapshotPriceText(row.price) }}</td>
                <td>
                  <span class="side-badge" :class="sideClass(row.position_side)">{{ sideLabel(row.position_side) }}</span>
                </td>
              </template>
              <td class="num">{{ formatMoney(row.equity) }}</td>
              <td class="num" :class="pnlClass(row.unrealized_pnl)">{{ formatPnl(row.unrealized_pnl) }}</td>
              <td class="num" :class="pnlDisplayClass(rowPnlAvailable(row), row.today_pnl)">
                {{ pnlText(rowPnlAvailable(row), row.today_pnl) }}
              </td>
            </tr>
          </tbody>
        </table>
        <div v-else class="empty-state">
          <strong>暂无最近权益快照</strong>
          <span>策略运行并同步 OKX 账户权益后会显示最近快照。</span>
          <em>成交价、手续费、资金费和持仓盈亏均以交易所回报为准。</em>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { LiveEquityHistory, TradingMode } from '@/types'
import { formatMoney, formatPrice } from '@/utils/format'
import {
  compareEquitySnapshotsByTime,
  equitySnapshotTimestamp,
  finiteNumber,
  isPortfolioPositionSide,
  isValidEquitySnapshot,
} from '@/utils/liveStrategyCore'
import { livePnlClass as pnlClass } from '@/utils/liveStrategyDisplay/format'

const props = defineProps<{
  history: LiveEquityHistory | null
  mode?: TradingMode
}>()

const sortedSnapshots = computed(() =>
  [...(props.history?.snapshots ?? [])]
    .filter(isValidEquitySnapshot)
    .sort(compareEquitySnapshotsByTime)
)
const sortedDaily = computed(() => [...(props.history?.daily ?? [])].sort(compareDailyByLatest))

const latest = computed(() => {
  const rows = sortedSnapshots.value
  return rows.length > 0 ? rows[rows.length - 1] : null
})
const isAccountEquityHistory = computed(() =>
  isAccountEquitySource(props.history?.source)
    || sortedSnapshots.value.some(row => isAccountEquitySource(row.source))
)
const pnlHistoryAvailable = computed(() => props.history?.pnl_available !== false)

const panelMode = computed(() => props.history?.mode ?? props.mode ?? 'simulated')
const panelModeText = computed(() => panelMode.value === 'live' ? '实盘' : '模拟盘')

const panelTitle = computed(() =>
  isAccountEquityHistory.value
    ? `OKX账户权益（${panelModeText.value}）`
    : panelMode.value === 'live' ? '策略权益（实盘模式）' : '策略权益（模拟模式）'
)

const runLabel = computed(() => {
  if (isAccountEquityHistory.value) return `OKX账户 · ${panelModeText.value}`
  const runId = props.history?.run_id || ''
  if (!runId) return '未记录'
  return `${runId.slice(0, 18)} · ${panelMode.value === 'live' ? '实盘模式' : '模拟模式'}`
})

const latestSnapshotText = computed(() =>
  latest.value ? `最新 ${formatDateTime(equitySnapshotTimestamp(latest.value))}` : '暂无快照'
)

const dailyRows = computed(() => sortedDaily.value.slice(0, 8))
const snapshotRows = computed(() => sortedSnapshots.value.slice(-10).reverse())

function compareDailyByLatest(left: LiveEquityHistory['daily'][number], right: LiveEquityHistory['daily'][number]): number {
  return right.end_timestamp - left.end_timestamp
    || right.trading_day.localeCompare(left.trading_day)
}

function formatPnl(value: number | null): string {
  const parsed = finiteNumber(value)
  if (parsed === null) return '--'
  const sign = parsed > 0 ? '+' : ''
  return `${sign}${parsed.toFixed(2)}`
}

function formatPctPoint(value: number | null): string {
  const parsed = finiteNumber(value)
  if (parsed === null) return '--'
  const sign = parsed > 0 ? '+' : ''
  return `${sign}${parsed.toFixed(2)}%`
}

function rowPnlAvailable(row: { pnl_available?: boolean }): boolean {
  return pnlHistoryAvailable.value && row.pnl_available !== false
}

function pnlText(available: boolean, value: number | null): string {
  return available ? formatPnl(value) : '--'
}

function pnlDisplayClass(available: boolean, value: number | null): string {
  return available ? pnlClass(value) : 'flat'
}

function snapshotPriceText(value: number | null): string {
  const parsed = finiteNumber(value)
  return parsed !== null && parsed > 0 ? formatPrice(parsed) : '--'
}

function sideLabel(side: string): string {
  if (side === 'long') return '多'
  if (side === 'short') return '空'
  if (isPortfolioPositionSide(side)) return '组合'
  return '空仓'
}

function sideClass(side: string): string {
  if (side === 'long') return 'long'
  if (side === 'short') return 'short'
  if (isPortfolioPositionSide(side)) return 'portfolio'
  return 'flat'
}

function isAccountEquitySource(source: unknown): boolean {
  return typeof source === 'string' && source.trim() === 'okx_account_balance'
}

function formatTime(ts: number): string {
  if (!Number.isFinite(ts) || ts <= 0) return '--'
  return new Date(ts).toLocaleTimeString('zh-CN', {
    timeZone: 'Asia/Shanghai',
    hour12: false,
  })
}

function formatDateTime(ts: number): string {
  if (!Number.isFinite(ts) || ts <= 0) return '--'
  return new Date(ts).toLocaleString('zh-CN', {
    timeZone: 'Asia/Shanghai',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  })
}
</script>

<style scoped>
.le-panel {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
}
.le-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border);
}
.le-title { font-size: 13px; font-weight: 600; }
.le-run {
  min-width: 0;
  color: var(--color-text-tertiary);
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.le-section { border-top: 1px solid var(--color-border); }
.le-section:first-of-type { border-top: none; }
.le-section-title {
  padding: 7px 12px;
  color: var(--color-text-secondary);
  font-size: 12px;
  font-weight: 600;
}
.le-wrap {
  max-height: 260px;
  overflow: auto;
}
table {
  width: 100%;
  min-width: 760px;
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
td {
  padding: 5px 8px;
  border-top: 1px solid var(--color-border);
  white-space: nowrap;
}
th.num,
td.num {
  text-align: right;
  font-variant-numeric: tabular-nums;
}
.pct {
  display: block;
  margin-top: 1px;
  color: var(--color-text-tertiary);
  font-size: 10px;
  line-height: 1.25;
}
.pnl-cell {
  min-width: 88px;
}
.pnl-main {
  display: block;
  line-height: 1.35;
}
.time-cell { color: var(--color-text-secondary); font-size: 11px; }
.positive { color: var(--color-positive); }
.negative { color: var(--color-negative); }
.flat { color: var(--color-text-secondary); }
.side-badge {
  display: inline-block;
  min-width: 30px;
  padding: 1px 5px;
  border-radius: 3px;
  text-align: center;
  font-size: 10px;
  font-weight: 500;
}
.side-badge.long { background: rgba(38,166,154,0.15); color: var(--color-positive); }
.side-badge.short { background: rgba(239,83,80,0.15); color: var(--color-negative); }
.side-badge.portfolio { background: rgba(148,163,184,0.14); color: var(--color-text-secondary); }
.side-badge.flat { background: rgba(148,163,184,0.14); color: var(--color-text-secondary); }
.empty-state {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 20px;
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
</style>
