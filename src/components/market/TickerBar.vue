<template>
  <div class="ticker-bar" v-if="ticker">
    <div class="ticker-price">
      <span class="price-value">{{ formatPrice(ticker.last) }}</span>
      <span class="price-change" :class="changeClass">
        {{ formatSignedPrice(priceChange24h) }} ({{ formatSignedPercent(changePercent24h) }})
      </span>
    </div>
    <div class="ticker-stats">
      <div class="stat">
        <span class="stat-label">24h高</span>
        <span class="stat-value">{{ formatPrice(ticker.high24h) }}</span>
      </div>
      <div class="stat">
        <span class="stat-label">24h低</span>
        <span class="stat-value">{{ formatPrice(ticker.low24h) }}</span>
      </div>
      <div class="stat">
        <span class="stat-label">24h量</span>
        <span class="stat-value">{{ formatVolume(ticker.vol24h) }}</span>
      </div>
      <div class="stat">
        <span class="stat-label">买一</span>
        <span class="stat-value bid">{{ formatPrice(ticker.bid) }}</span>
      </div>
      <div class="stat">
        <span class="stat-label">卖一</span>
        <span class="stat-value ask">{{ formatPrice(ticker.ask) }}</span>
      </div>
    </div>
  </div>
  <div class="ticker-bar empty" v-else>
    <span class="text-muted">等待行情数据...</span>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { Ticker } from '@/types'
import { formatPrice, formatVolume } from '@/utils/format'

const props = defineProps<{ ticker: Ticker | null }>()

const ticker = computed(() => props.ticker)

const priceChange24h = computed(() => {
  if (!ticker.value) return Number.NaN
  const { last, open24h } = ticker.value
  if (!Number.isFinite(last) || !Number.isFinite(open24h) || open24h <= 0) return Number.NaN
  return last - open24h
})

const changePercent24h = computed(() => {
  if (!ticker.value) return Number.NaN
  const value = ticker.value.change24h
  return Number.isFinite(value) ? value : Number.NaN
})

const changeClass = computed(() => {
  const c = changePercent24h.value
  if (c > 0) return 'positive'
  if (c < 0) return 'negative'
  return ''
})

function formatSignedPrice(value: number): string {
  if (!Number.isFinite(value)) return '--'
  if (value === 0) return '0.00'
  const prefix = value > 0 ? '+' : '-'
  return `${prefix}${formatPrice(Math.abs(value))}`
}

function formatSignedPercent(value: number): string {
  if (!Number.isFinite(value)) return '--'
  if (value === 0) return '0.00%'
  const prefix = value > 0 ? '+' : '-'
  return `${prefix}${Math.abs(value).toFixed(2)}%`
}
</script>

<style scoped>
.ticker-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  margin-bottom: 8px;
  min-height: 48px;
}
.ticker-bar.empty {
  display: flex;
  align-items: center;
  justify-content: center;
}
.ticker-price { display: flex; align-items: baseline; gap: 10px; }
.price-value { font-size: 22px; font-weight: 700; }
.price-change { font-size: 14px; }
.price-change.positive { color: var(--color-positive); }
.price-change.negative { color: var(--color-negative); }
.ticker-stats { display: flex; gap: 16px; }
.stat { text-align: center; }
.stat-label { font-size: 11px; color: var(--color-text-tertiary); display: block; }
.stat-value { font-size: 13px; font-weight: 500; }
.stat-value.bid { color: var(--color-positive); }
.stat-value.ask { color: var(--color-negative); }
.text-muted { color: var(--color-text-tertiary); font-size: 13px; }
</style>
