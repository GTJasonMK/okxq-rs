import { computed, ref, shallowRef, triggerRef, type Ref } from 'vue'
import * as api from '@/api/market'
import type { SyncRecord } from '@/types'
import type { InventoryCacheRebuildProgress, InventoryRow, InventorySummary } from '@/types/dataCenter'
import { describeError, logger } from '@/utils/logger'
import {
  buildInventoryTableTotals,
  buildSyncRecordScopeIndex,
  emptyInventorySummary,
  formatCount,
  inventoryRowsToSyncRecords,
  replaceSyncRecordScopesInPlace,
  type SyncRecordScopeIndex,
} from '@/utils/dataCenter'

type DataCenterInventoryFeedback = {
  message: Ref<string>
  error: Ref<string>
  clearFeedback: () => void
}

export function useDataCenterInventory(feedback: DataCenterInventoryFeedback) {
  const inventoryRows = shallowRef<InventoryRow[]>([])
  const inventorySummary = ref<InventorySummary>(emptyInventorySummary())
  const inventoryLoading = ref(false)
  const inventoryRebuilding = ref(false)
  const inventoryRebuildProgress = ref<InventoryCacheRebuildProgress | null>(null)
  const syncRecords = shallowRef<SyncRecord[]>([])
  const syncRecordsByScope = shallowRef<SyncRecordScopeIndex>(new Map())
  const inventoryTableTotals = computed(() => buildInventoryTableTotals(inventorySummary.value))

  async function loadInventory() {
    feedback.clearFeedback()
    inventoryLoading.value = true
    try {
      const inventory = await api.fetchInventory()
      applyInventoryPayload(inventory)
    } catch (err) {
      feedback.error.value = describeError(err)
      logger.error('inventory load failed', {
        scope: 'data-center',
        error: describeError(err),
        raw: err,
      })
    } finally {
      inventoryLoading.value = false
    }
  }

  async function refreshInventoryData() {
    try {
      const inventory = await api.fetchInventory()
      applyInventoryPayload(inventory)
    } catch (err) {
      logger.warn('inventory refresh failed', {
        scope: 'data-center',
        error: describeError(err),
        raw: err,
      })
    }
  }

  async function rebuildInventoryCache() {
    feedback.clearFeedback()
    inventoryRebuilding.value = true
    try {
      const started = await api.startInventoryCacheRebuild({ concurrency: 8 })
      inventoryRebuildProgress.value = started.progress
      const result = await waitForInventoryCacheRebuild()
      inventoryRebuildProgress.value = result
      const inventory = await api.fetchInventory()
      applyInventoryPayload(inventory)
      feedback.message.value = [
        `全库扫描完成：重建 ${formatCount(result.sync_records_rebuilt)} 条周期缓存`,
        `清理陈旧缓存 ${formatCount(result.stale_sync_records_deleted)} 条`,
        `缓存 K 线 ${formatCount(result.cached_candles_total)} 根`,
      ].join('，')
    } catch (err) {
      feedback.error.value = describeError(err)
      logger.error('inventory cache rebuild failed', {
        scope: 'data-center',
        error: describeError(err),
        raw: err,
      })
    } finally {
      inventoryRebuilding.value = false
    }
  }

  function applyInventoryPayload(inventory: { summary: InventorySummary; rows: InventoryRow[] }) {
    const records = inventoryRowsToSyncRecords(inventory.rows)
    inventorySummary.value = inventory.summary
    inventoryRows.value = inventory.rows
    syncRecords.value = records
    syncRecordsByScope.value = buildSyncRecordScopeIndex(records)
    return records
  }

  function replaceSyncRecordScopes(records: SyncRecord[], scopeKeys: Set<string>) {
    replaceSyncRecordScopesInPlace(
      syncRecordsByScope.value,
      records,
      scopeKeys,
    )
    triggerRef(syncRecordsByScope)
    syncRecords.value = records
    return records
  }

  async function waitForInventoryCacheRebuild() {
    const deadline = Date.now() + 30 * 60_000
    while (Date.now() < deadline) {
      const { progress } = await api.fetchInventoryCacheRebuildStatus()
      if (progress) {
        inventoryRebuildProgress.value = progress
        if (progress.status === 'completed') return progress
        if (progress.status === 'failed') {
          throw new Error(progress.error || progress.message || '库存缓存重建失败')
        }
      }
      await delay(1000)
    }
    throw new Error('库存缓存重建超时，请稍后刷新状态')
  }

  return {
    inventoryRows,
    inventorySummary,
    inventoryLoading,
    inventoryRebuilding,
    inventoryRebuildProgress,
    syncRecords,
    syncRecordsByScope,
    inventoryTableTotals,
    loadInventory,
    refreshInventoryData,
    rebuildInventoryCache,
    applyInventoryPayload,
    replaceSyncRecordScopes,
  }
}

function delay(ms: number) {
  return new Promise(resolve => window.setTimeout(resolve, ms))
}
