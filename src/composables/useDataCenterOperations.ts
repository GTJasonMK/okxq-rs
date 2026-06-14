import { ref, type Ref } from 'vue'
import * as api from '@/api/market'
import type { SyncJob } from '@/types'
import type { DataCenterTab, InventoryGapRepairPayload } from '@/types/dataCenter'
import { describeError, logger } from '@/utils/logger'
import {
  formatCount,
  gapRepairMethodLabel,
  hasValidInventoryGapRange,
  inventoryGapKey,
  isValidTimestamp,
} from '@/utils/dataCenter'

type DataCenterOperationsOptions = {
  activeTab: Ref<DataCenterTab>
  message: Ref<string>
  error: Ref<string>
  clearFeedback: () => void
  refreshActiveGapRepairSource: () => Promise<void>
  loadPageData: () => Promise<void>
  loadGuardianData: () => Promise<void>
  trackSubmittedJobs: (jobs: SyncJob[]) => void
}

export function useDataCenterOperations(options: DataCenterOperationsOptions) {
  const guardianRunning = ref(false)
  const gapRepairingKey = ref('')

  async function repairInventoryGap(payload: InventoryGapRepairPayload) {
    options.clearFeedback()
    const key = inventoryGapKey(payload.inst_id, payload.inst_type, payload.timeframe)
    if (!hasValidInventoryGapRange(payload)) {
      options.error.value = `${payload.inst_id} ${payload.timeframe} 缺少有效本地时间范围，无法精确补齐`
      return
    }
    gapRepairingKey.value = key
    try {
      const plan = await api.fetchMarketGapPlan({
        inst_id: payload.inst_id,
        inst_type: payload.inst_type,
        timeframe: payload.timeframe,
        start_ts: payload.start_ts,
        end_ts: payload.end_ts,
        limit: 100,
      })
      if (plan.missing_candles <= 0) {
        options.message.value = `${payload.inst_id} ${payload.timeframe} 当前范围无缺失 K 线`
        await options.refreshActiveGapRepairSource()
        return
      }
      const startTs = isValidTimestamp(plan.range.start_ts) ? plan.range.start_ts : payload.start_ts
      const endTs = isValidTimestamp(plan.range.end_ts) ? plan.range.end_ts : payload.end_ts
      const job = await api.startGapRepairJob({
        inst_id: payload.inst_id,
        inst_type: payload.inst_type,
        timeframe: payload.timeframe,
        start_ts: startTs,
        end_ts: endTs,
        method: 'auto',
      })
      options.message.value = [
        `${payload.inst_id} ${payload.timeframe} 已提交精确补齐`,
        `缺失 ${formatCount(plan.missing_candles)} 根`,
        gapRepairMethodLabel(plan.methods),
      ].filter(Boolean).join('，')
      options.trackSubmittedJobs([job])
      await options.refreshActiveGapRepairSource()
    } catch (err) {
      options.error.value = describeError(err)
      logger.error('inventory gap repair failed', {
        scope: 'data-center',
        inst_id: payload.inst_id,
        inst_type: payload.inst_type,
        timeframe: payload.timeframe,
        start_ts: payload.start_ts,
        end_ts: payload.end_ts,
        error: describeError(err),
        raw: err,
      })
    } finally {
      gapRepairingKey.value = ''
    }
  }

  async function runGuardian() {
    options.clearFeedback()
    guardianRunning.value = true
    try {
      const result = await api.runDataGuardianNow()
      options.message.value = '已按当前关注清单和每币种采集规则提交补齐扫描'
      options.trackSubmittedJobs(result.last_sync_results ?? [])
      if (options.activeTab.value === 'guardian') await options.loadGuardianData()
      else await options.loadPageData()
    } catch (err) {
      options.error.value = describeError(err)
      logger.error('data guardian manual run failed', {
        scope: 'data-center',
        error: describeError(err),
        raw: err,
      })
    } finally {
      guardianRunning.value = false
    }
  }

  return {
    guardianRunning,
    gapRepairingKey,
    repairInventoryGap,
    runGuardian,
  }
}
