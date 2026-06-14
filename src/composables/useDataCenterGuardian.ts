import { computed, ref, type Ref } from 'vue'
import * as api from '@/api/market'
import type { GuardianConfig, GuardianPlan, GuardianStatus } from '@/types/dataCenter'
import { describeError, logger } from '@/utils/logger'
import {
  guardianCurrentTargetText,
  guardianErrorMessages,
} from '@/utils/dataCenter'

type DataCenterGuardianFeedback = {
  error: Ref<string>
  clearFeedback: () => void
}

export function useDataCenterGuardian(feedback: DataCenterGuardianFeedback) {
  const guardianPlans = ref<GuardianPlan[]>([])
  const guardianStatus = ref<GuardianStatus | null>(null)
  const guardianStatusLoading = ref(false)
  const guardianQueuePreview = computed(() => guardianStatus.value?.backfill_queue_preview ?? [])
  const guardianErrors = computed(() => guardianErrorMessages(guardianStatus.value))
  const guardianCurrentTarget = computed(() => guardianCurrentTargetText(guardianStatus.value))
  let guardianStatusSequence = 0

  async function loadGuardianData() {
    feedback.clearFeedback()
    guardianStatusLoading.value = true
    try {
      const [status, config] = await Promise.all([
        api.fetchGuardianStatus(),
        api.fetchGuardianConfig(),
      ])
      guardianStatus.value = status
      applyGuardianConfig(config)
    } catch (err) {
      feedback.error.value = describeError(err)
      logger.error('guardian status load failed', {
        scope: 'data-center',
        error: describeError(err),
        raw: err,
      })
    } finally {
      guardianStatusLoading.value = false
    }
  }

  async function refreshGuardianStatus() {
    const sequence = ++guardianStatusSequence
    try {
      const status = await api.fetchGuardianStatus()
      if (sequence !== guardianStatusSequence) return
      guardianStatus.value = status
    } catch (err) {
      logger.warn('guardian status refresh failed', {
        scope: 'data-center',
        error: describeError(err),
        raw: err,
      })
    }
  }

  function applyGuardianConfig(config: GuardianConfig, fallbackPlans = guardianPlans.value) {
    const plans = Array.isArray(config.settings?.plans) ? config.settings.plans : fallbackPlans
    guardianPlans.value = plans
    return plans
  }

  return {
    guardianPlans,
    guardianStatus,
    guardianStatusLoading,
    guardianQueuePreview,
    guardianErrors,
    guardianCurrentTarget,
    loadGuardianData,
    refreshGuardianStatus,
    applyGuardianConfig,
  }
}
