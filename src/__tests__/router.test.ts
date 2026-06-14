import { describe, expect, it } from 'vitest'
import router from '@/router'

describe('核心路由入口', () => {
  it('进入数据中心并保留查询参数', async () => {
    await router.push('/data-center?symbol=BTC-USDT&tab=watchlist')
    await router.isReady()

    expect(router.currentRoute.value.path).toBe('/data-center')
    expect(router.currentRoute.value.query).toMatchObject({
      symbol: 'BTC-USDT',
      tab: 'watchlist',
    })
    expect(document.title).toBe('数据中心 - OKXQ')
  })

  it('进入交易页并保留查看模式', async () => {
    await router.push('/trading?mode=live')
    await router.isReady()

    expect(router.currentRoute.value.path).toBe('/trading')
    expect(router.currentRoute.value.query).toMatchObject({ mode: 'live' })
    expect(document.title).toBe('交易 - OKXQ')
  })
})
