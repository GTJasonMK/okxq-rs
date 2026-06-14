<template>
  <div class="rt-trades">
    <div class="rt-header">
      <span>价格</span>
      <span>数量</span>
      <span>时间</span>
    </div>
    <div v-if="trades.length > 0" class="rt-list">
      <div
        v-for="(trade, index) in trades"
        :key="trade.trade_id || index"
        class="rt-row"
        :class="{ buy: trade.side === 'buy', sell: trade.side === 'sell' }"
      >
        <span class="rt-price" :class="trade.side === 'buy' ? 'bid-color' : 'ask-color'">
          {{ formatPrice(trade.price) }}
        </span>
        <span class="rt-size">{{ formatVolume(trade.size) }}</span>
        <span class="rt-time">{{ formatTradeTime(trade.ts) }}</span>
      </div>
    </div>
    <div v-else class="rt-empty">等待成交数据...</div>
  </div>
</template>

<script setup lang="ts">
import type { RecentTrade } from '@/types'
import { formatPrice, formatVolume } from '@/utils/format'

defineProps<{
  trades: RecentTrade[]
}>()

function formatTradeTime(ts: number): string {
  const value = new Date(ts)
  return value.toLocaleTimeString('zh-CN', { hour12: false })
}
</script>

<style scoped>
.rt-trades {
  display: flex;
  flex: 1;
  min-height: 0;
  flex-direction: column;
}

.rt-header {
  display: grid;
  grid-template-columns: 1fr 1fr 1fr;
  padding: 6px 10px;
  color: var(--color-text-tertiary);
  font-size: 11px;
  border-bottom: 1px solid var(--color-border);
}

.rt-list {
  flex: 1;
  overflow-y: auto;
}

.rt-row {
  display: grid;
  grid-template-columns: 1fr 1fr 1fr;
  padding: 2px 10px;
  line-height: 1.6;
}

.rt-row:hover {
  background: rgba(255, 255, 255, 0.03);
}

.rt-price {
  font-weight: 500;
}

.bid-color {
  color: var(--color-positive);
}

.ask-color {
  color: var(--color-negative);
}

.rt-size,
.rt-time {
  color: var(--color-text-secondary);
  text-align: right;
}

.rt-empty {
  padding: 20px;
  text-align: center;
  color: var(--color-text-tertiary);
}
</style>
