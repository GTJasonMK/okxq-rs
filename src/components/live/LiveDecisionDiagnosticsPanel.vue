<template>
  <div class="vl-decision-card">
    <div class="vl-decision-head">
      <div>
        <div class="vl-card-header inline">当前策略决策</div>
        <div class="vl-decision-sub">
          <span v-if="diagnostics">
            {{ diagnostics.symbol }} · {{ diagnostics.timeframe }} · {{ diagnostics.candle_count ?? '--' }} 根K线
            · {{ diagnostics.realtime_candle_applied ? '实时K线' : '已收盘K线' }}
          </span>
          <span v-else>{{ scopeText }}</span>
        </div>
      </div>
      <button class="vl-mini-btn" :disabled="loading" @click="$emit('refresh')">
        {{ loading ? '评估中...' : '刷新' }}
      </button>
    </div>

    <div
      v-if="panelNotice"
      class="vl-decision-state"
      :class="panelNotice.kind"
    >
      <strong>{{ panelNotice.title }}</strong>
      <span>{{ panelNotice.detail }}</span>
      <em>{{ panelNotice.next }}</em>
    </div>

    <template v-if="diagnostics">
      <div class="vl-decision-summary">
        <div>
          <span>协议</span>
          <strong>{{ diagnostics.decision_protocol }}</strong>
        </div>
        <div>
          <span>动作数</span>
          <strong>{{ diagnostics.action_summary.total }}</strong>
        </div>
        <div>
          <span>覆盖币种</span>
          <strong>{{ diagnostics.selected_symbols.length || actionSymbols.length }}</strong>
        </div>
        <div>
          <span>执行状态</span>
          <strong>{{ verdictText }}</strong>
        </div>
      </div>

      <div class="vl-action-strip">
        <span v-for="item in summaryItems" :key="item.key" :class="{ active: item.count > 0 }">
          {{ item.label }} <strong>{{ item.count }}</strong>
        </span>
      </div>

      <div v-if="diagnostics.summary" class="vl-decision-message">
        {{ diagnostics.summary }}
      </div>

      <div v-if="diagnostics.execution_decision" class="vl-gates">
        <div class="vl-section-title">执行预览</div>
        <div
          v-if="diagnostics.execution_decision.skipped_action_count > 0"
          class="vl-skip-summary"
        >
          <strong>跳过 {{ diagnostics.execution_decision.skipped_action_count }} 个动作</strong>
          <span>
            执行意图 {{ diagnostics.execution_decision.executable_intent_count }} · 保护单动作 {{ diagnostics.execution_decision.risk_action_count }} · 等待 {{ diagnostics.execution_decision.idle_action_count }}
          </span>
        </div>
        <div
          v-for="gate in diagnostics.execution_decision.gates"
          :key="gate.key"
          class="vl-gate"
          :class="gate.status || (gate.blocking ? 'block' : 'pass')"
        >
          <span>{{ gate.label || gate.key }}</span>
          <strong>{{ gateStatusText(gate.status) }}</strong>
          <em>{{ gate.detail || '--' }}</em>
        </div>
        <div
          v-for="(action, index) in diagnostics.execution_decision.skipped_actions"
          :key="`skip-${action.action}-${action.symbol}-${index}`"
          class="vl-skipped-action"
        >
          <span>{{ actionText(action.action) }} · {{ action.symbol || '--' }}</span>
          <strong>{{ skippedReason(action) }}</strong>
        </div>
      </div>

      <div class="vl-section-title">策略动作</div>
      <div v-if="diagnostics.actions.length > 0" class="vl-action-table-wrap">
        <table class="vl-action-table">
          <thead>
            <tr>
              <th>动作</th>
              <th>币种</th>
              <th>方向</th>
              <th>订单类型</th>
              <th>价格</th>
              <th>数量</th>
              <th>触发价</th>
              <th>订单目标/变更</th>
              <th>计划退出</th>
              <th>来源</th>
              <th>原因</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(action, index) in diagnostics.actions" :key="`${action.action}-${action.symbol}-${index}`">
              <td>{{ actionText(action.action) }}</td>
              <td>{{ action.symbol || '--' }}</td>
              <td>{{ sideText(action.side) }}</td>
              <td>{{ action.order_type || '--' }}</td>
              <td>{{ priceText(action) }}</td>
              <td>{{ sizeText(action) }}</td>
              <td>{{ numberText(action.trigger_price) }}</td>
              <td class="management-cell" :title="managementTitle(action)">
                {{ managementText(action) }}
              </td>
              <td class="time-cell" :title="plannedExitTitle(action)">
                {{ shortDateTime(action.planned_exit_time) }}
              </td>
              <td class="source-cell" :title="sourceTitle(action)">
                {{ sourceText(action) }}
              </td>
              <td class="reason-cell" :title="action.reason">{{ action.reason || '--' }}</td>
            </tr>
          </tbody>
        </table>
      </div>
      <div v-else class="vl-empty-row">策略当前未返回动作。</div>

      <div v-if="diagnostics.execution_logs.length > 0" class="vl-logs">
        <div class="vl-section-title">策略内部日志</div>
        <div
          v-for="(log, index) in diagnostics.execution_logs"
          :key="`${log.stage}-${index}`"
          class="vl-log-row"
          :class="log.level"
        >
          <span>{{ log.stage }}</span>
          <strong>{{ log.message }}</strong>
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { LiveDecisionDiagnostics, LiveExecutionGate, LiveStrategyAction } from '@/types'

