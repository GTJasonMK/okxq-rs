import { computed, ref, watch } from 'vue'
import type { LocationQuery, LocationQueryRaw } from 'vue-router'
import type { DataCenterTab, DataCenterTabItem } from '@/types/dataCenter'
import { describeError, logger } from '@/utils/logger'
import {
  DATA_CENTER_TAB_KEY,
  DEFAULT_DATA_CENTER_TAB,
  dataCenterTabDescription,
  normalizeDataCenterTab,
} from '@/utils/dataCenter'

type DataCenterRoute = {
  path: string
  query: LocationQuery
}

type DataCenterRouter = {
  push: (location: { path: string; query: LocationQueryRaw }) => Promise<unknown> | unknown
  replace: (location: { path: string; query: LocationQueryRaw }) => Promise<unknown> | unknown
}

type DataCenterTabsOptions = {
  tabs: DataCenterTabItem[]
  route: DataCenterRoute
  router: DataCenterRouter
  loadTabData: (tab: DataCenterTab) => Promise<void> | void
}

export function useDataCenterTabs(options: DataCenterTabsOptions) {
  const activeTab = ref<DataCenterTab>(resolvePreferredTab())
  const activeTabHint = computed(() => dataCenterTabDescription(options.tabs, activeTab.value))

  function readStoredTab() {
    try {
      return normalizeDataCenterTab(window.localStorage.getItem(DATA_CENTER_TAB_KEY))
    } catch (err) {
      logger.warn('data center tab preference read failed', {
        scope: 'data-center',
        error: describeError(err),
        raw: err,
      })
      return ''
    }
  }

  function persistActiveTab(tab: DataCenterTab) {
    try {
      window.localStorage.setItem(DATA_CENTER_TAB_KEY, tab)
    } catch (err) {
      logger.warn('data center tab preference save failed', {
        scope: 'data-center',
        tab,
        error: describeError(err),
        raw: err,
      })
    }
  }

  function resolvePreferredTab(): DataCenterTab {
    return normalizeDataCenterTab(options.route.query.tab) || readStoredTab() || DEFAULT_DATA_CENTER_TAB
  }

  async function setActiveTab(tab: DataCenterTab) {
    await syncRouteTab(tab, false)
  }

  async function syncRouteTab(tab: DataCenterTab, replace = true) {
    const normalized = normalizeDataCenterTab(tab) || DEFAULT_DATA_CENTER_TAB
    activeTab.value = normalized
    persistActiveTab(normalized)
    await options.loadTabData(normalized)
    if (normalizeDataCenterTab(options.route.query.tab) === normalized) return
    const nextLocation = {
      path: options.route.path,
      query: { ...options.route.query, tab: normalized },
    }
    try {
      if (replace) await options.router.replace(nextLocation)
      else await options.router.push(nextLocation)
    } catch (err) {
      logger.warn('data center route tab sync failed', {
        scope: 'data-center',
        tab: normalized,
        error: describeError(err),
        raw: err,
      })
    }
  }

  watch(() => options.route.query.tab, (value) => {
    const normalized = normalizeDataCenterTab(value)
    if (normalized && normalized !== activeTab.value) {
      activeTab.value = normalized
      persistActiveTab(normalized)
      void options.loadTabData(normalized)
    }
  })

  return {
    activeTab,
    activeTabHint,
    resolvePreferredTab,
    setActiveTab,
    syncRouteTab,
  }
}
