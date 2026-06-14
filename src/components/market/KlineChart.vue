<template>
  <div class="kline-chart-wrap">
    <div ref="containerRef" class="kline-chart"></div>
    <div class="kline-bottom-gutter" aria-hidden="true"></div>
    <div v-if="legend" class="kline-legend">
      <span class="legend-symbol">{{ legend.symbol }}</span>
      <span>{{ legend.time }}</span>
      <span>O {{ legend.open }}</span>
      <span>H {{ legend.high }}</span>
      <span>L {{ legend.low }}</span>
      <span :class="legend.positive ? 'positive' : 'negative'">C {{ legend.close }}</span>
      <span :class="legend.positive ? 'positive' : 'negative'">{{ legend.changePct }}</span>
      <span>Vol {{ legend.volume }}</span>
      <span v-if="legend.ma5" class="ma5">MA5 {{ legend.ma5 }}</span>
      <span v-if="legend.ma10" class="ma10">MA10 {{ legend.ma10 }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { Candle, Timeframe } from '@/types'
import type { CandleRangeDays } from '@/types/marketView'
import type { StrategyTriggerMarker, StrategyTriggerMarkerMode } from '@/types/strategy-visualization'
import { useKlineChart } from '@/composables/useKlineChart'

const props = defineProps<{
  candles: Candle[]
  timeframe: Timeframe
  rangeDays: CandleRangeDays
  markers?: StrategyTriggerMarker[]
  markerMode?: StrategyTriggerMarkerMode
}>()
const { containerRef, legend } = useKlineChart(props)
</script>

<style scoped>
.kline-chart-wrap {
  position: relative;
  isolation: isolate;
  display: grid;
  grid-template-rows: minmax(0, 1fr) var(--kline-bottom-gutter, 0px);
  box-sizing: border-box;
  width: 100%;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  contain: layout paint;
}
.kline-chart {
  grid-row: 1;
  position: relative;
  z-index: 1;
  align-self: stretch;
  display: block;
  width: 100%;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  contain: layout paint;
}
.kline-bottom-gutter {
  grid-row: 2;
  position: relative;
  z-index: 0;
  min-height: 0;
  pointer-events: none;
}
.kline-chart :deep(.tv-lightweight-charts) {
  max-width: 100%;
  max-height: 100%;
  overflow: hidden !important;
  contain: paint;
}
.kline-legend {
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
.legend-symbol {
  color: var(--color-text-primary);
  font-weight: 600;
}
.positive { color: var(--color-positive); }
.negative { color: var(--color-negative); }
.ma5 { color: #f6c85d; }
.ma10 { color: #6be6c1; }
</style>
