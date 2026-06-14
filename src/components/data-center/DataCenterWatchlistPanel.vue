<template>
  <template v-if="active">
    <DataCenterWatchlistToolbar
      :new-symbol="newSymbol"
      :adding="adding"
      :can-open-rule-dialog="canOpenRuleDialog"
      :loading="loading"
      :guardian-running="guardianRunning"
      :visible-symbols-count="visibleSymbolsCount"
      :watched-symbols-count="watchedSymbolsCount"
      :enabled-instrument-count="enabledInstrumentCount"
      :active-jobs-count="activeJobsCount"
      :managed-plan-labels="managedPlanLabels"
      :message="message"
      :error="error"
      @update:new-symbol="$emit('update:new-symbol', $event)"
      @open-rule-dialog="$emit('open-rule-dialog')"
      @load-page-data="$emit('load-page-data')"
      @run-guardian="$emit('run-guardian')"
    />

    <section v-if="watchedRowSources.length === 0" class="dc-empty">
      <div class="dc-empty-title">数据库暂无标的</div>
      <div class="dc-empty-text">数据库有 K 线后会自动显示在这里；也可以先接管规则并提交补齐。</div>
    </section>

    <section
      v-else
      ref="watchlistViewport"
      class="dc-content"
      @scroll="syncVirtualViewport"
    >
      <div
        v-if="virtualWindow.beforeHeight > 0"
        class="dc-virtual-spacer"
        :style="{ height: `${virtualWindow.beforeHeight}px` }"
      ></div>
      <DataCenterWatchlistRow
        v-for="row in visibleWatchedRows"
        :key="row.symbol"
        :row="row"
        :enabled-plans="enabledPlans"
        :repairing-symbol="repairingSymbol"
        :gap-repairing-key="gapRepairingKey"
        :deleting-symbol="deletingSymbol"
        @open-market="$emit('open-market', $event)"
        @edit-symbol="$emit('edit-symbol', $event)"
        @repair-symbol="$emit('repair-symbol', $event)"
        @repair-gap="$emit('repair-gap', $event)"
        @delete-symbol="$emit('delete-symbol', $event)"
        @cancel-row-active-jobs="$emit('cancel-row-active-jobs', $event)"
      />
      <div
        v-if="virtualWindow.afterHeight > 0"
        class="dc-virtual-spacer"
        :style="{ height: `${virtualWindow.afterHeight}px` }"
      ></div>
    </section>
  </template>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import DataCenterWatchlistRow from '@/components/data-center/DataCenterWatchlistRow.vue'
import DataCenterWatchlistToolbar from '@/components/data-center/DataCenterWatchlistToolbar.vue'
import type { SyncJob, WatchedSymbol, WatchedSymbolSyncPlan } from '@/types'
import type { ExactGapRepairPayload, WatchedRow } from '@/types/dataCenter'
import {
  createWatchedRowsBuilder,
  type WatchedRowSource,
} from '@/utils/dataCenter'

const WATCHLIST_ROW_ESTIMATED_HEIGHT = 230
const WATCHLIST_OVERSCAN_ROWS = 8
const WATCHLIST_DEFAULT_VIEWPORT_HEIGHT = 920

const props = defineProps<{
  active: boolean
  newSymbol: string
  adding: boolean
  canOpenRuleDialog: boolean
  loading: boolean
  guardianRunning: boolean
  visibleSymbolsCount: number
  watchedSymbolsCount: number
  enabledInstrumentCount: number
  activeJobsCount: number
  managedPlanLabels: string
  message: string
  error: string
  watchedRowSources: WatchedRowSource[]
  syncJobs: SyncJob[]
  enabledPlans: WatchedSymbolSyncPlan[]
  repairingSymbol: string
  gapRepairingKey: string
  deletingSymbol: string
}>()

defineEmits<{
  'update:new-symbol': [value: string]
  'open-rule-dialog': []
  'load-page-data': []
  'run-guardian': []
  'open-market': [symbol: string]
  'edit-symbol': [row: WatchedSymbol]
  'repair-symbol': [row: WatchedSymbol]
  'repair-gap': [payload: ExactGapRepairPayload]
  'delete-symbol': [symbol: string]
  'cancel-row-active-jobs': [row: WatchedRow]
}>()

const watchlistViewport = ref<HTMLElement | null>(null)
const viewportScrollTop = ref(0)
const viewportHeight = ref(WATCHLIST_DEFAULT_VIEWPORT_HEIGHT)
const buildVisibleWatchedRows = createWatchedRowsBuilder()

const virtualWindow = computed(() => {
  const total = props.watchedRowSources.length
  if (total === 0) {
    return { start: 0, end: 0, beforeHeight: 0, afterHeight: 0 }
  }
  const rowHeight = WATCHLIST_ROW_ESTIMATED_HEIGHT
  const visibleRows = Math.max(1, Math.ceil(viewportHeight.value / rowHeight))
  const firstVisible = Math.floor(viewportScrollTop.value / rowHeight)
  const start = Math.max(0, firstVisible - WATCHLIST_OVERSCAN_ROWS)
  const end = Math.min(total, firstVisible + visibleRows + WATCHLIST_OVERSCAN_ROWS)
  return {
    start,
    end,
    beforeHeight: start * rowHeight,
    afterHeight: Math.max(0, (total - end) * rowHeight),
  }
})

const visibleWatchedRows = computed(() => (
  buildVisibleWatchedRows(
    props.watchedRowSources.slice(virtualWindow.value.start, virtualWindow.value.end),
    props.syncJobs,
  )
))

function syncVirtualViewport() {
  const viewport = watchlistViewport.value
  if (!viewport) return
  viewportScrollTop.value = Math.max(0, viewport.scrollTop)
  viewportHeight.value = viewport.clientHeight || WATCHLIST_DEFAULT_VIEWPORT_HEIGHT
}

function clampVirtualScroll() {
  const viewport = watchlistViewport.value
  if (!viewport) return
  const maxScrollTop = Math.max(
    0,
    props.watchedRowSources.length * WATCHLIST_ROW_ESTIMATED_HEIGHT - viewportHeight.value,
  )
  if (viewport.scrollTop > maxScrollTop) {
    viewport.scrollTop = maxScrollTop
  }
  syncVirtualViewport()
}

onMounted(() => {
  void nextTick(syncVirtualViewport)
  window.addEventListener('resize', syncVirtualViewport)
})

onBeforeUnmount(() => {
  window.removeEventListener('resize', syncVirtualViewport)
})

watch(() => props.watchedRowSources.length, () => {
  void nextTick(clampVirtualScroll)
})
</script>
