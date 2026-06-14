import { onMounted, ref } from 'vue'
import * as marketApi from '@/api/market'
import * as api from '@/api/system'
import { useSystemStore } from '@/stores/systemStore'
import type { SyncRuntimeConfig, SyncRuntimeSettings } from '@/types/market'
import type {
  OkxConfig,
  OkxConfigSaveRequest,
  OkxConfigTestResult,
  OkxWebsocketDiagnostic,
} from '@/types/system'
import { describeError } from '@/utils/logger'
import { settledErrorMessage } from '@/utils/settled'

const LOAD_LABELS = ['OKX 配置', '健康检查', '系统状态', '采集性能']

export function useSettingsView() {
  const systemStore = useSystemStore()
  const okxConfig = ref<OkxConfig | null>(null)
  const syncRuntimeConfig = ref<SyncRuntimeConfig | null>(null)
  const health = ref<unknown>(null)
  const error = ref<string | null>(null)
  const message = ref<string | null>(null)
  const testDetail = ref<string | null>(null)
  const savingOkx = ref(false)
  const testingOkx = ref(false)
  const savingSyncRuntime = ref(false)

  async function loadData() {
    error.value = null
    testDetail.value = null
    const tasks = await Promise.allSettled([
      api.fetchOkxConfig(),
      api.fetchHealth(),
      api.fetchSystemStatus(),
      marketApi.fetchSyncRuntimeConfig(),
    ])
    if (tasks[0].status === 'fulfilled') okxConfig.value = tasks[0].value
    if (tasks[1].status === 'fulfilled') health.value = tasks[1].value
    if (tasks[2].status === 'fulfilled') systemStore.applySystemStatus(tasks[2].value)
    if (tasks[3].status === 'fulfilled') syncRuntimeConfig.value = tasks[3].value
    error.value = settledErrorMessage(tasks, LOAD_LABELS)
  }

  async function saveOkxConfig(config: OkxConfigSaveRequest) {
    error.value = null
    message.value = null
    testDetail.value = null
    savingOkx.value = true
    try {
      await api.saveOkxConfig(config)
      okxConfig.value = await api.fetchOkxConfig()
      systemStore.applySystemStatus(await api.fetchSystemStatus())
      message.value = 'OKX 配置已保存'
    } catch (e) {
      error.value = describeError(e)
    } finally {
      savingOkx.value = false
    }
  }

  async function saveSyncRuntimeConfig(settings: SyncRuntimeSettings) {
    error.value = null
    message.value = null
    testDetail.value = null
    savingSyncRuntime.value = true
    try {
      syncRuntimeConfig.value = await marketApi.updateSyncRuntimeConfig(settings)
      message.value = '数据采集性能参数已保存'
    } catch (e) {
      error.value = describeError(e)
    } finally {
      savingSyncRuntime.value = false
    }
  }

  async function testOkxConfig(config: OkxConfigSaveRequest) {
    error.value = null
    message.value = null
    testDetail.value = null
    testingOkx.value = true
    try {
      const result = await api.testOkxConfig(config)
      testDetail.value = formatOkxTestDetail(result)
      if (result.success) {
        message.value = result.message
      } else {
        error.value = result.message || 'OKX 配置测试失败'
      }
    } catch (e) {
      error.value = describeError(e)
    } finally {
      testingOkx.value = false
    }
  }

  async function refreshData() {
    error.value = null
    message.value = null
    testDetail.value = null
    await loadData()
    if (!error.value) message.value = '系统状态已刷新'
  }

  onMounted(() => {
    void loadData()
  })

  return {
    systemStore,
    okxConfig,
    syncRuntimeConfig,
    health,
    error,
    message,
    testDetail,
    savingOkx,
    testingOkx,
    savingSyncRuntime,
    saveOkxConfig,
    saveSyncRuntimeConfig,
    testOkxConfig,
    refreshData,
  }
}

function formatOkxTestDetail(result: OkxConfigTestResult): string | null {
  const parts: string[] = []
  if (typeof result.data?.latency_ms === 'number') {
    const restStatus = result.data.rest_success === false ? '失败' : '通过'
    parts.push(`REST ${restStatus} ${result.data.latency_ms}ms`)
  } else if (result.data?.private_api === false) {
    parts.push('REST 未测试')
  }
  const proxy = result.data?.proxy?.trim()
  parts.push(`代理 ${proxy ? proxy : '直连'}`)
  const websocket = result.data?.websocket || {}
  const publicWs = websocket.public
  const businessWs = websocket.business
  if (publicWs) parts.push(formatWsDiagnostic('public WS', publicWs))
  if (businessWs) parts.push(formatWsDiagnostic('business WS', businessWs))
  return parts.length > 0 ? parts.join('；') : null
}

function formatWsDiagnostic(label: string, item: OkxWebsocketDiagnostic): string {
  if (item.success) {
    const latency = typeof item.latency_ms === 'number' ? ` ${item.latency_ms}ms` : ''
    return `${label} 通过${latency}`
  }
  const reason = item.error?.trim()
  return `${label} 失败${reason ? `：${reason}` : ''}`
}
