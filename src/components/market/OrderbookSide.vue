<template>
  <div :class="['ob-side', sideClass]">
    <div
      v-for="(row, index) in rows"
      :key="side + index"
      class="ob-row"
      :class="sideClass"
    >
      <span class="ob-price" :class="priceClass">{{ formatPrice(row.price) }}</span>
      <span class="ob-size">{{ formatVolume(row.size) }}</span>
      <div class="ob-depth-bar" :class="barClass" :style="{ width: row.depthPct + '%' }"></div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { formatPrice, formatVolume } from '@/utils/format'

type OrderbookSide = 'ask' | 'bid'

const props = defineProps<{
  side: OrderbookSide
  rows: Array<{ price: number; size: number; depthPct: number }>
}>()

const sideClass = computed(() => props.side)
const priceClass = computed(() => `${props.side}-color`)
const barClass = computed(() => `${props.side}-bar`)
</script>

<style scoped>
.ob-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  padding: 2px 10px;
  position: relative;
  line-height: 1.6;
}
.ob-row.ask:hover { background: rgba(239, 83, 80, 0.06); }
.ob-row.bid:hover { background: rgba(38, 166, 154, 0.06); }
.ob-price { font-weight: 500; z-index: 1; position: relative; }
.ask-color { color: var(--color-negative); }
.bid-color { color: var(--color-positive); }
.ob-size { color: var(--color-text-secondary); text-align: right; z-index: 1; position: relative; }
.ob-depth-bar {
  position: absolute;
  right: 0;
  top: 0;
  height: 100%;
  opacity: 0.12;
  z-index: 0;
}
.ask-bar { background: var(--color-negative); }
.bid-bar { background: var(--color-positive); }
</style>
