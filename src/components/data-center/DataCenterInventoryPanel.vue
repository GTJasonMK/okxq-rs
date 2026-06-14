<template>
  <section v-if="active" class="dc-panel">
    <div class="dc-panel-head">
      <div>
        <h2>数据库库存</h2>
        <p>本地库存按币种聚合，区分规则管理数据和删除残留。</p>
      </div>
      <div class="dc-panel-actions">
        <span v-if="activeJobsCount > 0" class="dc-muted">
          后台任务 {{ activeJobsCount }} 个，完成后自动刷新
        </span>
        <span v-if="rebuildProgress" class="dc-muted">
          {{ rebuildProgressLabel(rebuildProgress) }}
        </span>
        <button
          class="dc-btn"
          type="button"
          :disabled="loading || rebuilding"
          title="扫描 candles 全表并重建 sync_records，数据量大时会比较慢"
          @click="$emit('rebuild-cache')"
        >
          {{ rebuilding ? '扫描中' : '全库扫描' }}
        </button>
        <button class="dc-btn" type="button" :disabled="loading || rebuilding" @click="$emit('load-inventory')">
          {{ loading ? '刷新中' : '刷新库存' }}
        </button>
      </div>
    </div>
    <div v-if="message || error" class="dc-feedback" :class="{ error: !!error }">
      {{ error || message }}
    </div>
    <div class="dc-kpi-grid">
      <div class="dc-kpi">
        <span>库存币种</span>
        <strong>{{ summary.symbol_count ?? 0 }}</strong>
      </div>
      <div class="dc-kpi">
        <span>规则管理</span>
        <strong>{{ summary.managed_symbol_count ?? 0 }}</strong>
      </div>
      <div class="dc-kpi">
        <span>关注覆盖</span>
        <strong>{{ summary.watched_symbol_count ?? 0 }}/{{ summary.watched_list_count ?? 0 }}</strong>
      </div>
      <div class="dc-kpi">
        <span>删除残留</span>
        <strong>{{ summary.orphan_symbol_count ?? 0 }}</strong>
      </div>
      <div class="dc-kpi">
        <span>K 线</span>
        <strong>{{ formatCount(summary.total_candles ?? 0) }}</strong>
      </div>
    </div>
    <section
      ref="inventoryViewport"
      class="dc-inventory-table"
      @scroll="syncInventoryViewport"
    >
      <div
        v-if="inventoryWindow.beforeHeight > 0"
        class="dc-inventory-virtual-spacer"
        :style="{ height: `${inventoryWindow.beforeHeight}px` }"
      ></div>
      <article v-for="row in visibleInventoryRows" :key="row.symbol" class="dc-inventory-row">
        <div class="dc-row-head">
          <div>
            <div class="dc-symbol">{{ row.symbol }}</div>
            <div class="dc-meta">
              <span>{{ row.base_ccy || row.symbol.split('-')[0] }}</span>
              <span>{{ row.managed ? '规则管理' : '未纳入规则' }}</span>
              <span v-if="row.orphan">删除残留</span>
              <span>{{ formatCount(row.storage_counts?.total ?? 0) }} 条本地记录</span>
            </div>
          </div>
          <button class="dc-btn small" type="button" @click="$emit('open-market', row.symbol)">行情</button>
        </div>
        <div class="dc-markets">
          <div
            v-for="market in inventoryMarkets(row)"
            :key="`${row.symbol}-${market.inst_type}`"
            class="dc-market"
            :class="{ disabled: !market.managed }"
          >
            <div class="dc-market-head">
              <span>{{ market.inst_type }}</span>
              <strong>{{ market.inst_id }}</strong>
            </div>
            <div class="dc-coverage">
              <span class="dc-chip" :class="market.candle_count > 0 ? 'ok' : 'missing'">
                {{ market.timeframe_count }} 周期
              </span>
              <span class="dc-chip partial">
                {{ market.history_complete_count }} 全量
              </span>
              <span class="dc-chip" :class="market.gap_count > 0 ? 'failed' : 'ok'">
                {{ inventoryMarketGapLabel(market) }}
              </span>
            </div>
            <div class="dc-muted">{{ inventoryMarketSummary(market) }}</div>
            <div v-if="market.timeframes.length" class="dc-timeframes">
              <div
                v-for="timeframe in market.timeframes"
                :key="`${market.inst_id}-${timeframe.timeframe}`"
                class="dc-timeframe-row"
              >
                <div class="dc-timeframe-main">
                  <strong>{{ timeframe.timeframe }}</strong>
                  <span>{{ inventoryTimeframeRangeLabel(timeframe) }}</span>
                </div>
                <div class="dc-timeframe-meta">
                  <span class="dc-chip" :class="timeframe.gap_count > 0 ? 'failed' : 'ok'">
                    {{ inventoryTimeframeGapLabel(timeframe) }}
                  </span>
                  <span class="dc-chip partial">
                    {{ inventoryTimeframeCoverageLabel(timeframe) }}
                  </span>
                </div>
                <button
                  class="dc-btn small"
                  type="button"
                  :disabled="!canRepairTimeframe(timeframe) || gapRepairingKey === repairKey(market, timeframe)"
                  :title="repairButtonTitle(timeframe)"
                  @click="emitRepairGap(market, timeframe)"
                >
                  {{ gapRepairingKey === repairKey(market, timeframe) ? '提交中' : '精确补齐' }}
                </button>
              </div>
            </div>
          </div>
        </div>
      </article>
      <div
        v-if="inventoryWindow.afterHeight > 0"
        class="dc-inventory-virtual-spacer"
        :style="{ height: `${inventoryWindow.afterHeight}px` }"
      ></div>
    </section>
    <section v-if="tableTotals.length" class="dc-storage-totals">
      <div v-for="item in tableTotals" :key="item.key">
        <span>{{ storageCountLabel(item.key) }}</span>
        <strong>{{ formatCount(item.value) }}</strong>
      </div>
    </section>
  </section>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type {
  InventoryGapRepairPayload,
  InventoryCacheRebuildProgress,
  InventoryMarket,
  InventoryRow,
  InventorySummary,
  InventoryTimeframeRecord,
} from '@/types/dataCenter'
import {
  formatCount,
  inventoryMarketGapLabel,
  inventoryMarketSummary,
  inventoryMarkets,
  inventoryTimeframeCoverageLabel,
  inventoryTimeframeGapLabel,
  inventoryTimeframeRangeLabel,
  storageCountLabel,
} from '@/utils/dataCenter'

