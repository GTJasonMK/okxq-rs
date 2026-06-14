interface SubEntry { count: number; cleanup: (() => Promise<unknown>) | null }
const subs = new Map<string, SubEntry>()

export function useRealtimeManager() {
  async function acquire(
    key: string,
    sub: () => Promise<unknown>,
    unsub: () => Promise<unknown>,
  ): Promise<() => Promise<void>> {
    let entry = subs.get(key)
    if (!entry) { entry = { count: 0, cleanup: null }; subs.set(key, entry) }
    entry.count++
    if (entry.count === 1) {
      try {
        await sub()
        entry.cleanup = unsub
      } catch (error) {
        entry.count--
        if (entry.count <= 0) subs.delete(key)
        throw error
      }
    }
    let disposed = false
    return async () => {
      if (disposed) return
      disposed = true
      const cur = subs.get(key)
      if (!cur) return
      cur.count--
      if (cur.count <= 0) { await cur.cleanup?.(); subs.delete(key) }
    }
  }
  return { acquire }
}
