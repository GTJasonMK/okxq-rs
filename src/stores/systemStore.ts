import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { fetchHealth, fetchSystemStatus } from '@/api/system'
import type { SystemConfig } from '@/types/system'
import { describeError, logger } from '@/utils/logger'

export const useSystemStore = defineStore('system', () => {
  const connected = ref(false)
  const useSimulated = ref(true)
  const config = ref<SystemConfig | null>(null)
  const status = ref<Record<string, unknown>>({})
  const statusLoaded = ref(false)
  const loading = ref(false)

  const tradingMode = computed(() => useSimulated.value ? 'simulated' : 'live')
  const tradingModeLabel = computed(() => useSimulated.value ? '模拟盘' : '实盘')

  function applySystemStatus(data: unknown) {
    const nextStatus = isRecord(data) ? data : {}
    status.value = nextStatus
    statusLoaded.value = true

    const okx = isRecord(nextStatus.okx) ? nextStatus.okx : null
    const mode = typeof okx?.mode === 'string' ? okx.mode.toLowerCase() : ''
    if (mode === 'live') {
      useSimulated.value = false
    } else if (mode === 'simulated' || mode === 'demo') {
      useSimulated.value = true
    }
  }

  async function checkConnection(retries = 3): Promise<boolean> {
    for (let i = 0; i < retries; i++) {
      try {
        await fetchHealth()
        connected.value = true
        return true
      } catch {
        if (i < retries - 1) await new Promise(r => setTimeout(r, 1000))
      }
    }
    connected.value = false
    return false
  }

  async function loadConfig() {
    try {
      const data = await fetchSystemStatus()
      applySystemStatus(data)
    } catch (error) {
      logger.error('system status load failed', {
        scope: 'system',
        error: describeError(error),
        raw: error,
      })
    }
  }

  return {
    connected,
    useSimulated,
    config,
    status,
    statusLoaded,
    loading,
    tradingMode,
    tradingModeLabel,
    applySystemStatus,
    checkConnection,
    loadConfig,
  }
})

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}