const INVENTORY_ROW_ESTIMATED_HEIGHT = 360
const INVENTORY_OVERSCAN_ROWS = 6
const INVENTORY_DEFAULT_VIEWPORT_HEIGHT = 900

const props = defineProps<{
  active: boolean
  rows: InventoryRow[]
  summary: InventorySummary
  tableTotals: Array<{ key: string; value: number }>
  loading: boolean
  rebuilding: boolean
  rebuildProgress: InventoryCacheRebuildProgress | null
  activeJobsCount: number
  message: string
  error: string
  gapRepairingKey: string
}>()

const emit = defineEmits<{
  'load-inventory': []
  'rebuild-cache': []
  'open-market': [symbol: string]
  'repair-gap': [payload: InventoryGapRepairPayload]
}>()

const inventoryViewport = ref<HTMLElement | null>(null)
const inventoryScrollTop = ref(0)
const inventoryViewportHeight = ref(INVENTORY_DEFAULT_VIEWPORT_HEIGHT)

const inventoryWindow = computed(() => {
  const total = props.rows.length
  if (total === 0) {
    return { start: 0, end: 0, beforeHeight: 0, afterHeight: 0 }
  }
  const rowHeight = INVENTORY_ROW_ESTIMATED_HEIGHT
  const visibleRows = Math.max(1, Math.ceil(inventoryViewportHeight.value / rowHeight))
  const firstVisible = Math.floor(inventoryScrollTop.value / rowHeight)
  const start = Math.max(0, firstVisible - INVENTORY_OVERSCAN_ROWS)
  const end = Math.min(total, firstVisible + visibleRows + INVENTORY_OVERSCAN_ROWS)
  return {
    start,
    end,
    beforeHeight: start * rowHeight,
    afterHeight: Math.max(0, (total - end) * rowHeight),
  }
})

