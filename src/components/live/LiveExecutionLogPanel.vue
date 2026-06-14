<template>
  <section class="vl-execution-log-panel">
    <div class="vl-log-head">
      <div>
        <h3>执行日志</h3>
        <p>{{ subtitle }}</p>
      </div>
      <span v-if="logs.length" class="vl-log-count">{{ logs.length }}</span>
    </div>
    <div v-if="error" class="vl-log-error">{{ error }}</div>
    <div
      v-if="logs.length"
      ref="logViewport"
      class="vl-log-list"
      @scroll="syncLogViewport"
    >
      <div
        v-if="logWindow.beforeHeight > 0"
        class="vl-log-spacer"
        :style="{ height: `${logWindow.beforeHeight}px` }"
      ></div>
      <div
        v-for="entry in visibleLogs"
        :key="logEntryKey(entry)"
        class="vl-log-row"
        :class="`level-${entry.level || 'info'}`"
        :title="detailsTitle(entry)"
      >
        <time>{{ formatLogTime(entry) }}</time>
        <span class="vl-log-stage">{{ stageLabel(entry.stage) }}</span>
        <strong>{{ entry.message }}</strong>
        <span v-if="logProgressLabel(entry)" class="vl-log-progress">
          {{ logProgressLabel(entry) }}
        </span>
      </div>
      <div
        v-if="logWindow.afterHeight > 0"
        class="vl-log-spacer"
        :style="{ height: `${logWindow.afterHeight}px` }"
      ></div>
    </div>
    <div v-else class="vl-log-empty">暂无执行日志</div>
  </section>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { LiveExecutionLogEntry } from '@/types'

const LOG_ROW_ESTIMATED_HEIGHT = 33
const LOG_OVERSCAN_ROWS = 6
const LOG_DEFAULT_VIEWPORT_HEIGHT = 220

const props = defineProps<{
  logs: LiveExecutionLogEntry[]
  error?: string
}>()

const subtitle = computed(() => props.logs.length ? '最新阶段在上' : '等待策略启动或评估')
const logViewport = ref<HTMLElement | null>(null)
const logScrollTop = ref(0)
const logViewportHeight = ref(LOG_DEFAULT_VIEWPORT_HEIGHT)

const logWindow = computed(() => {
  const total = props.logs.length
  if (total === 0) {
    return { start: 0, end: 0, beforeHeight: 0, afterHeight: 0 }
  }
  const rowHeight = LOG_ROW_ESTIMATED_HEIGHT
  const visibleRows = Math.max(1, Math.ceil(logViewportHeight.value / rowHeight))
  const firstVisible = Math.floor(logScrollTop.value / rowHeight)
  const start = Math.max(0, firstVisible - LOG_OVERSCAN_ROWS)
  const end = Math.min(total, firstVisible + visibleRows + LOG_OVERSCAN_ROWS)
  return {
    start,
    end,
    beforeHeight: start * rowHeight,
    afterHeight: Math.max(0, (total - end) * rowHeight),
  }
})

const visibleLogs = computed(() => {
  const rows: LiveExecutionLogEntry[] = []
  for (let index = logWindow.value.start; index < logWindow.value.end; index += 1) {
    const entry = props.logs[props.logs.length - 1 - index]
    if (entry) rows.push(entry)
  }
  return rows
})

function syncLogViewport() {
  const viewport = logViewport.value
  if (!viewport) return
  logScrollTop.value = Math.max(0, viewport.scrollTop)
  logViewportHeight.value = viewport.clientHeight || LOG_DEFAULT_VIEWPORT_HEIGHT
}

function clampLogScroll() {
  const viewport = logViewport.value
  if (!viewport) return
  const maxScrollTop = Math.max(
    0,
    props.logs.length * LOG_ROW_ESTIMATED_HEIGHT - logViewportHeight.value,
  )
  if (viewport.scrollTop > maxScrollTop) {
    viewport.scrollTop = maxScrollTop
  }
  syncLogViewport()
}

function formatLogTime(entry: LiveExecutionLogEntry): string {
  const timestamp = entry.timestamp_ms || Date.parse(entry.time)
  if (!Number.isFinite(timestamp) || timestamp <= 0) return '--:--:--'
  const date = new Date(timestamp)
  const hours = String(date.getHours()).padStart(2, '0')
  const minutes = String(date.getMinutes()).padStart(2, '0')
  const seconds = String(date.getSeconds()).padStart(2, '0')
  return `${hours}:${minutes}:${seconds}`
}

function stageLabel(stage: string): string {
  const labels: Record<string, string> = {
    candles: 'K线',
    action_generation: '动作生成',
    candidate_generation: '候选生成',
    candidate_selection: '候选选择',
    context: '上下文',
    context_account: '账户',
    context_candles: '上下文K线',
    context_funding: '资金费率',
    context_orderbook: '盘口',
    context_orders: '订单上下文',
    context_positions: '持仓',
    decision: '决策',
    evaluate: '评估',
    intent: '动作',
    leverage: '杠杆',
    model_scoring: '模型评分',
    persist: '保存',
    planned_exit: '计划退出',
    planned_exit_worker: '退出调度',
    quantity: '数量',
    order_sync: '订单同步',
    fill_sync: '成交同步',
    risk: '风控',
    slippage: '滑点',
    start: '启动',
    stop: '停止',
    strategy: '策略',
    strategy_audit: '策略审计',
    strategy_call: '策略调用',
    strategy_decision: '策略决策',
    strategy_input: '策略输入',
    submit: '下单',
    subscribe: '订阅',
    trigger: '触发',
    unsubscribe: '退订',
  }
  return labels[stage] ?? (stage || '阶段')
}

