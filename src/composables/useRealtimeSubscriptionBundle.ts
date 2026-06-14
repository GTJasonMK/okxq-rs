import { onScopeDispose, ref, watch } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { describeError } from '@/utils/logger'
import { useRealtimeManager } from './useRealtimeManager'

type ReleaseFn = () => Promise<void>

export interface RealtimeSubscriptionSpec {
  key: string
  subscribe: () => Promise<unknown>
  unsubscribe: () => Promise<unknown>
}

interface RealtimeSubscriptionListener {
  eventName: string
  handlePayload: (payload: Record<string, unknown>, currentIds: readonly string[]) => void
}

interface RealtimeSubscriptionBundleOptions {
  source: () => readonly string[]
  listeners: RealtimeSubscriptionListener[]
  subscriptions: (currentIds: readonly string[]) => RealtimeSubscriptionSpec[]
  beforeSetup?: (currentIds: readonly string[]) => void
  resetOnEmptySource?: () => void
  resetOnSourceChange?: () => void
  clearErrorOnEmptySource?: boolean
}

export function useRealtimeSubscriptionBundle(options: RealtimeSubscriptionBundleOptions) {
  const { acquire } = useRealtimeManager()
  const connected = ref(false)
  const error = ref<string | null>(null)
  const cleanups: ReleaseFn[] = []
  const unlisteners: Array<() => void> = []
  let setupGeneration = 0
  let disposed = false

  function currentIds() {
    return options.source().filter(Boolean)
  }

  function isCurrentSetup(generation: number, ids: readonly string[]) {
    return !disposed && generation === setupGeneration && sameIds(currentIds(), ids)
  }

  async function releaseCleanups() {
    const releases = cleanups.splice(0)
    for (const release of releases) await release()
  }

  async function releaseLocalCleanups(releases: ReleaseFn[]) {
    while (releases.length > 0) {
      const release = releases.pop()
      if (release) await release()
    }
  }

  async function ensureListeners() {
    if (unlisteners.length > 0) return
    for (const listener of options.listeners) {
      const unlisten = await listen<Record<string, unknown>>(listener.eventName, (event) => {
        listener.handlePayload(event.payload, currentIds())
      })
      if (disposed) {
        unlisten()
        return
      }
      unlisteners.push(unlisten)
    }
  }

  async function setup(generation: number) {
    const ids = currentIds()
    if (ids.length === 0) {
      if (generation === setupGeneration) {
        options.resetOnEmptySource?.()
        connected.value = false
        if (options.clearErrorOnEmptySource) error.value = null
      }
      return
    }

    options.beforeSetup?.(ids)
    await ensureListeners()
    if (!isCurrentSetup(generation, ids)) return

    const releases: ReleaseFn[] = []
    for (const spec of options.subscriptions(ids)) {
      if (!isCurrentSetup(generation, ids)) {
        await releaseLocalCleanups(releases)
        return
      }
      const release = await acquire(spec.key, spec.subscribe, spec.unsubscribe)
      if (!isCurrentSetup(generation, ids)) {
        await release()
        await releaseLocalCleanups(releases)
        return
      }
      releases.push(release)
    }

    cleanups.push(...releases)
    connected.value = true
    error.value = null
  }

  function handleSetupError(generation: number, setupError: unknown) {
    if (disposed || generation !== setupGeneration) return
    connected.value = false
    error.value = describeError(setupError)
  }

  function startSetup() {
    const generation = ++setupGeneration
    setup(generation).catch(setupError => { handleSetupError(generation, setupError) })
  }

  startSetup()

  watch(options.source, async () => {
    const generation = ++setupGeneration
    try {
      options.resetOnSourceChange?.()
      connected.value = false
      await releaseCleanups()
      await setup(generation)
    } catch (setupError) {
      handleSetupError(generation, setupError)
    }
  })

  onScopeDispose(async () => {
    disposed = true
    setupGeneration += 1
    await releaseCleanups()
    while (unlisteners.length > 0) {
      unlisteners.pop()?.()
    }
  })

  return { connected, error }
}

function sameIds(left: readonly string[], right: readonly string[]) {
  if (left.length !== right.length) return false
  return left.every((id, index) => id === right[index])
}
