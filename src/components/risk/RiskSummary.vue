<template>
  <div class="risk-summary">
    <div class="rs-cards">
      <div class="rs-card">
        <div class="rs-card-label">VaR (95%)</div>
        <div class="rs-card-value" :class="pnlColor(numericField(metrics, 'var_95'))">
          {{ formatMoney(numericField(metrics, 'var_95')) }}
        </div>
        <div class="rs-card-sub">1日 95% 置信度</div>
      </div>
      <div class="rs-card">
        <div class="rs-card-label">最大回撤</div>
        <div class="rs-card-value negative">
          {{ formatPercent(numericField(metrics, 'max_drawdown')) }}
        </div>
        <div class="rs-card-sub">历史最大回撤</div>
      </div>
      <div class="rs-card">
        <div class="rs-card-label">当前敞口</div>
        <div class="rs-card-value">{{ formatMoney(totalExposure(snapshot)) }}</div>
        <div class="rs-card-sub">总风险敞口</div>
      </div>
      <div class="rs-card">
        <div class="rs-card-label">Sharpe</div>
        <div class="rs-card-value">{{ formatDecimal(numericField(metrics, 'sharpe_ratio')) }}</div>
        <div class="rs-card-sub">年化夏普</div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { formatMoney, formatPercent } from '@/utils/format'
import { pnlColor } from '@/utils/color'

defineProps<{ metrics: unknown | null; snapshot: unknown | null }>()

function numericField(obj: unknown, key: string): number | null {
  const v = (obj as Record<string, unknown> | null)?.[key]
  return typeof v === 'number' && Number.isFinite(v) ? v : null
}

function totalExposure(snapshot: unknown): number | null {
  const spotValue = numericField(snapshot, 'spot_value')
  const contractValue = numericField(snapshot, 'contract_value')
  return spotValue !== null && contractValue !== null ? spotValue + contractValue : null
}

function formatDecimal(value: number | null): string {
  return value !== null ? value.toFixed(2) : '--'
}
</script>

<style scoped>
.risk-summary { margin-bottom: 4px; }
.rs-cards {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 8px;
}
.rs-card {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  padding: 12px 14px;
  text-align: center;
}
.rs-card-label { font-size: 11px; color: var(--color-text-tertiary); margin-bottom: 4px; }
.rs-card-value { font-size: 20px; font-weight: 700; font-variant-numeric: tabular-nums; }
.rs-card-sub { font-size: 10px; color: var(--color-text-tertiary); margin-top: 2px; }
.positive { color: var(--color-positive); }
.negative { color: var(--color-negative); }
</style>
