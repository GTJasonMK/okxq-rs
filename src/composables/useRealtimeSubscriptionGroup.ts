import { useRealtimeSubscriptionBundle } from './useRealtimeSubscriptionBundle'

interface RealtimeSubscriptionGroupOptions {
  source: () => readonly string[]
  eventName: string
  subscriptionKey: (id: string) => string
  subscribe: (id: string) => Promise<unknown>
  unsubscribe: (id: string) => Promise<unknown>
  handlePayload: (payload: Record<string, unknown>, currentIds: readonly string[]) => void
  resetOnEmptySource?: () => void
  resetOnSourceChange?: () => void
  clearErrorOnEmptySource?: boolean
}

export function useRealtimeSubscriptionGroup(options: RealtimeSubscriptionGroupOptions) {
  return useRealtimeSubscriptionBundle({
    source: () => Array.from(new Set(options.source().filter(Boolean))),
    listeners: [{
      eventName: options.eventName,
      handlePayload: options.handlePayload,
    }],
    subscriptions: currentIds => currentIds.map(id => ({
      key: options.subscriptionKey(id),
      subscribe: () => options.subscribe(id),
      unsubscribe: () => options.unsubscribe(id),
    })),
    resetOnEmptySource: options.resetOnEmptySource,
    resetOnSourceChange: options.resetOnSourceChange,
    clearErrorOnEmptySource: options.clearErrorOnEmptySource,
  })
}
