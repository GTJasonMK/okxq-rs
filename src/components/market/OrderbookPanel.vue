<template>
  <div class="orderbook-panel">
    <div class="ob-header">
      <span>价格</span>
      <span>数量</span>
    </div>
    <OrderbookSide v-if="orderbook" side="ask" :rows="askRows" />
    <div class="ob-spread" v-if="spread !== null">
      <span class="spread-text">{{ formatPrice(spread) }} ({{ spreadPct }}%)</span>
    </div>
    <OrderbookSide v-if="orderbook" side="bid" :rows="bidRows" />
    <div class="ob-empty" v-if="!orderbook">等待订单簿数据...</div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import OrderbookSide from '@/components/market/OrderbookSide.vue'
import type { Orderbook } from '@/types'
import { formatPrice } from '@/utils/format'
import { orderbookDisplaySide } from '@/utils/marketView'

const MAX_ROWS = 12

const props = defineProps<{ orderbook: Orderbook | null }>()

const askSide = computed(() =>
  orderbookDisplaySide(props.orderbook?.asks ?? [], 'ask', MAX_ROWS)
)

const bidSide = computed(() =>
  orderbookDisplaySide(props.orderbook?.bids ?? [], 'bid', MAX_ROWS)
)

const spread = computed(() => {
  const bestBid = bidSide.value.best
  const bestAsk = askSide.value.best
  if (!bestBid || !bestAsk) return null
  return bestAsk.price - bestBid.price
})

const spreadPct = computed(() => {
  const bestBid = bidSide.value.best
  const bestAsk = askSide.value.best
  if (spread.value === null || !bestBid || !bestAsk) return '0.00'
  const mid = (bestAsk.price + bestBid.price) / 2
  if (mid === 0) return '0.00'
  return ((spread.value / mid) * 100).toFixed(4)
})

const askRows = computed(() => askSide.value.rows)
const bidRows = computed(() => bidSide.value.rows)

</script>

<style scoped>
.orderbook-panel {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
  font-size: 12px;
}
.ob-header {
  display: grid;
  grid-template-columns: 1fr 1fr;
  padding: 6px 10px;
  color: var(--color-text-tertiary);
  font-size: 11px;
  border-bottom: 1px solid var(--color-border);
}
.ob-spread {
  padding: 4px 10px;
  text-align: center;
  border-top: 1px solid var(--color-border);
  border-bottom: 1px solid var(--color-border);
  background: rgba(0,0,0,0.2);
}
.spread-text { font-size: 11px; color: var(--color-text-secondary); }
.ob-empty {
  padding: 20px;
  text-align: center;
  color: var(--color-text-tertiary);
}
</style>