function detailsTitle(entry: LiveExecutionLogEntry): string {
  const details = stringifyDetails(entry.details)
  return details ? `${entry.message}\n${details}` : entry.message
}

function logProgressLabel(entry: LiveExecutionLogEntry): string {
  const value = logProgressValue(entry)
  return value === null ? '' : `${Math.round(value)}%`
}

function logProgressValue(entry: LiveExecutionLogEntry): number | null {
  const raw = firstFiniteDetailNumber(
    entry.details,
    ['progress'],
    ['details', 'progress'],
    ['strategy_progress', 'progress'],
    ['details', 'strategy_progress', 'progress'],
  )
  if (raw === null) return null
  const percent = raw <= 1 ? raw * 100 : raw
  return Math.max(0, Math.min(100, percent))
}

function firstFiniteDetailNumber(details: Record<string, unknown>, ...paths: string[][]): number | null {
  for (const path of paths) {
    const value = detailPathValue(details, path)
    const numeric = typeof value === 'number'
      ? value
      : typeof value === 'string' && value.trim() !== ''
        ? Number(value)
        : Number.NaN
    if (Number.isFinite(numeric)) return numeric
  }
  return null
}

function detailPathValue(source: unknown, path: string[]): unknown {
  let current = source
  for (const key of path) {
    if (!current || typeof current !== 'object' || Array.isArray(current)) return undefined
    current = (current as Record<string, unknown>)[key]
  }
  return current
}

function logEntryKey(entry: LiveExecutionLogEntry): string {
  return `${entry.run_id || 'run'}:${entry.seq}:${entry.timestamp_ms}:${entry.stage}`
}

function stringifyDetails(details: Record<string, unknown>): string {
  if (!Object.keys(details).length) return ''
  try {
    return JSON.stringify(details, null, 2)
  } catch {
    return ''
  }
}

onMounted(() => {
  void nextTick(syncLogViewport)
  window.addEventListener('resize', syncLogViewport)
})

onBeforeUnmount(() => {
  window.removeEventListener('resize', syncLogViewport)
})

watch(() => props.logs.length, () => {
  void nextTick(clampLogScroll)
})
</script>

<style scoped>
.vl-execution-log-panel {
  display: flex;
  min-width: 0;
  min-height: 190px;
  max-height: 260px;
  flex-direction: column;
  gap: 8px;
  padding: 8px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: var(--color-bg-secondary);
}
.vl-log-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 8px;
  min-width: 0;
}
.vl-log-head h3 {
  margin: 0;
  color: var(--color-text-primary);
  font-size: 13px;
  font-weight: 700;
}
.vl-log-head p {
  margin: 2px 0 0;
  color: var(--color-text-tertiary);
  font-size: 11px;
}
.vl-log-count {
  flex: 0 0 auto;
  min-width: 26px;
  padding: 2px 7px;
  border-radius: 999px;
  background: rgba(41, 98, 255, 0.16);
  color: var(--color-text-primary);
  font-size: 11px;
  font-weight: 700;
  line-height: 1.4;
  text-align: center;
}
.vl-log-error {
  padding: 6px 8px;
  border: 1px solid rgba(239, 83, 80, 0.32);
  border-radius: 5px;
  background: rgba(239, 83, 80, 0.08);
  color: var(--color-negative);
  font-size: 11px;
}
.vl-log-list {
  display: flex;
  min-height: 0;
  flex: 1 1 auto;
  flex-direction: column;
  gap: 5px;
  overflow: auto;
  overscroll-behavior: contain;
}
.vl-log-row {
  display: grid;
  grid-template-columns: 56px 48px minmax(0, 1fr) 42px;
  align-items: center;
  gap: 6px;
  min-height: 28px;
  padding: 5px 7px;
  border-left: 3px solid rgba(148, 163, 184, 0.36);
  border-radius: 5px;
  background: rgba(148, 163, 184, 0.06);
  font-size: 11px;
}
.vl-log-row time {
  color: var(--color-text-tertiary);
  font-variant-numeric: tabular-nums;
}
.vl-log-stage {
  min-width: 0;
  overflow: hidden;
  color: var(--color-text-secondary);
  font-weight: 700;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.vl-log-row strong {
  min-width: 0;
  overflow: hidden;
  color: var(--color-text-primary);
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.vl-log-progress {
  justify-self: end;
  min-width: 36px;
  padding: 1px 5px;
  border-radius: 4px;
  background: rgba(41, 98, 255, 0.12);
  color: var(--color-text-secondary);
  font-size: 10px;
  font-variant-numeric: tabular-nums;
  font-weight: 700;
  line-height: 1.5;
  text-align: right;
}
.vl-log-row.level-success {
  border-left-color: rgba(38, 166, 154, 0.72);
}
.vl-log-row.level-warn {
  border-left-color: rgba(255, 183, 77, 0.8);
}
.vl-log-row.level-error {
  border-left-color: rgba(239, 83, 80, 0.82);
}
.vl-log-row.level-success .vl-log-stage {
  color: var(--color-positive);
}
.vl-log-row.level-warn .vl-log-stage {
  color: var(--color-warning);
}
.vl-log-row.level-error .vl-log-stage {
  color: var(--color-negative);
}
.vl-log-spacer {
  flex: 0 0 auto;
  min-height: 0;
}
.vl-log-empty {
  display: flex;
  flex: 1 1 auto;
  align-items: center;
  justify-content: center;
  min-height: 80px;
  border: 1px dashed rgba(148, 163, 184, 0.22);
  border-radius: 5px;
  color: var(--color-text-tertiary);
  font-size: 12px;
}
</style>
