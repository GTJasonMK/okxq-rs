<template>
  <div
    class="ecc-tooltip"
    :style="{ left: `${tooltip.x}px`, top: `${tooltip.y}px` }"
  >
    <div class="ecc-tooltip-head">
      <div class="ecc-tooltip-title">
        <span>{{ tooltip.time }}</span>
        <strong :class="tooltip.positionClass">{{ tooltip.positionDetail }}</strong>
      </div>
      <div class="ecc-tooltip-meta">
        <span>权益 {{ tooltip.equity }}</span>
        <strong :class="tooltip.positive ? 'positive' : 'negative'">{{ tooltip.change }}</strong>
      </div>
    </div>
    <div class="ecc-tooltip-summary">
      <span>
        未实现
        <strong :class="tooltip.unrealizedClass">{{ tooltip.unrealized }}</strong>
      </span>
      <span>
        暴露
        <strong :class="tooltip.positionClass">{{ tooltip.exposure }}</strong>
      </span>
      <span>
        事件
        <strong>{{ tooltip.events.length }}</strong>
      </span>
    </div>
    <div class="ecc-tooltip-positions">
      <div class="ecc-tooltip-positions-head">
        <span>{{ tooltip.positionTitle }}</span>
        <strong>{{ tooltip.positionsTotal }}</strong>
      </div>
      <div v-if="tooltip.positions.length > 0" class="ecc-tooltip-position-list">
        <div
          v-for="position in tooltip.positions"
          :key="position.key"
          class="ecc-tooltip-position"
        >
          <div class="position-main">
            <span class="position-symbol">{{ position.symbol }}</span>
            <span class="position-side" :class="position.sideClass">{{ position.side }}</span>
            <strong :class="position.pnlClass">{{ position.pnl }}</strong>
            <span :class="position.returnClass">{{ position.returnPct }}</span>
          </div>
          <div class="position-meta">
            <span>数量 {{ position.quantity }}</span>
            <span>入场 {{ position.entryPrice }}</span>
            <span>标记 {{ position.markPrice }}</span>
            <span>名义 {{ position.notional }}</span>
          </div>
        </div>
        <div v-if="tooltip.positionsMore" class="ecc-tooltip-more">
          {{ tooltip.positionsMore }}
        </div>
      </div>
      <div v-else class="ecc-tooltip-empty">{{ tooltip.positionEmpty }}</div>
    </div>
    <div class="ecc-tooltip-events">
      <div class="ecc-tooltip-events-head">
        <span>{{ tooltip.eventTitle }}</span>
        <strong>{{ tooltip.events.length }}</strong>
      </div>
      <div v-if="tooltip.events.length > 0" class="ecc-tooltip-event-list">
        <div
          v-for="event in tooltip.events"
          :key="event.key"
          class="ecc-tooltip-event"
        >
          <span class="event-time">{{ event.time }}</span>
          <span class="event-symbol">{{ event.symbol }}</span>
          <span class="event-side" :class="event.sideClass">{{ event.label }}</span>
          <strong :class="event.pnlClass">{{ event.pnl }}</strong>
        </div>
      </div>
      <div v-else class="ecc-tooltip-empty">本K线无交易事件</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { HoverTooltip } from '@/utils/equityCandleChart'

defineProps<{
  tooltip: HoverTooltip
}>()
</script>

