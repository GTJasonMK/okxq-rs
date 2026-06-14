<template>
  <section class="vl-chart-panel">
    <div class="vl-chart-content">
      <EquityCandleChart
        v-if="candles.length > 0"
        title="实时账户余额"
        :candles="candles"
        :snapshots="snapshots"
        :timeframe="timeframe"
        :trades="trades"
      />
      <div v-else class="vl-chart-empty">
        暂无账户权益快照，连接 OKX 私有账户后会自动刷新
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import EquityCandleChart from '@/components/strategy/EquityCandleChart.vue'
import type {
  BacktestEquitySnapshot,
  BacktestTrade,
  Timeframe,
} from '@/types'
import type { EquityCandle } from '@/utils/strategyExecution'

defineProps<{
  candles: EquityCandle[]
  snapshots: BacktestEquitySnapshot[]
  timeframe: Timeframe
  trades: BacktestTrade[]
}>()
</script>

<style scoped>
.vl-chart-panel {
  position: relative;
  z-index: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}
.vl-chart-content {
  position: relative;
  flex: 1 1 auto;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}
.vl-chart-content :deep(.equity-candle-chart) {
  height: 100%;
}
.vl-chart-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  min-height: 280px;
  padding: 16px;
  color: var(--color-text-tertiary);
  font-size: 12px;
  text-align: center;
}

@media (max-width: 1100px) {
  .vl-chart-panel {
    flex: 0 0 auto;
    min-height: 320px;
  }
}

@media (max-height: 840px) and (min-width: 1101px) {
  .vl-chart-empty {
    min-height: 280px;
  }
}
</style>
