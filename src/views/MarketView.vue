<template>
  <div class="view-market">
    <MarketSelector
      :symbol="store.activeSymbol"
      :timeframe="store.activeTimeframe"
      :range-days="candleRangeDays"
      :watched-symbols="store.watchedSymbols"
      @update:symbol="handleSymbolUpdate"
      @update:timeframe="store.setActiveTimeframe"
      @update:range-days="setCandleRangeDays"
    />
    <MarketControls
      :active-market-type="activeMarketType"
      :sync-spot="!!activeWatchedSymbol?.sync_spot"
      :sync-swap="!!activeWatchedSymbol?.sync_swap"
      :has-watched-symbol="!!activeWatchedSymbol"
      :repairing="repairing"
      @update:active-market-type="activeMarketType = $event"
      @repair-active="repairActive"
      @open-data-center="openDataCenter"
    />
    <div v-if="store.error" class="vm-error">{{ store.error }}</div>
    <div v-if="statusMessage" class="vm-notice">{{ statusMessage }}</div>
    <MarketSyncProgress :progress="repairProgress" />
    <TickerBar :ticker="displayTicker" />
    <div class="market-grid">
      <div class="chart-area">
        <KlineChart
          :candles="displayCandles"
          :timeframe="store.activeTimeframe"
          :range-days="candleRangeDays"
        />
        <div v-if="chartEmptyMessage" class="chart-empty">
          <div>{{ chartEmptyMessage }}</div>
          <button class="vm-action" @click="openDataCenter">管理关注与同步</button>
        </div>
      </div>
      <div class="side-panels">
        <OrderbookPanel :orderbook="displayOrderbook" />
        <RecentTrades
          :trades="displayTrades"
          :orderbook="depthOrderbook"
          @request-depth="handleDepthRequest"
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useMarketViewState } from '@/composables/useMarketViewState'
import MarketSelector from '@/components/market/MarketSelector.vue'
import MarketControls from '@/components/market/MarketControls.vue'
import MarketSyncProgress from '@/components/market/MarketSyncProgress.vue'
import TickerBar from '@/components/market/TickerBar.vue'
import KlineChart from '@/components/market/KlineChart.vue'
import OrderbookPanel from '@/components/market/OrderbookPanel.vue'
import RecentTrades from '@/components/market/RecentTrades.vue'

defineOptions({ name: 'MarketView' })

const {
  store,
  activeMarketType,
  repairing,
  repairProgress,
  candleRangeDays,
  activeWatchedSymbol,
  displayTicker,
  displayCandles,
  displayOrderbook,
  depthOrderbook,
  displayTrades,
  statusMessage,
  chartEmptyMessage,
  handleSymbolUpdate,
  setCandleRangeDays,
  handleDepthRequest,
  repairActive,
  openDataCenter,
} = useMarketViewState()
</script>

<style scoped>
.view-market {
  display: flex;
  flex-direction: column;
  height: 100%;
  gap: 8px;
}
.vm-action {
  min-height: 30px;
  padding: 0 10px;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  background: var(--color-bg-secondary);
  color: var(--color-text-secondary);
  cursor: pointer;
}
.vm-action:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}
.vm-action:hover:not(:disabled) {
  border-color: var(--color-accent);
  color: var(--color-text-primary);
}
.market-grid {
  flex: 1;
  display: grid;
  grid-template-columns: 1fr 340px;
  gap: 8px;
  min-height: 0;
}
.chart-area {
  position: relative;
  min-height: 0;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
  background: var(--color-bg-secondary);
}
.vm-error {
  padding: 8px 10px;
  border: 1px solid rgba(239,83,80,0.35);
  border-radius: 6px;
  background: rgba(239,83,80,0.08);
  color: var(--color-negative);
  font-size: 12px;
}
.vm-notice {
  padding: 8px 10px;
  border: 1px solid rgba(41, 98, 255, 0.28);
  border-radius: 6px;
  background: rgba(41, 98, 255, 0.08);
  color: var(--color-text-secondary);
  font-size: 12px;
}
.chart-empty {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  background: rgba(15, 17, 23, 0.78);
  color: var(--color-text-secondary);
  text-align: center;
  pointer-events: auto;
}
.side-panels {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-height: 0;
  overflow: hidden;
}
.side-panels > *:first-child { flex: 1; min-height: 0; overflow: hidden; }
.side-panels > *:last-child { flex: 1; min-height: 0; overflow: hidden; }
</style>
