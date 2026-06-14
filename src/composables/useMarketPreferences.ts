import { fetchPreference, updatePreferences } from '@/api/system'
import type { MarketSettings } from '@/types/marketView'
import { describeError, logger } from '@/utils/logger'
import {
  marketSettingsPayload,
  normalizeMarketSettings,
} from '@/utils/marketView'

const MARKET_SETTINGS_KEY = 'market_settings'
const MARKET_SETTINGS_STORAGE_KEY = 'okxq.market.settings'

type MarketPreferenceOptions = {
  apply: (settings: MarketSettings) => void
  current: () => MarketSettings
}

export function useMarketPreferences(options: MarketPreferenceOptions) {
  let ready = false
  let saveTimer = 0

  async function load() {
    const stored = readStoredMarketSettings()
    options.apply(stored)
    try {
      const remote = await fetchPreference<unknown>(MARKET_SETTINGS_KEY)
      const normalized = normalizeMarketSettings(remote)
      if (Object.keys(normalized).length > 0) {
        options.apply(normalized)
        persistStoredMarketSettings(normalized)
      }
    } catch (e) {
      logger.warn('market preferences load failed', {
        scope: 'market',
        error: describeError(e),
        raw: e,
      })
    } finally {
      ready = true
    }
  }

  function scheduleSave() {
    if (!ready) return
    if (saveTimer) window.clearTimeout(saveTimer)
    saveTimer = window.setTimeout(() => {
      void save()
    }, 500)
  }

  async function save() {
    const payload = marketSettingsPayload(options.current())
    persistStoredMarketSettings(payload)
    try {
      await updatePreferences({ [MARKET_SETTINGS_KEY]: payload })
    } catch (e) {
      logger.warn('market preferences save failed', {
        scope: 'market',
        error: describeError(e),
        raw: e,
      })
    }
  }

  function flush() {
    if (!saveTimer) return
    window.clearTimeout(saveTimer)
    saveTimer = 0
    void save()
  }

  return {
    loadMarketPreferences: load,
    scheduleSaveMarketPreferences: scheduleSave,
    flushMarketPreferences: flush,
  }
}

function readStoredMarketSettings(): MarketSettings {
  try {
    const raw = window.localStorage.getItem(MARKET_SETTINGS_STORAGE_KEY)
    if (!raw) return {}
    return normalizeMarketSettings(JSON.parse(raw))
  } catch (e) {
    logger.warn('market preferences local read failed', {
      scope: 'market',
      error: describeError(e),
      raw: e,
    })
    return {}
  }
}

function persistStoredMarketSettings(settings: MarketSettings) {
  try {
    window.localStorage.setItem(MARKET_SETTINGS_STORAGE_KEY, JSON.stringify(settings))
  } catch (e) {
    logger.warn('market preferences local save failed', {
      scope: 'market',
      error: describeError(e),
      raw: e,
    })
  }
}
