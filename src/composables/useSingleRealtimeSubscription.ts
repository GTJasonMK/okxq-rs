import { useRealtimeSubscriptionGroup } from './useRealtimeSubscriptionGroup'

interface SingleRealtimeSubscriptionOptions {
  source: () => string
  eventName: string
  subscriptionKey: (id: string) => string
  subscribe: (id: string) => Promise<unknown>
  unsubscribe: (id: string) => Promise<unknown>
  handlePayload: (payload: Record<string, unknown>, currentId: string) => void
  resetOnEmptySource?: () => void
  resetOnSourceChange?: () => void
}

export function useSingleRealtimeSubscription(options: SingleRealtimeSubscriptionOptions) {
  return useRealtimeSubscriptionGroup({
    source: () => {
      const id = options.source()
      return id ? [id] : []
    },
    eventName: options.eventName,
    subscriptionKey: options.subscriptionKey,
    subscribe: options.subscribe,
    unsubscribe: options.unsubscribe,
    handlePayload(payload, currentIds) {
      options.handlePayload(payload, currentIds[0] ?? '')
    },
    resetOnEmptySource: options.resetOnEmptySource,
    resetOnSourceChange: options.resetOnSourceChange,
  })
}
