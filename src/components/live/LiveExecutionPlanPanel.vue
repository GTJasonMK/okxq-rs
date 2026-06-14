<template>
  <section class="lep-panel">
    <div class="lep-header">
      <div>
        <span class="lep-title">退出计划</span>
        <span class="lep-subtitle">等待 {{ waitingCount }} · 处理中 {{ processingCount }} · 已提交 {{ submittedCount }} · 完成 {{ finishedCount }}</span>
      </div>
      <span class="lep-source">OKX 持仓平仓</span>
    </div>

    <div class="lep-wrap">
      <table v-if="sortedPlans.length > 0">
        <thead>
          <tr>
            <th>计划时间</th>
            <th>品种</th>
            <th>方向</th>
            <th>状态</th>
            <th class="num">尝试</th>
            <th>下次处理</th>
            <th>入场单</th>
            <th>平仓单</th>
            <th>错误</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="plan in sortedPlans" :key="plan.id || plan.plan_key">
            <td class="time-cell" :title="formatDateTime(plan.planned_exit_time)">
              {{ formatDateTime(plan.planned_exit_time) }}
            </td>
            <td class="symbol-cell">{{ plan.inst_id || plan.symbol || '--' }}</td>
            <td>
              <span class="side-badge" :class="sideClass(plan.close_side)">
                {{ closeSideLabel(plan.close_side) }}
              </span>
            </td>
            <td>
              <span class="status-badge" :class="statusClass(plan)">
                {{ statusLabel(plan) }}
              </span>
            </td>
            <td class="num">{{ plan.attempt_count }}</td>
            <td class="time-cell" :title="nextProcessTitle(plan)">
              {{ nextProcessText(plan) }}
            </td>
            <td class="id-cell" :title="plan.entry_order_id || plan.entry_client_order_id">
              {{ shortOrderId(plan.entry_order_id || plan.entry_client_order_id) }}
            </td>
            <td class="id-cell" :title="plan.exit_order_id || plan.exit_client_order_id">
              {{ shortOrderId(plan.exit_order_id || plan.exit_client_order_id) }}
            </td>
            <td class="error-cell" :title="plan.last_error">{{ plan.last_error || '--' }}</td>
          </tr>
        </tbody>
      </table>
      <div v-else class="empty-state">
        <strong>当前范围暂无退出计划</strong>
        <span>开仓成交后，策略返回的计划退出时间会在这里显示。</span>
        <em>到期后引擎会按 OKX 当前持仓提交平仓单。</em>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { LiveExecutionPlan } from '@/types'
import { formatLiveDateTime as formatDateTime } from '@/utils/liveStrategyDisplay/format'

const props = defineProps<{
  plans: LiveExecutionPlan[]
}>()

const sortedPlans = computed(() =>
  [...props.plans].sort((left, right) =>
    timestampValue(right.planned_exit_time) - timestampValue(left.planned_exit_time)
      || right.id - left.id
  )
)
const waitingCount = computed(() =>
  props.plans.filter(plan => plan.status === 'scheduled').length
)
const processingCount = computed(() =>
  props.plans.filter(plan => plan.status === 'exit_processing').length
)
const submittedCount = computed(() =>
  props.plans.filter(plan => plan.status === 'exit_submitted').length
)
const finishedCount = computed(() =>
  props.plans.filter(plan =>
    plan.status === 'exit_filled'
      || plan.status === 'skipped_no_position'
      || plan.status === 'skipped_invalid_plan'
  ).length
)

function statusLabel(plan: LiveExecutionPlan): string {
  const status = plan.status.trim()
  if (status === 'scheduled') {
    const now = Date.now()
    const retryAt = timestampValue(plan.next_attempt_at)
    const exitAt = timestampValue(plan.planned_exit_time)
    if (retryAt > now) return '等待重试'
    if (exitAt > 0 && exitAt <= now) return '待处理'
    return '等待退出'
  }
  if (status === 'exit_processing') return '处理中'
  if (status === 'exit_submitted') return '平仓单已提交'
  if (status === 'exit_filled') return '已完成退出'
  if (status === 'skipped_no_position') return '无可平仓位'
  if (status === 'skipped_invalid_plan') return '计划无效'
  if (status === 'exit_canceled' || status === 'canceled' || status === 'cancelled') return '平仓单已撤销'
  if (status === 'exit_rejected' || status === 'rejected') return '平仓单被拒绝'
  return status || '--'
}

function statusClass(plan: LiveExecutionPlan): string {
  const status = plan.status.trim()
  if (status === 'exit_filled' || status === 'skipped_no_position') return 'done'
  if (status === 'exit_submitted' || status === 'exit_processing') return 'pending'
  if (status === 'scheduled') {
    const now = Date.now()
    const retryAt = timestampValue(plan.next_attempt_at)
    const exitAt = timestampValue(plan.planned_exit_time)
    return retryAt > now || (exitAt > 0 && exitAt <= now) ? 'warn' : 'pending'
  }
  if (status === 'skipped_invalid_plan' || status.includes('reject') || status.includes('cancel')) return 'bad'
  return 'neutral'
}

