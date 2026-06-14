<template>
  <div class="market-selector">
    <div class="watch-select">
      <ThemeSelect
        :model-value="props.symbol"
        :options="watchSymbolOptions"
        placeholder="选择关注币种"
        size="md"
        @update:model-value="handleWatchSymbolChange"
      />
    </div>
    <div class="symbol-input">
      <input
        v-model="localSymbol"
        class="symbol-field"
        placeholder="BTC-USDT"
        @keydown.enter="emitSymbol"
        @blur="emitSymbol"
      />
    </div>
    <div class="timeframe-buttons">
      <button
        v-for="tf in timeframes"
        :key="tf"
        class="tf-btn"
        :class="{ active: tf === props.timeframe }"
        @click="$emit('update:timeframe', tf)"
      >
        {{ tf }}
      </button>
    </div>
    <div class="range-select">
      <span class="range-label">范围</span>
      <ThemeSelect
        :model-value="rangeSelectValue"
        :options="rangeSelectOptions"
        placeholder="选择范围"
        size="md"
        @update:model-value="handleRangeChange"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { Timeframe } from '@/types'
import type { WatchedSymbol } from '@/types/market'
import type { CandleRangeDays } from '@/types/marketView'
import type { ThemeSelectValue } from '@/composables/useThemeSelect'
import { candleRangeOptionsForTimeframe, VALID_MARKET_TIMEFRAMES } from '@/utils/marketView'
import ThemeSelect from '@/components/shared/ThemeSelect.vue'

const timeframes: Timeframe[] = [...VALID_MARKET_TIMEFRAMES]

const props = defineProps<{
  symbol: string
  timeframe: Timeframe
  rangeDays: CandleRangeDays
  watchedSymbols: WatchedSymbol[]
}>()

const emit = defineEmits<{
  'update:symbol': [value: string]
  'update:timeframe': [value: Timeframe]
  'update:range-days': [value: CandleRangeDays]
}>()

const localSymbol = ref(props.symbol)
const rangeOptions = computed(() => candleRangeOptionsForTimeframe(props.timeframe))
const rangeSelectValue = computed(() => String(props.rangeDays))
const rangeSelectOptions = computed(() =>
  rangeOptions.value.map(option => ({
    label: option.label,
    value: String(option.value),
  }))
)
const watchSymbolOptions = computed(() =>
  props.watchedSymbols.map(item => ({
    label: `${item.symbol}${marketSuffix(item)}`,
    value: item.symbol,
  }))
)

watch(() => props.symbol, (symbol) => {
  localSymbol.value = symbol
})

function emitSymbol() {
  const v = localSymbol.value.trim().toUpperCase()
  if (v && v !== props.symbol) emit('update:symbol', v)
}

function handleWatchSymbolChange(value: ThemeSelectValue) {
  if (value && value !== props.symbol) emit('update:symbol', value)
}

function handleRangeChange(value: ThemeSelectValue) {
  const range = Number(value) as CandleRangeDays
  if (rangeOptions.value.some(option => option.value === range) && range !== props.rangeDays) {
    emit('update:range-days', range)
  }
}

function marketSuffix(item: WatchedSymbol) {
  if (item.sync_spot && item.sync_swap) return ' · 现货/永续'
  if (item.sync_swap) return ' · 永续'
  if (item.sync_spot) return ' · 现货'
  return ''
}
</script>

<style scoped>
.market-selector {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 8px;
  flex-wrap: wrap;
}
.symbol-field {
  width: 140px;
  padding: 6px 10px;
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 4px;
  color: var(--color-text-primary);
  font-size: 14px;
  font-weight: 600;
  text-transform: uppercase;
  outline: none;
}
.watch-select {
  width: 190px;
}
.symbol-field:focus { border-color: var(--color-accent); }
.timeframe-buttons,
.range-select {
  display: flex;
  align-items: center;
  gap: 2px;
  min-width: 0;
}
.timeframe-buttons {
  flex: 1 1 420px;
  flex-wrap: wrap;
}
.range-select {
  flex: 0 0 132px;
  gap: 6px;
}
.range-label {
  padding: 0 4px;
  color: var(--color-text-tertiary);
  font-size: 12px;
}
.tf-btn {
  flex: 0 0 auto;
  padding: 4px 10px;
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 4px;
  color: var(--color-text-secondary);
  font-size: 12px;
  cursor: pointer;
  transition: all 0.15s;
}
.tf-btn:hover {
  color: var(--color-text-primary);
  border-color: var(--color-text-tertiary);
}
.tf-btn.active {
  background: var(--color-accent);
  color: #fff;
  border-color: var(--color-accent);
}
</style>
