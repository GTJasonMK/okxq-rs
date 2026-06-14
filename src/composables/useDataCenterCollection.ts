import { ref, type Ref } from 'vue'
import * as api from '@/api/market'
import type { TickCollectorStatus } from '@/types/dataCenter'
import { describeError, logger } from '@/utils/logger'

type DataCenterCollectionFeedback = {
  message: Ref<string>
  error: Ref<string>
  clearFeedback: () => void
}

export function useDataCenterCollection(feedback: DataCenterCollectionFeedback) {
  const tickCollectorStatus = ref<TickCollectorStatus | null>(null)
  const collectionLoading = ref(false)
  const collectionMutating = ref(false)

  async function loadCollectionStatus() {
    feedback.clearFeedback()
    collectionLoading.value = true
    try {
      tickCollectorStatus.value = await api.fetchTickCollectorStatus()
    } catch (err) {
      feedback.error.value = describeError(err)
      logger.error('tick collector status load failed', {
        scope: 'data-center',
        error: describeError(err),
        raw: err,
      })
    } finally {
      collectionLoading.value = false
    }
  }

  async function startCollection() {
    feedback.clearFeedback()
    collectionMutating.value = true
    try {
      const result = await api.startTickCollector()
      feedback.message.value = result.message || '秒级采集器已启动'
      tickCollectorStatus.value = result.status
      await loadCollectionStatus()
    } catch (err) {
      feedback.error.value = describeError(err)
      logger.error('tick collector start failed', {
        scope: 'data-center',
        error: describeError(err),
        raw: err,
      })
    } finally {
      collectionMutating.value = false
    }
  }

  async function stopCollection() {
    feedback.clearFeedback()
    collectionMutating.value = true
    try {
      const result = await api.stopTickCollector()
      feedback.message.value = result.message || '秒级采集器已停止'
      tickCollectorStatus.value = result.status
    } catch (err) {
      feedback.error.value = describeError(err)
      logger.error('tick collector stop failed', {
        scope: 'data-center',
        error: describeError(err),
        raw: err,
      })
    } finally {
      collectionMutating.value = false
    }
  }

  return {
    tickCollectorStatus,
    collectionLoading,
    collectionMutating,
    loadCollectionStatus,
    startCollection,
    stopCollection,
  }
}