type PanelState = {
  kind: 'error' | 'loading' | 'empty'
  title: string
  detail: string
  next: string
}

const props = withDefaults(defineProps<{
  diagnostics: LiveDecisionDiagnostics | null
  scopeText: string
  loading: boolean
  refreshSource?: string
  error: string
  autoEnabled: boolean
  running: boolean
}>(), {
  refreshSource: '',
})

defineEmits<{
  refresh: []
}>()

const summaryItems = computed(() => {
  const summary = props.diagnostics?.action_summary
  return [
    { key: 'open_position', label: '开仓', count: summary?.open_position ?? 0 },
    { key: 'close_position', label: '平仓', count: summary?.close_position ?? 0 },
    { key: 'place_risk_order', label: '保护单', count: summary?.place_risk_order ?? 0 },
    { key: 'cancel_order', label: '撤单', count: summary?.cancel_order ?? 0 },
    { key: 'modify_order', label: '改单', count: summary?.modify_order ?? 0 },
    { key: 'hold', label: '等待', count: summary?.hold ?? 0 },
  ]
})

const actionSymbols = computed(() => {
  const symbols = props.diagnostics?.actions.map(action => action.symbol).filter(Boolean) ?? []
  return Array.from(new Set(symbols))
})

const verdictText = computed(() => {
  const verdict = props.diagnostics?.execution_decision?.verdict || ''
  const labels: Record<string, string> = {
    ready: '可执行',
    blocked: '已阻断',
    hold: '等待',
    preview: '预览',
    mismatch: '目标不匹配',
    mixed: '混合动作',
  }
  return labels[verdict] || '--'
})

const panelNotice = computed<PanelState | null>(() => {
  if (props.error && props.diagnostics) {
    return {
      kind: 'error',
      title: '刷新失败，已保留上一次决策',
      detail: props.error,
      next: '下方仍是上一次匹配结果，不是最新决策。',
    }
  }
  if (props.loading && props.refreshSource === 'manual' && props.diagnostics) {
    return {
      kind: 'loading',
      title: '正在刷新当前决策',
      detail: '下面显示的是上一次结果，避免刷新期间误判为空。',
      next: '刷新完成后会更新动作、执行预览和策略日志。',
    }
  }
  if (props.diagnostics) return null
  if (props.loading) {
    return {
      kind: 'loading',
      title: '正在评估当前策略决策',
      detail: props.scopeText,
      next: '结果会显示模型返回的 actions、执行预览和策略内部日志。',
    }
  }
  if (props.error) {
    return {
      kind: 'error',
      title: '决策诊断失败',
      detail: props.error,
      next: '请确认策略、品种、周期和 K 线数据是否匹配。',
    }
  }
  if (!props.autoEnabled) {
    return {
      kind: 'empty',
      title: props.running ? '未自动评估决策' : '策略未运行，未自动诊断',
      detail: props.scopeText,
      next: props.running
        ? '点击刷新后，页面会跟随当前查看层的已收盘 K 线自动更新。'
        : '点击刷新可手动评估当前配置；启动策略后仍需先手动刷新一次再跟随实时 K 线更新。',
    }
  }
  return {
    kind: 'empty',
    title: '暂无决策诊断数据',
    detail: props.scopeText,
    next: '可以点击刷新，或先确认左侧策略、品种、周期和右侧查看层。',
  }
})

function actionText(value: string) {
  const labels: Record<string, string> = {
    open_position: '开仓',
    close_position: '平仓',
    place_risk_order: '保护单',
    cancel_order: '撤单',
    modify_order: '改单',
    hold: '等待',
  }
  return labels[value] || value || '--'
}

