<template>
  <div
    v-if="progress"
    class="vb-run-progress"
    :class="{ failed: progress.status === 'failed', completed: progress.status === 'completed' }"
  >
    <div class="vb-run-progress-head">
      <span>{{ stageText }}</span>
      <strong>{{ percent }}%</strong>
    </div>
    <div class="vb-run-progress-track">
      <span :style="{ width: `${percent}%` }"></span>
    </div>
    <div class="vb-run-progress-meta">
      <span>{{ progress.message || '运行策略' }}</span>
      <span v-if="candleText">{{ candleText }}</span>
    </div>
    <div v-if="detailRows.length" class="vb-run-progress-details">
      <span
        v-for="row in detailRows"
        :key="row.key"
        class="vb-run-progress-detail"
      >
        <b>{{ row.label }}</b>
        <em>{{ row.value }}</em>
      </span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { BacktestProgress } from '@/types'

const props = defineProps<{
  progress: BacktestProgress | null
}>()

const percent = computed(() => Math.max(0, Math.min(100, Math.round(props.progress?.progress ?? 0))))
const stageText = computed(() => progressStageLabel(props.progress?.stage ?? props.progress?.status ?? ''))
const candleText = computed(() => {
  const progress = props.progress
  if (!progress || progress.total_candles <= 0) return ''
  return `${progress.processed_candles}/${progress.total_candles} K线`
})
const detailRows = computed(() => progressDetailRows(props.progress?.strategy_progress))

interface DetailRow {
  key: string
  label: string
  value: string
}

const detailFieldOrder = [
  'step',
  'instrument_rules_source',
  'symbol',
  'inst_type',
  'timeframe',
  'window_days',
  'expected_candles',
  'loaded_candles',
  'context_series_index',
  'context_series_count',
  'min_bars',
  'primary_context_candles',
  'first_evaluable_time',
  'last_window_time',
  'blocked_requirements',
  'evaluated_steps',
  'warmup_skipped',
  'context_skipped',
  'actions',
  'intents',
  'risk_actions',
  'skipped_actions',
  'submitted_intents_total',
  'risk_actions_total',
  'skipped_actions_total',
  'total_trades',
  'initial_capital',
  'position_size',
  'final_capital',
  'persist_result',
  'ctVal',
  'ctValCcy',
  'lotSz',
  'minSz',
  'tickSz',
  'current_timestamp',
  'window_start',
  'window_end',
] as const

const detailLabels: Record<string, string> = {
  step: '步骤',
  instrument_rules_source: '规格来源',
  symbol: '标的',
  inst_type: '市场',
  timeframe: '周期',
  window_days: '窗口天数',
  expected_candles: '预计K线',
  loaded_candles: '已加载K线',
  context_series_index: '当前序列',
  context_series_count: '上下文序列',
  min_bars: 'Warmup要求',
  primary_context_candles: '上下文K线',
  first_evaluable_time: '最早可评估',
  last_window_time: '窗口最后K线',
  blocked_requirements: '阻塞序列',
  evaluated_steps: '已评估',
  warmup_skipped: 'Warmup跳过',
  context_skipped: '上下文等待',
  actions: '模型动作',
  intents: '执行意图',
  risk_actions: '保护单动作',
  skipped_actions: '跳过动作',
  submitted_intents_total: '累计意图',
  risk_actions_total: '累计保护单动作',
  skipped_actions_total: '累计跳过',
  total_trades: '交易数',
  initial_capital: '初始资金',
  position_size: '仓位',
  final_capital: '最终资金',
  persist_result: '保存结果',
  ctVal: 'ctVal',
  ctValCcy: 'ctValCcy',
  lotSz: 'lotSz',
  minSz: 'minSz',
  tickSz: 'tickSz',
  current_timestamp: '当前时间',
  window_start: '开始时间',
  window_end: '结束时间',
}

const stepLabels: Record<string, string> = {
  parse_window: '解析窗口',
  window_ready: '窗口就绪',
  load_strategy_config: '加载策略配置',
  strategy_config_ready: '配置就绪',
  load_primary_candles: '加载主K线',
  primary_candles_ready: '主K线就绪',
  instrument_rules_prepare: '解析交易规格',
  instrument_rules_ready: '交易规格就绪',
  context_load_start: '加载上下文',
  context_series_load: '加载上下文序列',
  context_series_ready: '上下文序列就绪',
  context_load_ready: '上下文就绪',
  strategy_loop_start: '开始历史执行',
  warmup_wait: '等待Warmup',
  context_wait: '等待上下文',
  strategy_evaluate_start: '调用evaluate',
  strategy_evaluate_done: 'evaluate完成',
  simulation_done: '订单模拟完成',
  persist_result: '保存结果',
}