function closeSideLabel(side: string): string {
  const normalized = side.trim().toLowerCase()
  if (normalized === 'sell') return '卖出平多'
  if (normalized === 'buy') return '买入平空'
  return side || '--'
}

function sideClass(side: string): string {
  const normalized = side.trim().toLowerCase()
  if (normalized === 'sell') return 'sell'
  if (normalized === 'buy') return 'buy'
  return 'flat'
}

function nextProcessText(plan: LiveExecutionPlan): string {
  const retryAt = timestampValue(plan.next_attempt_at)
  const exitAt = timestampValue(plan.planned_exit_time)
  if (plan.status === 'scheduled' && retryAt > 0 && retryAt > exitAt) {
    return formatDateTime(retryAt)
  }
  if (plan.status === 'scheduled') return formatDateTime(exitAt)
  if (plan.status === 'exit_processing') return '正在处理'
  if (plan.updated_at > 0) return formatDateTime(plan.updated_at)
  return '--'
}

function nextProcessTitle(plan: LiveExecutionPlan): string {
  const retryAt = timestampValue(plan.next_attempt_at)
  if (plan.status === 'scheduled' && retryAt > 0) {
    return `next_attempt_at ${formatDateTime(retryAt)}`
  }
  return `planned_exit_time ${formatDateTime(plan.planned_exit_time)}`
}

function shortOrderId(value: string): string {
  const trimmed = value.trim()
  if (!trimmed) return '--'
  if (trimmed.length <= 14) return trimmed
  return `${trimmed.slice(0, 6)}...${trimmed.slice(-6)}`
}

function timestampValue(value: number | null): number {
  return typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : 0
}
</script>

<style scoped>
.lep-panel {
  min-height: 100%;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: var(--color-bg-secondary);
  overflow: hidden;
}
.lep-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--color-border);
}
.lep-title {
  display: block;
  color: var(--color-text-primary);
  font-size: 13px;
  font-weight: 700;
}
.lep-subtitle {
  display: block;
  margin-top: 2px;
  color: var(--color-text-tertiary);
  font-size: 11px;
}
.lep-source {
  flex: 0 0 auto;
  color: var(--color-text-tertiary);
  font-size: 11px;
}
.lep-wrap {
  max-height: 100%;
  overflow: auto;
  overscroll-behavior: contain;
}
table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
}
th,
td {
  padding: 8px 10px;
  border-bottom: 1px solid rgba(148, 163, 184, 0.12);
  text-align: left;
  vertical-align: middle;
  white-space: nowrap;
}
th {
  position: sticky;
  top: 0;
  z-index: 1;
  background: var(--color-bg-secondary);
  color: var(--color-text-tertiary);
  font-size: 11px;
  font-weight: 600;
}
.num {
  text-align: right;
}
.symbol-cell {
  color: var(--color-text-primary);
  font-weight: 600;
}
.time-cell,
.id-cell {
  color: var(--color-text-secondary);
  font-variant-numeric: tabular-nums;
}
.error-cell {
  max-width: 260px;
  overflow: hidden;
  color: var(--color-text-tertiary);
  text-overflow: ellipsis;
}
.side-badge,
.status-badge {
  display: inline-flex;
  align-items: center;
  min-height: 22px;
  padding: 2px 8px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 700;
  white-space: nowrap;
}
.side-badge.sell {
  background: rgba(239, 83, 80, 0.12);
  color: var(--color-negative);
}
.side-badge.buy {
  background: rgba(38, 166, 154, 0.12);
  color: var(--color-positive);
}
.side-badge.flat,
.status-badge.neutral {
  background: rgba(148, 163, 184, 0.12);
  color: var(--color-text-tertiary);
}
.status-badge.pending {
  background: rgba(41, 98, 255, 0.14);
  color: #82a4ff;
}
.status-badge.warn {
  background: rgba(255, 183, 77, 0.14);
  color: var(--color-warning);
}
.status-badge.done {
  background: rgba(38, 166, 154, 0.14);
  color: var(--color-positive);
}
.status-badge.bad {
  background: rgba(239, 83, 80, 0.14);
  color: var(--color-negative);
}
.empty-state {
  display: grid;
  gap: 6px;
  padding: 24px 16px;
  color: var(--color-text-secondary);
  text-align: center;
}
.empty-state strong {
  color: var(--color-text-primary);
  font-size: 13px;
}
.empty-state span,
.empty-state em {
  color: var(--color-text-tertiary);
  font-size: 12px;
  font-style: normal;
}
</style>