function sideText(value: string) {
  const labels: Record<string, string> = {
    long: '多',
    buy: '买',
    short: '空',
    sell: '卖',
    flat: '平',
  }
  return labels[value] || value || '--'
}

function priceText(action: LiveStrategyAction) {
  if (action.price !== null) return numberText(action.price)
  if (action.reference_price !== null) return `${numberText(action.reference_price)} ref`
  return '--'
}

function sizeText(action: LiveStrategyAction) {
  if (action.exchange_size) return action.exchange_size
  return numberText(action.position_size)
}

function numberText(value: number | null) {
  if (typeof value !== 'number' || !Number.isFinite(value)) return '--'
  return Math.abs(value) >= 100 ? value.toFixed(2) : value.toFixed(6).replace(/0+$/, '').replace(/\.$/, '')
}

function shortDateTime(value: number | null) {
  const timestamp = timestampValue(value)
  if (timestamp <= 0) return '--'
  return new Date(timestamp).toLocaleString('zh-CN', {
    timeZone: 'Asia/Shanghai',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  })
}

function plannedExitTitle(action: LiveStrategyAction) {
  const parts = [
    action.planned_exit_time ? `计划时间 ${shortDateTime(action.planned_exit_time)}` : '',
    action.planned_exit_reason ? `原因 ${action.planned_exit_reason}` : '',
    action.planned_exit_contract ? `合同 ${action.planned_exit_contract}` : '',
    action.planned_hold_bars !== null ? `计划持有K线 ${action.planned_hold_bars}` : '',
    action.hold_bars !== null ? `持有K线 ${action.hold_bars}` : '',
  ].filter(Boolean)
  return parts.length > 0 ? parts.join(' · ') : '--'
}

function sourceText(action: LiveStrategyAction) {
  const head = action.candidate_source || action.layer_id || action.family || '--'
  const suffix = [
    action.action_timeframe,
    action.source_index !== null ? `#${action.source_index}` : '',
  ].filter(Boolean).join(' ')
  return suffix ? `${head} · ${suffix}` : head
}

function sourceTitle(action: LiveStrategyAction) {
  const parts = [
    action.candidate_source ? `候选来源 ${action.candidate_source}` : '',
    action.layer_id ? `层 ${action.layer_id}` : '',
    action.family ? `族 ${action.family}` : '',
    action.action_timeframe ? `周期 ${action.action_timeframe}` : '',
    action.source_index !== null ? `来源索引 ${action.source_index}` : '',
    action.source_time ? `来源时间 ${shortDateTime(action.source_time)}` : '',
    action.feature_bar_time ? `特征K线 ${shortDateTime(action.feature_bar_time)}` : '',
    action.entry_time ? `入场时间 ${shortDateTime(action.entry_time)}` : '',
    action.candidate_entry_price !== null ? `候选入场价 ${numberText(action.candidate_entry_price)}` : '',
  ].filter(Boolean)
  return parts.length > 0 ? parts.join(' · ') : '--'
}

function managementText(action: LiveStrategyAction) {
  if (action.action !== 'cancel_order' && action.action !== 'modify_order') {
    if (action.action === 'place_risk_order') {
      return targetOrderTypeText(action.target_order_type || action.order_type)
    }
    return '--'
  }

  const parts = [
    ...managementTargetParts(action),
    ...managementChangeParts(action),
  ]
  return parts.length > 0 ? parts.join(' · ') : '--'
}

function managementTitle(action: LiveStrategyAction) {
  if (action.action !== 'cancel_order' && action.action !== 'modify_order') {
    if (action.action === 'place_risk_order') {
      return targetOrderTypeText(action.target_order_type || action.order_type)
    }
    return '--'
  }

  const orderId = cleanText(action.order_id)
  const clientOrderId = cleanText(action.client_order_id)
  const requestId = cleanText(action.request_id)
  const parts = [
    action.target_order_kind ? `目标 ${targetKindText(action.target_order_kind)}` : '',
    action.target_order_type ? `类型 ${targetOrderTypeText(action.target_order_type)}` : '',
    orderId ? `订单ID ${orderId}` : '',
    clientOrderId ? `客户订单ID ${clientOrderId}` : '',
    action.new_size ? `新数量 ${action.new_size}` : '',
    action.new_price ? `新价格 ${action.new_price}` : '',
    requestId ? `请求ID ${requestId}` : '',
    action.cancel_on_fail ? '失败时撤单' : '',
  ].filter(Boolean)
  return parts.length > 0 ? parts.join(' · ') : '--'
}