<style scoped>
.ecc-tooltip {
  position: absolute;
  z-index: 5;
  display: flex;
  flex-direction: column;
  box-sizing: border-box;
  width: min(404px, calc(100% - 16px));
  max-height: min(520px, calc(100% - 16px));
  overflow: hidden;
  border: 1px solid rgba(255,255,255,0.1);
  border-radius: 6px;
  background: rgba(14, 17, 28, 0.94);
  box-shadow: 0 14px 36px rgba(0,0,0,0.34);
  color: var(--color-text-secondary);
  font-size: 11px;
  line-height: 1.35;
  pointer-events: none;
  backdrop-filter: blur(8px);
}
.ecc-tooltip-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
  padding: 8px 10px;
  border-bottom: 1px solid rgba(255,255,255,0.07);
}
.ecc-tooltip-title,
.ecc-tooltip-meta {
  min-width: 0;
}
.ecc-tooltip-title {
  display: flex;
  flex: 1 1 auto;
  flex-direction: column;
  gap: 3px;
}
.ecc-tooltip-meta {
  display: flex;
  flex: 0 0 auto;
  flex-direction: column;
  align-items: flex-end;
  gap: 3px;
  font-variant-numeric: tabular-nums;
}
.ecc-tooltip-title span,
.ecc-tooltip-title strong,
.ecc-tooltip-meta span,
.ecc-tooltip-meta strong {
  min-width: 0;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ecc-tooltip-title span {
  color: var(--color-text-primary);
  font-size: 12px;
  font-weight: 600;
}
.ecc-tooltip-title strong {
  align-self: flex-start;
}
.ecc-tooltip-meta span {
  color: var(--color-text-tertiary);
}
.ecc-tooltip-meta strong {
  font-size: 12px;
}
.ecc-tooltip-summary {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 14px;
  padding: 6px 10px;
  border-bottom: 1px solid rgba(255,255,255,0.07);
  font-variant-numeric: tabular-nums;
}
.ecc-tooltip-summary span {
  display: inline-flex;
  gap: 4px;
  align-items: baseline;
  min-width: 0;
  color: var(--color-text-tertiary);
}
.ecc-tooltip-summary strong {
  color: var(--color-text-primary);
  font-size: 11px;
  font-weight: 600;
}
.ecc-tooltip-summary strong.positive {
  color: var(--color-positive);
}
.ecc-tooltip-summary strong.negative {
  color: var(--color-negative);
}
.ecc-tooltip-summary span,
.ecc-tooltip-summary strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ecc-tooltip-events {
  flex: 0 1 auto;
  min-width: 0;
  min-height: 0;
}
.ecc-tooltip-positions {
  flex: 1 1 auto;
  min-width: 0;
  min-height: 0;
  border-bottom: 1px solid rgba(255,255,255,0.07);
}
.ecc-tooltip-positions-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 7px 10px;
  color: var(--color-text-secondary);
}
.ecc-tooltip-position-list {
  max-height: 260px;
  overflow: auto;
  border-top: 1px solid rgba(255,255,255,0.06);
}
.ecc-tooltip-position {
  padding: 6px 10px;
  border-bottom: 1px solid rgba(255,255,255,0.05);
  font-variant-numeric: tabular-nums;
}
.ecc-tooltip-position:last-child {
  border-bottom: 0;
}
.position-main {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 42px 68px 54px;
  gap: 8px;
  align-items: center;
}
.position-main span,
.position-main strong,
.position-meta span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.position-main strong,
.position-main span:last-child {
  text-align: right;
}
.position-symbol {
  color: var(--color-text-primary);
  font-weight: 600;
}
.position-side {
  justify-self: start;
  border-radius: 4px;
  padding: 1px 5px;
  background: rgba(148, 163, 184, 0.12);
}
.position-side.positive {
  background: rgba(38,166,154,0.16);
}
.position-side.negative {
  background: rgba(239,83,80,0.16);
}
.position-meta {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 8px;
  margin-top: 3px;
  color: var(--color-text-tertiary);
}
.ecc-tooltip-more {
  padding: 6px 10px 7px;
  border-top: 1px solid rgba(255,255,255,0.05);
  color: var(--color-text-tertiary);
}
.ecc-tooltip-events-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 7px 10px;
  color: var(--color-text-secondary);
}
.ecc-tooltip-event-list {
  max-height: 156px;
  overflow: auto;
  border-top: 1px solid rgba(255,255,255,0.06);
}
.ecc-tooltip-event {
  display: grid;
  grid-template-columns: minmax(0, 1.2fr) 42px 42px 58px;
  gap: 6px;
  align-items: center;
  padding: 6px 10px;
  border-bottom: 1px solid rgba(255,255,255,0.05);
  font-variant-numeric: tabular-nums;
}
.ecc-tooltip-event:last-child {
  border-bottom: 0;
}
.ecc-tooltip-event span,
.ecc-tooltip-event strong {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ecc-tooltip-event strong {
  text-align: right;
}
.event-symbol {
  color: var(--color-text-tertiary);
  font-weight: 600;
}
.event-side {
  justify-self: start;
  border-radius: 4px;
  padding: 1px 5px;
  background: rgba(148, 163, 184, 0.12);
}
.event-side.positive {
  background: rgba(38,166,154,0.16);
}
.event-side.negative {
  background: rgba(239,83,80,0.16);
}
.ecc-tooltip-empty {
  padding: 8px 10px 10px;
  border-top: 1px solid rgba(255,255,255,0.06);
  color: var(--color-text-tertiary);
}
.positive { color: var(--color-positive); }
.negative { color: var(--color-negative); }
</style>
