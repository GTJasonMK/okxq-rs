import { describe, expect, it, vi } from 'vitest'
import { useRealtimeManager } from '@/composables/useRealtimeManager'

describe('useRealtimeManager', () => {
  it('同一个 key 只订阅一次，全部释放后才退订', async () => {
    const { acquire } = useRealtimeManager()
    const key = `test:${crypto.randomUUID()}`
    const subscribe = vi.fn().mockResolvedValue(undefined)
    const unsubscribe = vi.fn().mockResolvedValue(undefined)

    const releaseA = await acquire(key, subscribe, unsubscribe)
    const releaseB = await acquire(key, subscribe, unsubscribe)

    expect(subscribe).toHaveBeenCalledTimes(1)
    expect(unsubscribe).not.toHaveBeenCalled()

    await releaseA()
    expect(unsubscribe).not.toHaveBeenCalled()

    await releaseB()
    expect(unsubscribe).toHaveBeenCalledTimes(1)
  })

  it('订阅失败时回滚引用计数，下一次 acquire 可以重新订阅', async () => {
    const { acquire } = useRealtimeManager()
    const key = `test:${crypto.randomUUID()}`
    const subscribe = vi.fn()
      .mockRejectedValueOnce(new Error('temporary ws failure'))
      .mockResolvedValueOnce(undefined)
    const unsubscribe = vi.fn().mockResolvedValue(undefined)

    await expect(acquire(key, subscribe, unsubscribe)).rejects.toThrow('temporary ws failure')

    const release = await acquire(key, subscribe, unsubscribe)
    await release()

    expect(subscribe).toHaveBeenCalledTimes(2)
    expect(unsubscribe).toHaveBeenCalledTimes(1)
  })

  it('release 重复调用只退订一次', async () => {
    const { acquire } = useRealtimeManager()
    const key = `test:${crypto.randomUUID()}`
    const subscribe = vi.fn().mockResolvedValue(undefined)
    const unsubscribe = vi.fn().mockResolvedValue(undefined)

    const release = await acquire(key, subscribe, unsubscribe)
    await release()
    await release()

    expect(subscribe).toHaveBeenCalledTimes(1)
    expect(unsubscribe).toHaveBeenCalledTimes(1)
  })
})