function managementTargetParts(action: LiveStrategyAction) {
  const orderId = cleanText(action.order_id)
  const clientOrderId = cleanText(action.client_order_id)
  const parts = [
    action.target_order_kind ? targetKindText(action.target_order_kind) : '',
    action.target_order_type ? targetOrderTypeText(action.target_order_type) : '',
  ].filter(Boolean)

  if (orderId) {
    parts.push(`订单 ${shortId(orderId)}`)
  } else if (clientOrderId) {
    parts.push(`客户单 ${shortId(clientOrderId)}`)
  }

  return parts
}

function managementChangeParts(action: LiveStrategyAction) {
  if (action.action !== 'modify_order') return []
  return [
    action.new_size ? `新数量 ${action.new_size}` : '',
    action.new_price ? `新价 ${action.new_price}` : '',
    action.cancel_on_fail ? '失败撤单' : '',
  ].filter(Boolean)
}

function cleanText(value: string) {
  return typeof value === 'string' ? value.trim() : ''
}

function shortId(value: string) {
  const text = cleanText(value)
  if (text.length <= 18) return text
  return `${text.slice(0, 8)}...${text.slice(-6)}`
}

function targetKindText(value: string) {
  const labels: Record<string, string> = {
    exchange: '普通订单',
    algo: '保护单',
    any: '自动匹配',
  }
  const key = cleanText(value).toLowerCase()
  return labels[key] || cleanText(value) || '--'
}

function targetOrderTypeText(value: string) {
  const labels: Record<string, string> = {
    market: '市价',
    limit: '限价',
    post_only: 'Post Only',
    ioc: 'IOC',
    fok: 'FOK',
    optimal_limit_ioc: '最优限价 IOC',
    stop_market: '止损市价',
    stop_loss_market: '止损市价',
    stop_loss_limit: '止损限价',
    take_profit_market: '止盈市价',
    take_profit_limit: '止盈限价',
  }
  const key = cleanText(value).toLowerCase()
  return labels[key] || cleanText(value) || '--'
}

function timestampValue(value: number | null) {
  return typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : 0
}

function skippedReason(action: LiveStrategyAction) {
  const rawReason = action.raw._execution_skip_reason
  return typeof rawReason === 'string' && rawReason.trim()
    ? rawReason
    : action.reason || '--'
}

function gateStatusText(status: LiveExecutionGate['status']) {
  const labels: Record<string, string> = {
    pass: '通过',
    block: '阻断',
    wait: '等待',
    skip: '跳过',
    monitor: '监控',
    info: '信息',
  }
  return labels[status] || status || '--'
}
</script>

<style scoped>
.vl-decision-card {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
}
.vl-decision-head {
  padding: 8px 10px;
  border-bottom: 1px solid var(--color-border);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}