const visibleInventoryRows = computed(() => (
  props.rows.slice(inventoryWindow.value.start, inventoryWindow.value.end)
))

function syncInventoryViewport() {
  const viewport = inventoryViewport.value
  if (!viewport) return
  inventoryScrollTop.value = Math.max(0, viewport.scrollTop)
  inventoryViewportHeight.value = viewport.clientHeight || INVENTORY_DEFAULT_VIEWPORT_HEIGHT
}

function clampInventoryScroll() {
  const viewport = inventoryViewport.value
  if (!viewport) return
  const maxScrollTop = Math.max(
    0,
    props.rows.length * INVENTORY_ROW_ESTIMATED_HEIGHT - inventoryViewportHeight.value,
  )
  if (viewport.scrollTop > maxScrollTop) {
    viewport.scrollTop = maxScrollTop
  }
  syncInventoryViewport()
}

function repairKey(market: InventoryMarket, timeframe: InventoryTimeframeRecord) {
  return `${market.inst_id}:${market.inst_type}:${timeframe.timeframe}`
}

function canRepairTimeframe(timeframe: InventoryTimeframeRecord) {
  return (
    timeframe.gap_count > 0 &&
    isValidTimestamp(timeframe.oldest_timestamp) &&
    isValidTimestamp(timeframe.newest_timestamp) &&
    Number(timeframe.newest_timestamp) >= Number(timeframe.oldest_timestamp)
  )
}

function repairButtonTitle(timeframe: InventoryTimeframeRecord) {
  if (canRepairTimeframe(timeframe)) return '按该周期本地时间范围精确补齐缺失 K 线'
  if (timeframe.gap_count <= 0) return '当前周期没有缺失 K 线'
  return '缺少有效本地时间范围，无法精确补齐'
}

function emitRepairGap(market: InventoryMarket, timeframe: InventoryTimeframeRecord) {
  if (!canRepairTimeframe(timeframe)) return
  emit('repair-gap', {
    inst_id: market.inst_id,
    inst_type: market.inst_type,
    timeframe: timeframe.timeframe,
    start_ts: Number(timeframe.oldest_timestamp),
    end_ts: Number(timeframe.newest_timestamp),
  })
}

function rebuildProgressLabel(progress: InventoryCacheRebuildProgress) {
  const pct = Math.max(0, Math.min(100, Math.round(progress.progress || 0)))
  if (progress.target_groups > 0 && progress.status === 'running') {
    return [
      progress.message || '并发扫描中',
      `${pct}%`,
      `分组 ${formatCount(progress.processed_groups)} / ${formatCount(progress.target_groups)}`,
      `K线 ${formatCount(progress.processed_candles)}`,
      `并发 ${formatCount(progress.scan_concurrency)}`,
    ].join(' · ')
  }
  if (progress.target_candles > 0 && progress.status === 'running') {
    return `${progress.message} · ${pct}% · ${formatCount(progress.processed_candles)} / ${formatCount(progress.target_candles)}`
  }
  return `${progress.message || progress.status} · ${pct}%`
}

function isValidTimestamp(value: unknown) {
  return typeof value === 'number' && Number.isFinite(value) && value > 0
}

onMounted(() => {
  void nextTick(syncInventoryViewport)
  window.addEventListener('resize', syncInventoryViewport)
})

onBeforeUnmount(() => {
  window.removeEventListener('resize', syncInventoryViewport)
})

watch(() => props.rows.length, () => {
  void nextTick(clampInventoryScroll)
})
</script>
