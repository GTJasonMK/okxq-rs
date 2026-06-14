import { computed, onMounted, reactive, ref, watch } from 'vue'
import {
  fetchContractAccountConfig,
  fetchContractLeverage,
  placeOrder,
  setLeverage,
} from '@/api/trading'
import {
  enabledWatchScopesFromSymbols,
  fetchWatchedSymbols,
  type EnabledWatchScope,
} from '@/api/market'
import { useSystemStore } from '@/stores/systemStore'
import type {
  ContractAccountConfig,
  ContractLeverageInfo,
  InstType,
  MarginMode,
  OrderSide,
  OrderType,
  PositionSide,
  TradingMode,
} from '@/types'
import { describeError } from '@/utils/logger'

type TradeInstType = Extract<InstType, 'SPOT' | 'SWAP'>
type ContractOrderIntent = 'open_long' | 'close_long' | 'open_short' | 'close_short'

interface UseOrderFormOptions {
  mode?: () => TradingMode | undefined
  modeLocked?: () => boolean
  onSubmitted?: () => void
}

export function useOrderForm(options: UseOrderFormOptions = {}) {
  const systemStore = useSystemStore()

  const form = reactive({
    inst_type: 'SWAP' as TradeInstType,
    inst_id: 'BTC-USDT-SWAP',
    side: 'buy' as OrderSide,
    ord_type: 'limit' as OrderType,
    sz: 0.01,
    px: 0,
    td_mode: 'cross' as MarginMode,
    lever: 3,
    pos_side: 'long' as PositionSide,
    reduce_only: false,
    sync_leverage: true,
  })

  const submitting = ref(false)
  const leverageApplying = ref(false)
  const contractMetaLoading = ref(false)
  const loadingScopes = ref(false)
  const error = ref<string | null>(null)
  const contractMetaError = ref<string | null>(null)
  const leverageMessage = ref<string | null>(null)
  const selectedScopeKey = ref('')
  const manualSymbolVisible = ref(false)
  const watchedScopes = ref<EnabledWatchScope[]>([])
  const contractAccountConfig = ref<ContractAccountConfig | null>(null)
  const contractLeverage = ref<ContractLeverageInfo[]>([])

  const resolvedMode = computed(() => options.mode?.() ?? systemStore.tradingMode)
  const modeLocked = computed(() => options.modeLocked?.() === true)
  const isContract = computed(() => form.inst_type === 'SWAP')
  const orderInstId = computed(() => normalizeInstIdForType(form.inst_id, form.inst_type))
  const positionMode = computed(() => contractAccountConfig.value?.pos_mode ?? '')
  const isLongShortMode = computed(() => positionMode.value === 'long_short_mode')
  const hasKnownContractPositionMode = computed(() =>
    !isContract.value || positionMode.value === 'net_mode' || positionMode.value === 'long_short_mode'
  )
  const leverageUsesPositionSide = computed(() =>
    isContract.value && isLongShortMode.value && form.td_mode === 'isolated'
  )
  const currentLeverage = computed(() =>
    contractLeverage.value.find(item =>
      normalizeInstIdForType(item.inst_id, 'SWAP') === orderInstId.value &&
      item.mgn_mode === form.td_mode &&
      (!leverageUsesPositionSide.value || item.pos_side === form.pos_side)
    )?.lever ?? 0
  )

  const marketTypeOptions = [
    { value: 'SWAP' as TradeInstType, label: '永续合约' },
    { value: 'SPOT' as TradeInstType, label: '现货' },
  ]

  const orderTypes = [
    { value: 'limit' as OrderType, label: '限价' },
    { value: 'market' as OrderType, label: '市价' },
  ]
  const sideOptions = computed(() => [
    { value: 'buy' as OrderSide, label: '买入' },
    { value: 'sell' as OrderSide, label: '卖出' },
  ])
  const showSideToggle = computed(() => !isContract.value)

  const tdModeOptions = [
    { value: 'cross' as MarginMode, label: '全仓' },
    { value: 'isolated' as MarginMode, label: '逐仓' },
  ]

  const positionSideOptions = [
    { value: 'long', label: '做多' },
    { value: 'short', label: '做空' },
  ]

  const contractIntentOptions = [
    { value: 'open_long' as ContractOrderIntent, label: '开多', side: 'buy' as OrderSide, pos_side: 'long' as PositionSide, reduce_only: false },
    { value: 'close_long' as ContractOrderIntent, label: '平多', side: 'sell' as OrderSide, pos_side: 'long' as PositionSide, reduce_only: true },
    { value: 'open_short' as ContractOrderIntent, label: '开空', side: 'sell' as OrderSide, pos_side: 'short' as PositionSide, reduce_only: false },
    { value: 'close_short' as ContractOrderIntent, label: '平空', side: 'buy' as OrderSide, pos_side: 'short' as PositionSide, reduce_only: true },
  ]
  const visibleContractIntentOptions = computed(() => {
    if (isLongShortMode.value) return contractIntentOptions
    return contractIntentOptions
      .filter(item => item.value === 'open_long' || item.value === 'open_short')
      .map(item => ({
        ...item,
        label: item.value === 'open_long' ? '做多' : '做空',
      }))
  })

  const scopeOptions = computed(() =>
    watchedScopes.value.map(scope => ({
      value: scopeKey(scope),
      label: `${scope.symbol} · ${scope.inst_type === 'SWAP' ? '永续' : '现货'} · ${scope.inst_id}`,
    })),
  )
  const showManualSymbolInput = computed(() =>
    manualSymbolVisible.value || !selectedScopeKey.value || scopeOptions.value.length === 0
  )
  const canHideManualSymbolInput = computed(() =>
    manualSymbolVisible.value && !!selectedScopeKey.value && scopeOptions.value.length > 0
  )

  const selectedScopeModel = computed({
    get: () => selectedScopeKey.value,
    set: (value: string | number) => {
      selectedScopeKey.value = String(value)
      const scope = watchedScopes.value.find(item => scopeKey(item) === selectedScopeKey.value)
      if (!scope) return
      form.inst_type = scope.inst_type
      form.inst_id = scope.inst_id
      form.td_mode = defaultTdMode(scope.inst_type)
      manualSymbolVisible.value = false
      leverageMessage.value = null
      void refreshContractMeta()
    },
  })

  const manualSymbolModel = computed({
    get: () => form.inst_id,
    set: (value: string | number) => {
      form.inst_id = String(value || '').trim().toUpperCase()
      selectedScopeKey.value = findScopeKey(form.inst_id, form.inst_type)
      leverageMessage.value = null
    },
  })

  const marketTypeModel = computed({
    get: () => form.inst_type,
    set: (value: string | number) => {
      const instType = value === 'SPOT' ? 'SPOT' : 'SWAP'
      form.inst_type = instType
      form.inst_id = normalizeInstIdForType(form.inst_id, instType)
      form.td_mode = defaultTdMode(instType)
      selectedScopeKey.value = findScopeKey(form.inst_id, instType)
      leverageMessage.value = null
      void refreshContractMeta()
    },
  })

  const tdModeModel = computed({
    get: () => form.td_mode,
    set: (value: string | number) => {
      form.td_mode = value === 'isolated' ? 'isolated' : 'cross'
      leverageMessage.value = null
      void refreshContractMeta()
    },
  })

  const positionSideModel = computed({
    get: () => form.pos_side,
    set: (value: string | number) => {
      form.pos_side = value === 'short' ? 'short' : 'long'
      leverageMessage.value = null
      void refreshContractMeta()
    },
  })

  const canSubmit = computed(
    () =>
      !modeLocked.value &&
      !!orderInstId.value &&
      form.sz > 0 &&
      (form.ord_type === 'market' || form.px > 0) &&
      hasKnownContractPositionMode.value &&
      (!isLongShortMode.value || form.pos_side === 'long' || form.pos_side === 'short'),
  )

  const modeLabel = computed(() => {
    if (!systemStore.statusLoaded) return '模式读取中'
    const label = resolvedMode.value === 'live' ? '实盘' : '模拟盘'
    return modeLocked.value ? `查看：${label} LOCK` : label
  })

  const positionModeLabel = computed(() => {
    if (!isContract.value) return '现货无持仓模式'
    if (positionMode.value === 'long_short_mode') return '双向持仓'
    if (positionMode.value === 'net_mode') return '单向持仓'
    return contractMetaLoading.value ? '读取中' : '未知'
  })

  const submitLabel = computed(() => {
    const type = form.ord_type === 'market' ? '市价' : '限价'
    const market = isContract.value ? `${form.td_mode === 'cross' ? '全仓' : '逐仓'} ${form.lever}x` : '现货'
    return `${orderActionLabel.value} ${form.sz} ${orderInstId.value} (${type} · ${market})`
  })

  const orderActionLabel = computed(() => {
    if (!isContract.value) return form.side === 'buy' ? '买入' : '卖出'
    if (!isLongShortMode.value) {
      const direction = form.side === 'buy' ? '做多/减空' : '做空/减多'
      return form.reduce_only ? `只减仓 ${direction}` : direction
    }
    if (form.pos_side === 'long') return form.side === 'buy' ? '开多' : '平多'
    return form.side === 'sell' ? '开空' : '平空'
  })

  async function loadWatchedScopes() {
    loadingScopes.value = true
    try {
      watchedScopes.value = enabledWatchScopesFromSymbols(await fetchWatchedSymbols())
      selectedScopeKey.value = findScopeKey(form.inst_id, form.inst_type)
    } catch (caught) {
      contractMetaError.value = `关注品种读取失败：${describeError(caught)}`
    } finally {
      loadingScopes.value = false
    }
  }

  async function refreshContractMeta() {
    if (!isContract.value || !orderInstId.value) {
      contractAccountConfig.value = null
      contractLeverage.value = []
      contractMetaError.value = null
      return
    }

    contractMetaLoading.value = true
    contractMetaError.value = null
    const mode = resolvedMode.value
    const instId = orderInstId.value
    try {
      const [configResult, leverageResult] = await Promise.allSettled([
        fetchContractAccountConfig(mode),
        fetchContractLeverage(instId, { mode, mgn_mode: form.td_mode }),
      ])
      if (configResult.status === 'fulfilled') {
        contractAccountConfig.value = configResult.value
      }
      if (leverageResult.status === 'fulfilled') {
        contractLeverage.value = leverageResult.value
      }
      const errors = [configResult, leverageResult]
        .filter((item): item is PromiseRejectedResult => item.status === 'rejected')
        .map(item => describeError(item.reason))
      if (errors.length > 0) contractMetaError.value = errors.join('；')
    } finally {
      contractMetaLoading.value = false
    }
  }

  async function applyLeverage(options: { rethrow?: boolean; refresh?: boolean } = {}) {
    if (!isContract.value || leverageApplying.value) return
    if (modeLocked.value) {
      error.value = '当前查看模式与系统默认交易模式不一致，已禁止设置杠杆。'
      return
    }
    if (!Number.isFinite(form.lever) || form.lever < 1) {
      error.value = '杠杆倍数必须大于等于 1。'
      return
    }
    const instId = orderInstId.value
    if (!instId) {
      error.value = '请先选择或输入有效的合约品种。'
      return
    }
    leverageApplying.value = true
    error.value = null
    leverageMessage.value = null
    try {
      await setLeverage(instId, form.lever, {
        mode: resolvedMode.value,
        mgn_mode: form.td_mode,
        pos_side: leverageUsesPositionSide.value ? form.pos_side : undefined,
      })
      leverageMessage.value = `已设置 ${instId} ${form.td_mode === 'cross' ? '全仓' : '逐仓'} ${form.lever}x`
      if (options.refresh !== false) await refreshContractMeta()
    } catch (caught) {
      error.value = describeError(caught)
      if (options.rethrow) throw caught
    } finally {
      leverageApplying.value = false
    }
  }

  async function submit() {
    if (submitting.value) return
    if (modeLocked.value) {
      error.value = '当前查看模式与系统默认交易模式不一致，已禁止下单。请在“设置”切换默认模式后重试。'
      return
    }
    if (!canSubmit.value) return
    if (!systemStore.statusLoaded) {
      await systemStore.loadConfig()
    }
    submitting.value = true
    error.value = null
    try {
      if (isContract.value && form.sync_leverage) {
        await applyLeverage({ rethrow: true, refresh: false })
      }
      await placeOrder({
        inst_id: orderInstId.value,
        inst_type: form.inst_type,
        td_mode: isContract.value ? form.td_mode : 'cash',
        side: form.side,
        ord_type: form.ord_type,
        sz: form.sz,
        px: form.ord_type === 'limit' ? form.px : undefined,
        ...(isLongShortMode.value ? { pos_side: form.pos_side } : {}),
        ...(isContract.value ? { reduce_only: form.reduce_only } : {}),
        mode: resolvedMode.value,
      })
      options.onSubmitted?.()
    } catch (caught) {
      error.value = describeError(caught)
    } finally {
      submitting.value = false
    }
  }

  function applyContractIntent(intent: ContractOrderIntent) {
    const option = contractIntentOptions.find(item => item.value === intent)
    if (!option) return
    form.side = option.side
    form.pos_side = option.pos_side
    form.reduce_only = option.reduce_only
  }

  function showManualSymbolEditor() {
    manualSymbolVisible.value = true
  }

  function hideManualSymbolEditor() {
    if (!canHideManualSymbolInput.value) return
    manualSymbolVisible.value = false
  }

  function findScopeKey(instId: string, instType: TradeInstType): string {
    const normalizedId = normalizeInstIdForType(instId, instType)
    return scopeKey(watchedScopes.value.find(scope =>
      scope.inst_type === instType && scope.inst_id === normalizedId
    ))
  }

  onMounted(() => {
    void loadWatchedScopes()
    void refreshContractMeta()
  })

  watch(resolvedMode, () => {
    void refreshContractMeta()
  })

  return {
    systemStore,
    resolvedMode,
    modeLocked,
    form,
    submitting,
    leverageApplying,
    contractMetaLoading,
    loadingScopes,
    error,
    contractMetaError,
    leverageMessage,
    watchedScopes,
    selectedScopeModel,
    manualSymbolModel,
    manualSymbolVisible,
    showManualSymbolInput,
    canHideManualSymbolInput,
    scopeOptions,
    marketTypeOptions,
    marketTypeModel,
    orderTypes,
    sideOptions,
    showSideToggle,
    tdModeOptions,
    tdModeModel,
    positionSideOptions,
    positionSideModel,
    contractIntentOptions: visibleContractIntentOptions,
    canSubmit,
    isContract,
    isLongShortMode,
    currentLeverage,
    orderInstId,
    modeLabel,
    positionModeLabel,
    orderActionLabel,
    submitLabel,
    refreshContractMeta,
    applyLeverage,
    applyContractIntent,
    showManualSymbolEditor,
    hideManualSymbolEditor,
    submit,
  }
}

function scopeKey(scope: EnabledWatchScope | undefined): string {
  return scope ? `${scope.inst_type}:${scope.inst_id}` : ''
}

function defaultTdMode(instType: TradeInstType): MarginMode {
  return instType === 'SWAP' ? 'cross' : 'cash'
}

function normalizeInstIdForType(value: string, instType: TradeInstType): string {
  const normalized = normalizeSpotInstId(value)
  if (!normalized) return ''
  return instType === 'SWAP' ? `${normalized}-SWAP` : normalized
}

function normalizeSpotInstId(value: string): string {
  let normalized = String(value || '').trim().toUpperCase()
  if (!normalized) return ''
  if (!normalized.includes('-')) normalized = `${normalized}-USDT`
  if (normalized.endsWith('-SWAP')) normalized = normalized.slice(0, -5)
  return normalized
}