.vl-card-header.inline {
  padding: 0;
  border-bottom: none;
  font-size: 13px;
  font-weight: 600;
}
.vl-decision-sub {
  margin-top: 2px;
  color: var(--color-text-tertiary);
  font-size: 11px;
}
.vl-mini-btn {
  padding: 4px 8px;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  background: var(--color-bg-primary);
  color: var(--color-text-secondary);
  font-size: 12px;
  cursor: pointer;
  white-space: nowrap;
}
.vl-mini-btn:disabled { opacity: 0.5; cursor: not-allowed; }
.vl-decision-state {
  display: flex;
  flex-direction: column;
  gap: 5px;
  margin: 8px 10px;
  padding: 8px;
  border: 1px solid rgba(148,163,184,0.28);
  border-radius: 4px;
  background: rgba(148,163,184,0.06);
  color: var(--color-text-secondary);
  font-size: 12px;
  line-height: 1.45;
}
.vl-decision-state strong {
  color: var(--color-text-primary);
  font-size: 12px;
  font-weight: 700;
}
.vl-decision-state em {
  color: var(--color-text-tertiary);
  font-size: 11px;
  font-style: normal;
}
.vl-decision-state.loading {
  border-color: rgba(41,98,255,0.28);
  background: rgba(41,98,255,0.08);
}
.vl-decision-state.error {
  border-color: rgba(239,83,80,0.32);
  background: rgba(239,83,80,0.07);
}
.vl-decision-state.error strong { color: var(--color-negative); }
.vl-decision-state.empty { text-align: center; }
.vl-decision-summary {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 8px;
  padding: 10px;
}
.vl-decision-summary div {
  min-width: 0;
  padding: 8px;
  border: 1px solid rgba(148,163,184,0.18);
  border-radius: 4px;
  background: rgba(148,163,184,0.05);
}
.vl-decision-summary span,
.vl-action-strip span,
.vl-gate em,
.vl-log-row span {
  color: var(--color-text-tertiary);
  font-size: 11px;
}
.vl-decision-summary strong {
  display: block;
  margin-top: 3px;
  overflow: hidden;
  color: var(--color-text-primary);
  font-size: 13px;
  font-weight: 700;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.vl-action-strip {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  padding: 0 10px 10px;
}
.vl-action-strip span {
  padding: 4px 7px;
  border: 1px solid rgba(148,163,184,0.2);
  border-radius: 4px;
  background: rgba(148,163,184,0.05);
}
.vl-action-strip span.active {
  border-color: rgba(38,166,154,0.32);
  background: rgba(38,166,154,0.08);
  color: var(--color-text-primary);
}
.vl-decision-message,
.vl-empty-row {
  margin: 0 10px 10px;
  padding: 8px;
  border: 1px solid rgba(148,163,184,0.18);
  border-radius: 4px;
  color: var(--color-text-secondary);
  font-size: 12px;
}
.vl-section-title {
  padding: 8px 10px 6px;
  color: var(--color-text-primary);
  font-size: 12px;
  font-weight: 700;
}
.vl-gates {
  border-top: 1px solid rgba(148,163,184,0.12);
}
.vl-gate,
.vl-log-row,
.vl-skipped-action {
  display: grid;
  grid-template-columns: 120px 64px minmax(0, 1fr);
  gap: 8px;
  align-items: center;
  padding: 6px 10px;
  border-top: 1px solid rgba(148,163,184,0.1);
  font-size: 12px;
}
.vl-gate strong {
  color: var(--color-text-secondary);
  font-size: 12px;
}
.vl-skip-summary {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin: 0 10px 6px;
  padding: 7px 8px;
  border: 1px solid rgba(239,83,80,0.24);
  border-radius: 4px;
  background: rgba(239,83,80,0.06);
  color: var(--color-text-secondary);
  font-size: 12px;
}
.vl-skip-summary strong {
  color: var(--color-negative);
  font-size: 12px;
}
.vl-skip-summary span {
  color: var(--color-text-tertiary);
  font-size: 11px;
}
.vl-skipped-action {
  grid-template-columns: 180px minmax(0, 1fr);
}
.vl-skipped-action strong {
  overflow: hidden;
  color: var(--color-negative);
  font-size: 12px;
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.vl-gate.pass strong,
.vl-gate.success strong {
  color: var(--color-positive);
}
.vl-gate.block strong,
.vl-gate.error strong {
  color: var(--color-negative);
}
.vl-action-table-wrap {
  margin: 0 10px 10px;
  overflow: auto;
  border: 1px solid rgba(148,163,184,0.16);
  border-radius: 4px;
}
.vl-action-table {
  width: 100%;
  min-width: 1240px;
  border-collapse: collapse;
  font-size: 12px;
}
.vl-action-table th,
.vl-action-table td {
  padding: 7px 8px;
  border-bottom: 1px solid rgba(148,163,184,0.12);
  color: var(--color-text-secondary);
  text-align: left;
  white-space: nowrap;
}
.vl-action-table th {
  color: var(--color-text-tertiary);
  font-weight: 600;
}
.vl-action-table tr:last-child td {
  border-bottom: none;
}
.management-cell,
.reason-cell,
.source-cell,
.time-cell {
  max-width: 220px;
  overflow: hidden;
  text-overflow: ellipsis;
}
.reason-cell {
  max-width: 240px;
}
.vl-logs {
  border-top: 1px solid rgba(148,163,184,0.12);
}
.vl-log-row {
  grid-template-columns: 120px minmax(0, 1fr);
}
.vl-log-row strong {
  overflow: hidden;
  color: var(--color-text-secondary);
  font-size: 12px;
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.vl-log-row.error strong { color: var(--color-negative); }
.vl-log-row.warn strong { color: var(--color-warning); }
.vl-log-row.success strong { color: var(--color-positive); }
@media (max-width: 1100px) {
  .vl-decision-summary {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
  .vl-gate,
  .vl-log-row,
  .vl-skipped-action {
    grid-template-columns: minmax(0, 1fr);
    gap: 3px;
  }
}
@media (min-width: 1101px) {
  .vl-decision-card {
    max-height: min(720px, calc(100vh - 190px));
    overflow-y: auto;
    overscroll-behavior: contain;
  }
}
</style>
