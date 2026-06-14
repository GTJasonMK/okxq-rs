import { onMounted, ref } from 'vue'
import * as marketApi from '@/api/market'
import * as api from '@/api/scanner'
import { useScannerStore } from '@/stores/scannerStore'
import type { ScannerProfile } from '@/types'
import { describeError } from '@/utils/logger'

interface CreateScannerProfileInput {
  name: string
  conditions: string[]
}

export function useScannerView() {
  const store = useScannerStore()
  const error = ref<string | null>(null)
  const message = ref<string | null>(null)

  async function loadData() {
    store.loading = true
    error.value = null
    try {
      const [profiles, conditions, results] = await Promise.all([
        api.fetchProfiles(),
        api.fetchConditions(),
        api.fetchResults(),
      ])
      store.profiles = profiles as never
      store.conditions = conditions as never
      store.results = results as never
    } catch (e) {
      error.value = describeError(e)
    } finally {
      store.loading = false
    }
  }

  async function createProfile(data: CreateScannerProfileInput) {
    store.loading = true
    error.value = null
    message.value = null
    try {
      const scope = await marketApi.fetchDefaultWatchScope()
      if (!scope) throw new Error('请先在数据中心添加关注币种并启用数据目标')
      await api.createProfile({
        ...data,
        inst_type: scope.inst_type,
        timeframe: '1H',
        symbols: [],
        logic: 'and',
      })
      message.value = '扫描配置已创建'
      await loadData()
    } catch (e) {
      error.value = describeError(e)
    } finally {
      store.loading = false
    }
  }

  async function runProfile(profile: ScannerProfile) {
    store.loading = true
    error.value = null
    message.value = null
    try {
      const response = await api.runProfileScan(profile.id)
      store.results = response.results as never
      message.value = `扫描完成，命中 ${response.results.length} 个品种`
    } catch (e) {
      error.value = describeError(e)
    } finally {
      store.loading = false
    }
  }

  async function deleteProfile(profile: ScannerProfile) {
    store.loading = true
    error.value = null
    message.value = null
    try {
      await api.deleteProfile(profile.id)
      message.value = '扫描配置已删除'
      await loadData()
    } catch (e) {
      error.value = describeError(e)
    } finally {
      store.loading = false
    }
  }

  onMounted(() => {
    void loadData()
  })

  return {
    store,
    error,
    message,
    loadData,
    createProfile,
    runProfile,
    deleteProfile,
  }
}
