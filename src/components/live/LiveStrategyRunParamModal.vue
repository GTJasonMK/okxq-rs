<template>
  <BacktestParamModal
    allow-empty-boolean
    close-label="关闭启动策略参数弹窗"
    draft-empty-text="暂无策略参数"
    draft-section-title="策略参数"
    secondary-draft-empty-text="暂无运行参数"
    secondary-draft-section-title="实盘/模拟盘运行参数"
    submit-label="启动策略"
    :draft-rows="strategyDraftRows"
    :running="running"
    :runtime-rows="runtimeRows"
    :secondary-draft-rows="runtimeDraftRows"
    :subtitle="strategy?.name || form.strategy_id"
    title="启动策略参数"
    title-id="live-run-param-title"
    @close="emit('close')"
    @reset="resetDrafts"
    @submit="submit"
  />
</template>

<script setup lang="ts">
import {
  computed,
  ref,
  watch,
} from 'vue'
import BacktestParamModal from '@/components/backtest/BacktestParamModal.vue'
import type { StrategyMeta } from '@/types'
import { formatMoney } from '@/utils/format'
import {
  buildParamsFromDraftRows,
  formatRatio,
  type ParamDraftRow,
} from '@/utils/backtestResultCard'
import type { LiveStrategyControlForm } from '@/utils/liveStrategyForm'
import {
  liveRunRuntimeDraftRows,
  liveRunStrategyDraftRows,
} from '@/utils/liveStrategyRunParams'

const props = defineProps<{
  form: LiveStrategyControlForm
  running: boolean
  strategy: StrategyMeta | null
}>()

const emit = defineEmits<{
  close: []
  submit: [payload: Record<string, unknown>]
}>()

const strategyDraftRows = ref<ParamDraftRow[]>([])
const runtimeDraftRows = ref<ParamDraftRow[]>([])

const runtimeRows = computed(() => [
  { label: '策略ID', value: props.strategy?.id || props.form.strategy_id || '--' },
  { label: '策略名称', value: props.strategy?.name || '--' },
  { label: '标的', value: props.form.symbol || '--' },
  { label: '市场类型', value: props.form.inst_type || props.strategy?.runtime?.inst_type || '--' },
  { label: '周期', value: props.form.timeframe || '--' },
  { label: '初始资金', value: formatMoney(props.form.initial_capital) },
  { label: '默认仓位', value: formatRatio(props.form.position_size) },
])

watch(() => [props.strategy, props.form] as const, resetDrafts, { immediate: true, deep: true })

function resetDrafts() {
  strategyDraftRows.value = liveRunStrategyDraftRows(props.form)
  runtimeDraftRows.value = liveRunRuntimeDraftRows(props.form, props.strategy)
}

function submit() {
  const strategyParams = buildParamsFromDraftRows(strategyDraftRows.value, 'strict')
  if (!strategyParams) return
  const runtimeParams = buildParamsFromDraftRows(runtimeDraftRows.value, 'strict')
  if (!runtimeParams) return
  const {
    initial_capital,
    position_size,
    stop_loss,
    take_profit,
    check_interval,
    risk_timeframe,
    ...paramOverrides
  } = runtimeParams
  emit('submit', {
    initial_capital,
    position_size,
    stop_loss,
    take_profit,
    check_interval,
    risk_timeframe,
    params: {
      ...strategyParams,
      ...paramOverrides,
    },
  })
}

</script>
