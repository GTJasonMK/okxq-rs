<template>
  <div class="ecc-metric-switch" role="group" aria-label="底部指标">
    <button
      v-for="option in metricOptions"
      :key="option.id"
      type="button"
      class="ecc-metric-option"
      :class="{ active: activeHistogramMetric === option.id }"
      @click="emit('select-histogram-metric', option.id)"
    >
      {{ option.label }}
    </button>
  </div>
  <div v-if="legend" class="ecc-legend">
    <span class="ecc-symbol">{{ title }}</span>
    <span>{{ legend.time }}</span>
    <span>O {{ legend.open }}</span>
    <span>H {{ legend.high }}</span>
    <span>L {{ legend.low }}</span>
    <span :class="legend.positive ? 'positive' : 'negative'">C {{ legend.close }}</span>
    <span :class="legend.positive ? 'positive' : 'negative'">{{ legend.change }}</span>
    <span>{{ legend.count }} 点</span>
  </div>
  <div v-if="rangeSummary" class="ecc-range">
    <span>最高 {{ rangeSummary.max }}</span>
    <span>最低 {{ rangeSummary.min }}</span>
    <span>区间 {{ rangeSummary.range }}</span>
    <span>最大回撤 {{ rangeSummary.drawdown }}</span>
  </div>
</template>

<script setup lang="ts">
import {
  EQUITY_HISTOGRAM_METRIC_OPTIONS,
  type EquityHistogramMetric,
  type Legend,
  type RangeSummary,
} from '@/utils/equityCandleChart'

defineProps<{
  activeHistogramMetric: EquityHistogramMetric
  legend: Legend | null
  rangeSummary: RangeSummary | null
  title?: string
}>()

const emit = defineEmits<{
  (event: 'select-histogram-metric', metric: EquityHistogramMetric): void
}>()

const metricOptions = EQUITY_HISTOGRAM_METRIC_OPTIONS
</script>

<style scoped>
.ecc-metric-switch {
  position: absolute;
  top: 8px;
  right: 8px;
  z-index: 3;
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 2px;
  max-width: min(420px, calc(100% - 16px));
  padding: 3px;
  border: 1px solid rgba(255,255,255,0.08);
  border-radius: 4px;
  background: rgba(3, 3, 4, 0.74);
  backdrop-filter: blur(4px);
}
.ecc-metric-option {
  border: 0;
  border-radius: 3px;
  background: transparent;
  color: var(--color-text-tertiary);
  cursor: pointer;
  font-size: 11px;
  line-height: 1.2;
  padding: 4px 7px;
}
.ecc-metric-option:hover {
  color: var(--color-text-primary);
}
.ecc-metric-option.active {
  background: rgba(38,166,154,0.16);
  color: var(--color-positive);
}
.ecc-legend {
  position: absolute;
  top: 8px;
  left: 8px;
  z-index: 2;
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  max-width: calc(100% - 16px);
  padding: 5px 8px;
  border: 1px solid rgba(255,255,255,0.08);
  border-radius: 4px;
  background: rgba(3, 3, 4, 0.72);
  color: var(--color-text-secondary);
  font-size: 11px;
  line-height: 1.3;
  pointer-events: none;
  backdrop-filter: blur(4px);
}
@media (min-width: 761px) {
  .ecc-legend {
    max-width: calc(100% - 454px);
  }
}
.ecc-symbol {
  color: var(--color-text-primary);
  font-weight: 600;
}
.ecc-range {
  position: absolute;
  right: 8px;
  bottom: 8px;
  z-index: 2;
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 8px;
  max-width: calc(100% - 16px);
  padding: 5px 8px;
  border: 1px solid rgba(255,255,255,0.08);
  border-radius: 4px;
  background: rgba(3, 3, 4, 0.72);
  color: var(--color-text-secondary);
  font-size: 11px;
  line-height: 1.3;
  pointer-events: none;
  backdrop-filter: blur(4px);
}
.positive { color: var(--color-positive); }
.negative { color: var(--color-negative); }

@media (max-width: 760px) {
  .ecc-metric-switch {
    top: 44px;
    left: 8px;
    right: auto;
    justify-content: flex-start;
  }
}
</style>
