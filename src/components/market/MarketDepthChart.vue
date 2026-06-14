<template>
  <div v-if="orderbook && depthChart.hasData" class="depth-view">
    <div class="depth-stats">
      <span class="bid-color">买 {{ formatVolume(depthChart.bidTotal) }} ({{ depthChart.bidDepthPct }}%)</span>
      <span>价差 {{ spread !== null ? formatPrice(spread) : '--' }}</span>
      <span class="ask-color">({{ 100 - depthChart.bidDepthPct }}%) 卖 {{ formatVolume(depthChart.askTotal) }}</span>
    </div>
    <svg
      ref="depthSvgRef"
      class="depth-svg"
      :viewBox="`0 0 ${CHART_WIDTH} ${CHART_HEIGHT}`"
      preserveAspectRatio="none"
      role="img"
      aria-label="订单簿累计深度图"
      @pointermove="onDepthPointerMove"
      @pointerleave="clearDepthHover"
    >
      <defs>
        <linearGradient :id="bidFillId" x1="0" x2="0" y1="0" y2="1">
          <stop offset="0%" stop-color="var(--color-positive)" stop-opacity="0.28" />
          <stop offset="100%" stop-color="var(--color-positive)" stop-opacity="0.02" />
        </linearGradient>
        <linearGradient :id="askFillId" x1="0" x2="0" y1="0" y2="1">
          <stop offset="0%" stop-color="var(--color-negative)" stop-opacity="0.28" />
          <stop offset="100%" stop-color="var(--color-negative)" stop-opacity="0.02" />
        </linearGradient>
      </defs>
      <line
        v-for="lineY in gridLines"
        :key="lineY"
        class="depth-grid"
        :x1="CHART_PADDING.left"
        :x2="CHART_WIDTH - CHART_PADDING.right"
        :y1="lineY"
        :y2="lineY"
      />
      <line
        v-if="depthChart.midX !== null"
        class="depth-midline"
        :x1="depthChart.midX"
        :x2="depthChart.midX"
        :y1="CHART_PADDING.top"
        :y2="CHART_HEIGHT - CHART_PADDING.bottom"
      />
      <polygon
        v-if="depthChart.bidAreaPoints"
        class="depth-area"
        :fill="`url(#${bidFillId})`"
        :points="depthChart.bidAreaPoints"
      />
      <polygon
        v-if="depthChart.askAreaPoints"
        class="depth-area"
        :fill="`url(#${askFillId})`"
        :points="depthChart.askAreaPoints"
      />
      <polyline
        v-if="depthChart.bidLinePoints"
        class="depth-line bid-line"
        :points="depthChart.bidLinePoints"
      />
      <polyline
        v-if="depthChart.askLinePoints"
        class="depth-line ask-line"
        :points="depthChart.askLinePoints"
      />
      <g v-if="depthHover" class="depth-hover">
        <line
          class="depth-hover-line"
          :x1="depthHover.x"
          :x2="depthHover.x"
          :y1="CHART_PADDING.top"
          :y2="CHART_HEIGHT - CHART_PADDING.bottom"
        />
        <rect
          class="depth-hover-label-bg"
          :x="depthHover.labelX - depthHover.labelWidth / 2"
          :y="CHART_HEIGHT - CHART_PADDING.bottom - 21"
          :width="depthHover.labelWidth"
          height="17"
          rx="3"
        />
        <text
          class="depth-hover-label"
          :x="depthHover.labelX"
          :y="CHART_HEIGHT - CHART_PADDING.bottom - 9"
          text-anchor="middle"
        >
          {{ depthHover.text }}
        </text>
      </g>
    </svg>
    <div class="depth-axis">
      <span>{{ formatPrice(depthChart.minPrice) }}</span>
      <span>{{ depthChart.midPrice !== null ? formatPrice(depthChart.midPrice) : '--' }}</span>
      <span>{{ formatPrice(depthChart.maxPrice) }}</span>
    </div>
  </div>
  <div v-else class="rt-empty">等待深度数据...</div>
</template>

<script setup lang="ts">
import type { Orderbook } from '@/types'
import { useMarketDepthChart } from '@/composables/useMarketDepthChart'
import { formatPrice, formatVolume } from '@/utils/format'

const props = defineProps<{
  orderbook: Orderbook | null
  bidDepth: number
  askDepth: number
}>()

const {
  CHART_WIDTH,
  CHART_HEIGHT,
  CHART_PADDING,
  gridLines,
  bidFillId,
  askFillId,
  depthSvgRef,
  depthHover,
  spread,
  depthChart,
  onDepthPointerMove,
  clearDepthHover,
} = useMarketDepthChart({
  orderbook: () => props.orderbook,
  bidDepth: () => props.bidDepth,
  askDepth: () => props.askDepth,
})
</script>

<style scoped>
.depth-view {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px;
}

.depth-stats,
.depth-axis {
  display: flex;
  justify-content: space-between;
  gap: 8px;
  color: var(--color-text-secondary);
  font-size: 11px;
}

.bid-color {
  color: var(--color-positive);
}

.ask-color {
  color: var(--color-negative);
}

.depth-svg {
  flex: 1;
  min-height: 180px;
  width: 100%;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  background: var(--color-bg-primary);
}

.depth-grid {
  stroke: rgba(255, 255, 255, 0.06);
  stroke-width: 1;
}

.depth-midline {
  stroke: rgba(255, 255, 255, 0.18);
  stroke-dasharray: 3 3;
  stroke-width: 1;
}

.depth-area {
  pointer-events: none;
}

.depth-line {
  fill: none;
  stroke-width: 2;
  stroke-linejoin: round;
  stroke-linecap: round;
  vector-effect: non-scaling-stroke;
}

.bid-line {
  stroke: var(--color-positive);
}

.ask-line {
  stroke: var(--color-negative);
}

.depth-hover {
  pointer-events: none;
}

.depth-hover-line {
  stroke: var(--color-accent);
  stroke-dasharray: 3 3;
  stroke-width: 1;
  opacity: 0.9;
  vector-effect: non-scaling-stroke;
}

.depth-hover-label-bg {
  fill: var(--color-bg-secondary);
  stroke: var(--color-border);
  stroke-width: 1;
  opacity: 0.96;
  vector-effect: non-scaling-stroke;
}

.depth-hover-label {
  fill: var(--color-text-primary);
  font-size: 10px;
  font-weight: 600;
  dominant-baseline: middle;
}

.depth-axis {
  color: var(--color-text-tertiary);
}

.depth-axis span:nth-child(2) {
  color: var(--color-text-secondary);
}

.rt-empty {
  padding: 20px;
  text-align: center;
  color: var(--color-text-tertiary);
}
</style>
