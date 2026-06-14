<template>
  <div class="fill-history">
    <div class="fh-header">
      <span class="fh-title">成交历史 ({{ fills.length }})</span>
    </div>
    <div
      ref="fillViewport"
      class="table-wrap"
      @scroll="syncFillViewport"
    >
      <table v-if="fills.length > 0">
        <thead>
          <tr>
            <th>品种</th>
            <th>方向</th>
            <th class="num">价格</th>
            <th class="num">数量</th>
            <th class="num">手续费</th>
            <th>时间</th>
          </tr>
        </thead>
        <tbody>
          <tr v-if="fillWindow.beforeHeight > 0" class="fh-virtual-spacer-row">
            <td
              :colspan="FILL_TABLE_COLUMN_COUNT"
              class="fh-virtual-spacer-cell"
              :style="{ height: `${fillWindow.beforeHeight}px` }"
            ></td>
          </tr>
          <tr v-for="f in visibleFills" :key="f.fill_id">
            <td class="symbol-cell">{{ f.inst_id }}</td>
            <td>
              <span class="side-badge" :class="f.side">{{ f.side === 'buy' ? '买' : '卖' }}</span>
            </td>
            <td class="num">{{ formatPrice(f.fill_px) }}</td>
            <td class="num">{{ formatNullableSize(f.fill_sz) }}</td>
            <td class="num" :class="feeColor(f.fee)">{{ formatPrice(f.fee) }} {{ f.fee_ccy }}</td>
            <td class="time-cell">{{ formatFillTime(f.fill_time) }}</td>
          </tr>
          <tr v-if="fillWindow.afterHeight > 0" class="fh-virtual-spacer-row">
            <td
              :colspan="FILL_TABLE_COLUMN_COUNT"
              class="fh-virtual-spacer-cell"
              :style="{ height: `${fillWindow.afterHeight}px` }"
            ></td>
          </tr>
        </tbody>
      </table>
      <div v-else class="empty-text">暂无成交记录</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { Fill } from '@/types'
import { formatPrice } from '@/utils/format'
import { pnlColor } from '@/utils/color'

const FILL_ROW_ESTIMATED_HEIGHT = 34
const FILL_OVERSCAN_ROWS = 8
const FILL_DEFAULT_VIEWPORT_HEIGHT = 320
const FILL_VIRTUALIZE_THRESHOLD = 60
const FILL_TABLE_COLUMN_COUNT = 6

const props = defineProps<{ fills: Fill[] }>()
const fillViewport = ref<HTMLElement | null>(null)
const fillScrollTop = ref(0)
const fillViewportHeight = ref(FILL_DEFAULT_VIEWPORT_HEIGHT)

const fillWindow = computed(() => {
  const total = props.fills.length
  if (total === 0 || total <= FILL_VIRTUALIZE_THRESHOLD) {
    return { start: 0, end: total, beforeHeight: 0, afterHeight: 0 }
  }
  const rowHeight = FILL_ROW_ESTIMATED_HEIGHT
  const visibleRows = Math.max(1, Math.ceil(fillViewportHeight.value / rowHeight))
  const firstVisible = Math.floor(fillScrollTop.value / rowHeight)
  const start = Math.max(0, firstVisible - FILL_OVERSCAN_ROWS)
  const end = Math.min(total, firstVisible + visibleRows + FILL_OVERSCAN_ROWS)
  return {
    start,
    end,
    beforeHeight: start * rowHeight,
    afterHeight: Math.max(0, (total - end) * rowHeight),
  }
})
const visibleFills = computed(() => props.fills.slice(fillWindow.value.start, fillWindow.value.end))

function syncFillViewport() {
  const viewport = fillViewport.value
  if (!viewport) return
  fillScrollTop.value = Math.max(0, viewport.scrollTop)
  fillViewportHeight.value = viewport.clientHeight || FILL_DEFAULT_VIEWPORT_HEIGHT
}

function clampFillScroll() {
  const viewport = fillViewport.value
  if (!viewport) return
  const maxScrollTop = Math.max(
    0,
    props.fills.length * FILL_ROW_ESTIMATED_HEIGHT - fillViewportHeight.value,
  )
  if (viewport.scrollTop > maxScrollTop) {
    viewport.scrollTop = maxScrollTop
  }
  syncFillViewport()
}

function formatNullableSize(value: number | null): string {
  return Number.isFinite(value) ? String(value) : '--'
}

function feeColor(value: number | null): string {
  return Number.isFinite(value) ? pnlColor(-(value as number)) : pnlColor(null)
}

function formatFillTime(ts: number | null): string {
  if (typeof ts !== 'number' || !Number.isFinite(ts)) return '--'
  const d = new Date(ts)
  return d.toLocaleString('zh-CN', { hour12: false })
}

onMounted(() => {
  void nextTick(syncFillViewport)
  window.addEventListener('resize', syncFillViewport)
})

onBeforeUnmount(() => {
  window.removeEventListener('resize', syncFillViewport)
})

watch(() => props.fills.length, () => {
  void nextTick(clampFillScroll)
})
</script>

<style scoped>
.fill-history {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  min-height: 0;
}
.fh-header {
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border);
}
.fh-title { font-size: 13px; font-weight: 600; }
.table-wrap {
  overflow: auto;
  min-height: 0;
  max-height: 320px;
}
table { width: 100%; border-collapse: collapse; font-size: 12px; }
th {
  text-align: left;
  padding: 6px 10px;
  color: var(--color-text-tertiary);
  font-weight: 500;
  font-size: 11px;
  white-space: nowrap;
}
th.num { text-align: right; }
td {
  padding: 5px 10px;
  border-top: 1px solid var(--color-border);
  white-space: nowrap;
}
td.num { text-align: right; font-variant-numeric: tabular-nums; }
.fh-virtual-spacer-cell {
  height: 0;
  padding: 0;
  border-top: 0;
}
.symbol-cell { font-weight: 600; }
.side-badge {
  display: inline-block;
  padding: 1px 6px;
  border-radius: 3px;
  font-size: 11px;
  font-weight: 500;
}
.side-badge.buy { background: rgba(38,166,154,0.15); color: var(--color-positive); }
.side-badge.sell { background: rgba(239,83,80,0.15); color: var(--color-negative); }
.time-cell { font-size: 11px; color: var(--color-text-secondary); }
.positive { color: var(--color-positive); }
.negative { color: var(--color-negative); }
.empty-text {
  padding: 24px;
  text-align: center;
  color: var(--color-text-tertiary);
  font-size: 13px;
}
</style>