function progressStageLabel(stage: string) {
  const labels: Record<string, string> = {
    idle: '等待运行',
    prepare: '准备回测',
    config: '加载配置',
    candles: '加载K线',
    context: '构建上下文',
    context_wait: '等待上下文',
    instrument_rules: '交易规格',
    strategy_evaluate: '策略评估',
    warmup: '等待Warmup',
    warmup_blocked: 'Warmup不足',
    strategy: '执行策略',
    candidate_generation: '生成候选',
    candidate_context: '构建候选上下文',
    base_layer_generation: '基础候选层',
    universe_candidate_generation: '全市场候选',
    model_scoring: '模型评分',
    action_generation: '生成交易动作',
    candidate_selection: '候选选择',
    simulate: '生成结果',
    persist: '保存结果',
    complete: '回测完成',
    completed: '回测完成',
    failed: '回测失败',
  }
  return labels[stage] || stage || '运行策略'
}

function progressDetailRows(value: unknown): DetailRow[] {
  if (!isRecord(value)) return []
  const rows: DetailRow[] = []
  for (const key of detailFieldOrder) {
    if (!Object.prototype.hasOwnProperty.call(value, key)) continue
    const displayValue = formatDetailValue(key, value[key])
    if (!displayValue) continue
    rows.push({
      key,
      label: detailLabels[key] ?? key,
      value: displayValue,
    })
    if (rows.length >= 12) break
  }
  return rows
}

function formatDetailValue(key: string, value: unknown): string {
  if (value === null || value === undefined || value === '') return ''
  if (key === 'step' && typeof value === 'string') return stepLabels[value] ?? value
  if (key === 'instrument_rules_source' && typeof value === 'string') {
    return instrumentRulesSourceLabel(value)
  }
  if ((key === 'first_evaluable_time' || key === 'last_window_time') && typeof value === 'string') {
    return value
  }
  if (key.endsWith('_timestamp') || key === 'window_start' || key === 'window_end') {
    return formatTimestamp(value)
  }
  if (typeof value === 'boolean') return value ? '是' : '否'
  if (typeof value === 'number') return formatDetailNumber(key, value)
  if (Array.isArray(value)) return `${value.length}项`
  if (typeof value === 'object') return ''
  return String(value)
}

function formatDetailNumber(key: string, value: number): string {
  if (!Number.isFinite(value)) return ''
  if (key === 'position_size' && Math.abs(value) <= 1) return `${(value * 100).toFixed(2)}%`
  if (Number.isInteger(value)) return String(value)
  if (Math.abs(value) >= 1) return value.toFixed(4).replace(/\.?0+$/, '')
  return value.toPrecision(6)
}

function formatTimestamp(value: unknown): string {
  const timestamp = typeof value === 'number' ? value : Number(value)
  if (!Number.isFinite(timestamp) || timestamp <= 0) return ''
  return new Date(timestamp).toLocaleString('zh-CN', { hour12: false })
}

function instrumentRulesSourceLabel(value: string): string {
  const normalized = value.trim().toLowerCase()
  if (normalized === 'simulated') return '模拟规格'
  if (normalized === 'params') return '手动参数'
  if (normalized === 'okx') return 'OKX实时规格'
  return value
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value)
}
</script>

<style scoped>
.vb-run-progress {
  padding: 8px 10px;
  border: 1px solid rgba(41, 98, 255, 0.32);
  border-radius: 6px;
  background: rgba(41, 98, 255, 0.08);
}
.vb-run-progress.completed {
  border-color: rgba(38,166,154,0.35);
  background: rgba(38,166,154,0.08);
}
.vb-run-progress.failed {
  border-color: rgba(239,83,80,0.35);
  background: rgba(239,83,80,0.08);
}
.vb-run-progress-head,
.vb-run-progress-meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  min-width: 0;
  font-size: 12px;
}
.vb-run-progress-head span,
.vb-run-progress-meta span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.vb-run-progress-head span {
  color: var(--color-text-primary);
  font-weight: 600;
}
.vb-run-progress-head strong {
  color: var(--color-text-primary);
  font-variant-numeric: tabular-nums;
}
.vb-run-progress-track {
  height: 5px;
  margin: 7px 0 6px;
  overflow: hidden;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.08);
}
.vb-run-progress-track span {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: var(--color-accent);
  transition: width 0.18s ease;
}
.vb-run-progress.completed .vb-run-progress-track span {
  background: var(--color-positive);
}
.vb-run-progress.failed .vb-run-progress-track span {
  background: var(--color-negative);
}
.vb-run-progress-meta {
  color: var(--color-text-secondary);
  font-size: 11px;
}
.vb-run-progress-details {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(132px, 1fr));
  gap: 5px 8px;
  margin-top: 7px;
  padding-top: 7px;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
}
.vb-run-progress-detail {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
  min-width: 0;
  padding: 4px 6px;
  border-radius: 4px;
  background: rgba(255, 255, 255, 0.045);
  font-size: 10.5px;
  line-height: 1.25;
}
.vb-run-progress-detail b,
.vb-run-progress-detail em {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-style: normal;
}
.vb-run-progress-detail b {
  flex: 0 0 auto;
  color: var(--color-text-tertiary);
  font-weight: 500;
}
.vb-run-progress-detail em {
  color: var(--color-text-primary);
  font-variant-numeric: tabular-nums;
  text-align: right;
}
</style>
