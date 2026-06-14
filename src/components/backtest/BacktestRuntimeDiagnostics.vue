<template>
  <div v-if="summary" class="rc-diagnostics">
    <div class="diagnostic-head">
      <span class="diagnostic-title">执行诊断</span>
      <span v-if="plannedExitContractLabel" class="diagnostic-status" :class="plannedExitContractClass">
        {{ plannedExitContractLabel }}
      </span>
    </div>
    <div class="diagnostic-grid">
      <div v-for="item in diagnosticItems" :key="item.label" class="diagnostic-item">
        <span class="diagnostic-label">{{ item.label }}</span>
        <span class="diagnostic-value" :class="item.tone">{{ item.value }}</span>
      </div>
    </div>
    <div v-if="warningMessages.length > 0" class="diagnostic-warnings">
      <span v-for="warning in warningMessages" :key="warning" class="diagnostic-warning">{{ warning }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import {
  formatCount,
  plannedExitContractLabels,
  type RuntimeSummary,
  warningLabels,
} from '@/utils/backtestResultCard'

const props = defineProps<{
  summary: RuntimeSummary | null
}>()

const plannedExitContract = computed(() => summaryString('planned_exit_contract'))

const plannedExitContractLabel = computed(() => {
  const contract = plannedExitContract.value
  if (!contract) return ''
  return plannedExitContractLabels[contract] ?? contract
})

const plannedExitContractClass = computed(() => {
  switch (plannedExitContract.value) {
    case 'planned_exit_complete':
    case 'no_open_actions':
      return 'status-ok'
    case 'planned_exit_partial':
      return 'status-warn'
    case 'planned_exit_missing':
      return 'status-danger'
    default:
      return 'status-neutral'
  }
})

const diagnosticItems = computed(() => [
  {
    label: '计划退出',
    value: plannedExitCoverage(),
    tone: 'neutral',
  },
  {
    label: '计划平仓',
    value: formatCount(summaryNumber('planned_close_count')),
    tone: 'neutral',
  },
  {
    label: '止盈止损',
    value: formatCount(summaryNumber('risk_close_count')),
    tone: 'neutral',
  },
  {
    label: '缺标记价',
    value: formatCount(summaryNumber('open_positions_missing_mark_count')),
    tone: summaryNumber('open_positions_missing_mark_count') > 0 ? 'danger' : 'neutral',
  },
])

const warningMessages = computed(() => {
  const warnings = props.summary?.warnings
  if (!Array.isArray(warnings)) return []
  return warnings
    .filter((warning): warning is string => typeof warning === 'string' && warning.trim().length > 0)
    .map(warning => warningLabels[warning] ?? warning)
})

function summaryNumber(key: string): number {
  const value = props.summary?.[key]
  return typeof value === 'number' && Number.isFinite(value) ? value : 0
}

function summaryString(key: string): string {
  const value = props.summary?.[key]
  return typeof value === 'string' ? value.trim() : ''
}

function plannedExitCoverage(): string {
  const withPlannedExit = summaryNumber('open_actions_with_planned_exit')
  const openActions = summaryNumber('open_action_count')
  if (openActions > 0) {
    return `${formatCount(withPlannedExit)}/${formatCount(openActions)}`
  }
  const coverage = summaryNumber('planned_exit_coverage_pct')
  return coverage > 0 ? `${coverage.toFixed(1)}%` : '0/0'
}
</script>

<style scoped>
.rc-diagnostics {
  padding: 9px 12px 10px;
  border-top: 1px solid var(--color-border);
}
.diagnostic-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  min-width: 0;
  margin-bottom: 8px;
}
.diagnostic-title {
  color: var(--color-text-secondary);
  font-size: 12px;
  font-weight: 600;
}
.diagnostic-status {
  flex: 0 1 auto;
  max-width: 100%;
  padding: 2px 7px;
  overflow-wrap: anywhere;
  border: 1px solid var(--color-border);
  border-radius: 999px;
  font-size: 11px;
  line-height: 1.2;
}
.diagnostic-status.status-ok { color: var(--color-positive); }
.diagnostic-status.status-warn { color: #b7791f; }
.diagnostic-status.status-danger { color: var(--color-negative); }
.diagnostic-status.status-neutral { color: var(--color-text-tertiary); }
.diagnostic-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(92px, 1fr));
  gap: 6px 12px;
}
.diagnostic-item {
  min-width: 0;
}
.diagnostic-label {
  display: block;
  margin-bottom: 2px;
  color: var(--color-text-tertiary);
  font-size: 11px;
  line-height: 1.2;
}
.diagnostic-value {
  display: block;
  overflow-wrap: anywhere;
  color: var(--color-text-primary);
  font-size: 13px;
  font-variant-numeric: tabular-nums;
  font-weight: 600;
  line-height: 1.2;
}
.diagnostic-value.warn { color: #b7791f; }
.diagnostic-value.danger { color: var(--color-negative); }
.diagnostic-warnings {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
  margin-top: 8px;
}
.diagnostic-warning {
  max-width: 100%;
  padding: 2px 6px;
  overflow-wrap: anywhere;
  border: 1px solid rgba(229, 62, 62, 0.28);
  border-radius: 4px;
  color: var(--color-negative);
  font-size: 11px;
  line-height: 1.25;
}
</style>
