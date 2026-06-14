import { ref } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import * as marketApi from '@/api/market'
import { useDataCenterCollection } from '@/composables/useDataCenterCollection'
import type { TickCollectorStatus } from '@/types/dataCenter'

vi.mock('@/api/market', () => ({
  fetchTickCollectorStatus: vi.fn(),
  startTickCollector: vi.fn(),
  stopTickCollector: vi.fn(),
}))

const fetchTickCollectorStatusMock = vi.mocked(marketApi.fetchTickCollectorStatus)
const startTickCollectorMock = vi.mocked(marketApi.startTickCollector)
const stopTickCollectorMock = vi.mocked(marketApi.stopTickCollector)

describe('useDataCenterCollection', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('加载采集器状态时更新状态并清理反馈', async () => {
    const state = feedback()
    const collection = useDataCenterCollection(state)
    fetchTickCollectorStatusMock.mockResolvedValue(tickCollectorStatus({
      running: true,
      total_trades_received: 12,
    }))

    await collection.loadCollectionStatus()

    expect(state.clearFeedback).toHaveBeenCalledTimes(1)
    expect(fetchTickCollectorStatusMock).toHaveBeenCalledTimes(1)
    expect(collection.tickCollectorStatus.value).toMatchObject({
      running: true,
      total_trades_received: 12,
    })
    expect(collection.collectionLoading.value).toBe(false)
    expect(state.error.value).toBe('')
  })

  it('启动采集器后按最新状态刷新面板', async () => {
    const state = feedback()
    const collection = useDataCenterCollection(state)
    startTickCollectorMock.mockResolvedValue({
      message: 'started',
      status: tickCollectorStatus({ running: true, total_trades_received: 1 }),
    })
    fetchTickCollectorStatusMock.mockResolvedValue(tickCollectorStatus({
      running: true,
      total_trades_received: 9,
    }))

    await collection.startCollection()

    expect(startTickCollectorMock).toHaveBeenCalledTimes(1)
    expect(fetchTickCollectorStatusMock).toHaveBeenCalledTimes(1)
    expect(collection.tickCollectorStatus.value).toMatchObject({
      running: true,
      total_trades_received: 9,
    })
    expect(collection.collectionMutating.value).toBe(false)
  })

  it('停止采集器后写入返回状态和提示', async () => {
    const state = feedback()
    const collection = useDataCenterCollection(state)
    stopTickCollectorMock.mockResolvedValue({
      message: 'stopped',
      status: tickCollectorStatus({ running: false }),
    })

    await collection.stopCollection()

    expect(state.clearFeedback).toHaveBeenCalledTimes(1)
    expect(stopTickCollectorMock).toHaveBeenCalledTimes(1)
    expect(collection.tickCollectorStatus.value?.running).toBe(false)
    expect(state.message.value).toBe('stopped')
    expect(collection.collectionMutating.value).toBe(false)
  })
})

function feedback() {
  return {
    message: ref(''),
    error: ref(''),
    clearFeedback: vi.fn(),
  }
}

function tickCollectorStatus(overrides: Partial<TickCollectorStatus> = {}): TickCollectorStatus {
  return {
    running: false,
    active_symbols: ['BTC-USDT'],
    book_channel: 'books5',
    total_trades_received: 0,
    total_bars_written: 0,
    last_trade_ts: 0,
    errors: [],
    ...overrides,
  }
}
