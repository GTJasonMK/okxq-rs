<template>
  <div class="recent-trades">
    <div class="rt-topbar">
      <div class="rt-title-area">
        <span class="rt-title">{{ viewMode === 'trades' ? '最近成交' : '深度图' }}</span>
        <div v-if="viewMode === 'depth'" class="depth-title-controls" aria-label="深度图范围">
          <label class="depth-title-control">
            <span>左</span>
            <input
              type="number"
              :min="1"
              :max="DEPTH_INPUT_MAX"
              :value="requestedBidDepth"
              @input="onBidDepthInput"
              @change="requestDepthSnapshot"
              @keydown.enter="requestDepthSnapshot"
            />
          </label>
          <label class="depth-title-control">
            <span>右</span>
            <input
              type="number"
              :min="1"
              :max="DEPTH_INPUT_MAX"
              :value="requestedAskDepth"
              @input="onAskDepthInput"
              @change="requestDepthSnapshot"
              @keydown.enter="requestDepthSnapshot"
            />
          </label>
        </div>
      </div>
      <div class="rt-tabs" role="tablist" aria-label="右下角行情视图">
        <button
          type="button"
          class="rt-tab"
          :class="{ active: viewMode === 'trades' }"
          role="tab"
          :aria-selected="viewMode === 'trades'"
          @click="viewMode = 'trades'"
        >
          成交
        </button>
        <button
          type="button"
          class="rt-tab"
          :class="{ active: viewMode === 'depth' }"
          role="tab"
          :aria-selected="viewMode === 'depth'"
          @click="viewMode = 'depth'"
        >
          深度
        </button>
      </div>
    </div>

    <RecentTradeList v-if="viewMode === 'trades'" :trades="trades" />
    <MarketDepthChart
      v-else
      :orderbook="orderbook"
      :bid-depth="requestedBidDepth"
      :ask-depth="requestedAskDepth"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import type { Orderbook, RecentTrade } from '@/types'
import MarketDepthChart from '@/components/market/MarketDepthChart.vue'
import RecentTradeList from '@/components/market/RecentTradeList.vue'

const DEFAULT_DEPTH_ROWS = 40
const DEPTH_INPUT_MAX = 5000

defineProps<{
  trades: RecentTrade[]
  orderbook: Orderbook | null
}>()

const emit = defineEmits<{
  'request-depth': [size: number]
}>()

const viewMode = ref<'trades' | 'depth'>('trades')
const visibleBidDepth = ref(DEFAULT_DEPTH_ROWS)
const visibleAskDepth = ref(DEFAULT_DEPTH_ROWS)

const requestedBidDepth = computed(() => clampRequestedDepth(visibleBidDepth.value))
const requestedAskDepth = computed(() => clampRequestedDepth(visibleAskDepth.value))

function onBidDepthInput(event: Event) {
  visibleBidDepth.value = clampRequestedDepth(Number((event.target as HTMLInputElement).value))
}

function onAskDepthInput(event: Event) {
  visibleAskDepth.value = clampRequestedDepth(Number((event.target as HTMLInputElement).value))
}

function requestDepthSnapshot() {
  emit('request-depth', Math.max(requestedBidDepth.value, requestedAskDepth.value))
}

function clampRequestedDepth(value: number) {
  const candidate = Number.isFinite(value) ? value : DEFAULT_DEPTH_ROWS
  return Math.max(1, Math.min(DEPTH_INPUT_MAX, Math.round(candidate)))
}
</script>

<style scoped>
.recent-trades {
  display: flex;
  flex-direction: column;
  max-height: 100%;
  overflow: hidden;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: var(--color-bg-secondary);
  font-size: 12px;
}

.rt-topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 7px 10px;
  border-bottom: 1px solid var(--color-border);
}

.rt-title-area {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.rt-title {
  color: var(--color-text-primary);
  font-size: 13px;
  font-weight: 600;
  white-space: nowrap;
}

.depth-title-controls {
  display: flex;
  align-items: center;
  gap: 5px;
  min-width: 0;
}

.depth-title-control {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  color: var(--color-text-tertiary);
  font-size: 11px;
  white-space: nowrap;
}

.depth-title-control input {
  width: 54px;
  height: 22px;
  padding: 0 4px;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  background: var(--color-bg-primary);
  color: var(--color-text-primary);
  font-size: 11px;
  outline: none;
}

.depth-title-control input:focus {
  border-color: var(--color-accent);
}

.depth-title-control input:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}

.rt-tabs {
  display: inline-flex;
  padding: 2px;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  background: var(--color-bg-primary);
}

.rt-tab {
  min-height: 22px;
  padding: 0 8px;
  border: none;
  border-radius: 3px;
  background: transparent;
  color: var(--color-text-secondary);
  cursor: pointer;
  font-size: 11px;
}

.rt-tab:hover {
  color: var(--color-text-primary);
}

.rt-tab.active {
  background: var(--color-bg-active);
  color: var(--color-accent);
}
</style>
