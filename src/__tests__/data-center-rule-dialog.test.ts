import { computed, ref } from 'vue'
import { describe, expect, it, vi } from 'vitest'
import { useDataCenterRuleDialog } from '@/composables/useDataCenterRuleDialog'
import type { WatchedSymbol } from '@/types'
import type { GuardianPlan, WatchedRow } from '@/types/dataCenter'

describe('useDataCenterRuleDialog', () => {
  it('打开已有关注规则时归一化输入并加载该规则表单', () => {
    const clearFeedback = vi.fn()
    const watchedSymbols = ref<WatchedSymbol[]>([
      watchedSymbol({
        symbol: 'BTC-USDT',
        sync_spot: false,
        sync_swap: true,
        sync_days: 45,
        sync_plans: [{
          timeframe: '1H',
          enabled: true,
          bootstrap_days: 45,
          archive_mode: 'rolling',
        }],
      }),
    ])
    const dialog = useDataCenterRuleDialog({
      watchedSymbols,
      watchedRows: computed(() => []),
      guardianPlans: ref<GuardianPlan[]>([]),
      adding: ref(false),
      clearFeedback,
    })

    dialog.newSymbol.value = ' btc '

    expect(dialog.openRuleDialog()).toBe(true)
    expect(clearFeedback).toHaveBeenCalledTimes(1)
    expect(dialog.ruleDialogOpen.value).toBe(true)
    expect(dialog.pendingRuleSymbol.value).toBe('BTC-USDT')
    expect(dialog.newSyncSpot.value).toBe(false)
    expect(dialog.newSyncSwap.value).toBe(true)
    expect(dialog.newSyncDays.value).toBe(45)
    expect(enabledTimeframes(dialog.newSyncPlans.value)).toEqual(['1H'])
  })

  it('接管库内标的时从库存周期生成可提交规则', () => {
    const dialog = useDataCenterRuleDialog({
      watchedSymbols: ref<WatchedSymbol[]>([]),
      watchedRows: computed(() => [
        {
          ...watchedSymbol({
            symbol: 'ETH-USDT',
            base_ccy: 'ETH',
            sync_spot: false,
            sync_swap: true,
          }),
          inventory_only: true,
          inventory_timeframes: ['1m', '3m'],
          jobs: [],
          jobSummary: emptyJobSummary(),
        } as WatchedRow,
      ]),
      guardianPlans: ref<GuardianPlan[]>([]),
      adding: ref(false),
    })

    dialog.newSymbol.value = 'eth-usdt'

    expect(dialog.openRuleDialog()).toBe(true)
    expect(dialog.pendingRuleSymbol.value).toBe('ETH-USDT')
    expect(dialog.newSyncSpot.value).toBe(false)
    expect(dialog.newSyncSwap.value).toBe(true)
    expect(enabledTimeframes(dialog.newSyncPlans.value)).toEqual(['1m', '3m'])
    expect(dialog.canSubmitRuleDialog.value).toBe(true)
  })

  it('保存后关闭弹窗并恢复 Guardian 默认规则', () => {
    const dialog = useDataCenterRuleDialog({
      watchedSymbols: ref<WatchedSymbol[]>([]),
      watchedRows: computed(() => []),
      guardianPlans: ref<GuardianPlan[]>([
        {
          timeframe: '1H',
          enabled: true,
          bootstrap_days: 60,
          archive_mode: 'rolling',
        },
      ]),
      adding: ref(false),
    })

    dialog.newSymbol.value = 'BTC-USDT'
    dialog.pendingRuleSymbol.value = 'BTC-USDT'
    dialog.ruleDialogOpen.value = true
    dialog.newSyncSpot.value = false

    dialog.resetAfterSaved()

    expect(dialog.newSymbol.value).toBe('')
    expect(dialog.pendingRuleSymbol.value).toBe('')
    expect(dialog.ruleDialogOpen.value).toBe(false)
    expect(dialog.newSyncSpot.value).toBe(true)
    expect(dialog.newSyncSwap.value).toBe(true)
    expect(enabledTimeframes(dialog.newSyncPlans.value)).toContain('1H')
    expect(dialog.newSyncPlans.value.find(plan => plan.timeframe === '1H')).toMatchObject({
      enabled: true,
      bootstrap_days: 90,
      archive_mode: 'rolling',
    })
  })
})

function enabledTimeframes(plans: WatchedSymbol['sync_plans']) {
  return (plans ?? []).filter(plan => plan.enabled).map(plan => plan.timeframe)
}

function emptyJobSummary() {
  return {
    total: 0,
    queued: 0,
    running: 0,
    completed: 0,
    failed: 0,
    cancelled: 0,
    active: 0,
    progress: 0,
    statusLabel: '无任务',
    phaseLabel: '',
    primaryText: '',
    secondaryText: '',
    taskText: '',
    segments: [],
    fetched: 0,
    targetFetch: 0,
    saved: 0,
    targetSave: 0,
    derived: 0,
    targetDerive: 0,
    batches: 0,
    targetBatches: 0,
    apiCalls: 0,
  }
}

function watchedSymbol(overrides: Partial<WatchedSymbol> = {}): WatchedSymbol {
  return {
    symbol: 'BTC-USDT',
    base_ccy: 'BTC',
    spot_inst_id: 'BTC-USDT',
    swap_inst_id: 'BTC-USDT-SWAP',
    sync_spot: true,
    sync_swap: true,
    archive_all_history: false,
    sync_days: 90,
    sync_plans: [],
    created_at: '2026-01-01T00:00:00.000Z',
    updated_at: '2026-01-01T00:00:00.000Z',
    ...overrides,
  }
}
