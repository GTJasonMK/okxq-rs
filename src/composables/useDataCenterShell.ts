import { onMounted, onUnmounted, type ComputedRef, type Ref } from 'vue'
import type { LocationQuery, LocationQueryRaw } from 'vue-router'
import type { SyncJob } from '@/types'
import type { DataCenterTab, GuardianStatus } from '@/types/dataCenter'
import { normalizeInputSymbol } from '@/utils/dataCenter'

type DataCenterRoute = {
  query: LocationQuery
}

type DataCenterRouter = {
  push: (location: { path: string; query: LocationQueryRaw }) => Promise<unknown> | unknown
}

type DataCenterShellOptions = {
  route: DataCenterRoute
  router: DataCenterRouter
  symbolInput: Ref<string>
  activeTab: Ref<DataCenterTab>
  activeJobs: ComputedRef<SyncJob[]>
  guardianStatus: Ref<GuardianStatus | null>
  resolvePreferredTab: () => DataCenterTab
  syncRouteTab: (tab: DataCenterTab, replace?: boolean) => Promise<void> | void
  hasPendingSyncJobObserve: () => boolean
  shouldRefreshSyncJobSource?: () => boolean
  refreshSyncProgressData: () => Promise<void>
  refreshSyncJobProgressData?: () => Promise<void>
  refreshInventoryData: () => Promise<void>
  refreshGuardianStatus: () => Promise<void>
}

export function useDataCenterShell(options: DataCenterShellOptions) {
  let pollTimer = 0
  const inflightPollTabs = new Set<DataCenterTab>()

  function openMarket(symbol: string) {
    void options.router.push({ path: '/market', query: { symbol } })
  }

  function applyRouteSymbol() {
    const symbol = normalizeInputSymbol(String(options.route.query.symbol || ''))
    if (symbol) options.symbolInput.value = symbol
  }

  function pollActiveTabData() {
    const tab = options.activeTab.value
    if (inflightPollTabs.has(tab)) return
    const shouldRefreshJobs = options.shouldRefreshSyncJobSource
      ? options.shouldRefreshSyncJobSource()
      : options.activeJobs.value.length > 0 || options.hasPendingSyncJobObserve()
    if (tab === 'watchlist' && shouldRefreshJobs) {
      runPollRefresh(tab, refreshJobProgressThenSource(options.refreshSyncProgressData))
      return
    }
    if (tab === 'inventory' && shouldRefreshJobs) {
      runPollRefresh(tab, refreshJobProgressThenSource(options.refreshInventoryData))
      return
    }
    if (tab === 'guardian' && (options.guardianStatus.value?.active || shouldRefreshJobs)) {
      runPollRefresh(tab, options.refreshGuardianStatus)
    }
  }

  function runPollRefresh(tab: DataCenterTab, refresh: () => Promise<void>) {
    inflightPollTabs.add(tab)
    let request: Promise<void>
    try {
      request = refresh()
    } catch (err) {
      inflightPollTabs.delete(tab)
      throw err
    }
    void request.finally(() => {
      inflightPollTabs.delete(tab)
    })
  }

  function refreshJobProgressThenSource(refreshSource: () => Promise<void>) {
    if (!options.refreshSyncJobProgressData) return refreshSource
    return async () => {
      await options.refreshSyncJobProgressData!()
      if (shouldContinueJobProgressPolling()) return
      await refreshSource()
    }
  }

  function shouldContinueJobProgressPolling() {
    return options.shouldRefreshSyncJobSource
      ? options.shouldRefreshSyncJobSource()
      : options.activeJobs.value.length > 0 || options.hasPendingSyncJobObserve()
  }

  onMounted(() => {
    applyRouteSymbol()
    void options.syncRouteTab(options.resolvePreferredTab(), true)
    pollTimer = window.setInterval(pollActiveTabData, 3000)
  })

  onUnmounted(() => {
    if (pollTimer) window.clearInterval(pollTimer)
    inflightPollTabs.clear()
  })

  return {
    openMarket,
    applyRouteSymbol,
    pollActiveTabData,
  }
}
