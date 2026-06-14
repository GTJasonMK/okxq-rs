<template>
  <div class="result-card">
    <BacktestResultHeader
      :result="result"
      :running="running"
      @open-engine-params="openEngineParams"
      @open-strategy-params="openStrategyParams"
    />
    <BacktestResultMetrics :result="result" />
    <BacktestResultIntegrityNotice :integrity="resultIntegrity" />
    <BacktestRuntimeDiagnostics :summary="runtimeSummary" />
    <BacktestParamModal
      v-if="showParams"
      close-label="关闭回测参数弹窗"
      draft-empty-text="暂无策略参数"
      draft-section-title="策略参数"
      readonly
      :readonly-rows="strategyReadableRows"
      :running="running"
      :runtime-rows="runtimeParamRows"
      :subtitle="result.strategy_name || result.strategy_id"
      title="回测参数"
      title-id="backtest-param-title"
      @close="showParams = false"
    />
    <BacktestParamModal
      v-if="showEngineParams"
      close-label="关闭回测引擎参数弹窗"
      detail-section-title="引擎字段明细"
      draft-section-title="实际执行模型"
      readonly
      :detail-rows="engineDetailRows"
      :readonly-rows="engineReadableRows"
      :running="running"
      :subtitle="result.strategy_name || result.strategy_id"
      title="回测引擎参数"
      title-id="backtest-engine-param-title"
      @close="showEngineParams = false"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import type { BacktestResult } from '@/types'
import { formatMoney } from '@/utils/format'
import BacktestParamModal from './BacktestParamModal.vue'
import BacktestResultHeader from './BacktestResultHeader.vue'
import BacktestResultIntegrityNotice from './BacktestResultIntegrityNotice.vue'
import BacktestResultMetrics from './BacktestResultMetrics.vue'
import BacktestRuntimeDiagnostics from './BacktestRuntimeDiagnostics.vue'
import {
  engineParamSpecsFromSource,
  formatPlainNumber,
  pickEngineParams,
  readableParamRows,
  readableRowsFromEngineSpecs,
  type EngineParamSpec,
  type RuntimeSummary,
} from '@/utils/backtestResultCard'

const props = withDefaults(defineProps<{
  result: BacktestResult
  running?: boolean
}>(), {
  running: false,
})
const running = computed(() => props.running)
const showParams = ref(false)
const showEngineParams = ref(false)

const runtimeSummary = computed<RuntimeSummary | null>(() => {
  const summary = props.result.runtime_action_summary
  if (!summary || Array.isArray(summary) || typeof summary !== 'object') {
    return null
  }
  return Object.keys(summary).length > 0 ? summary : null
})

const runtimeParamRows = computed(() => [
  { label: '策略ID', value: props.result.strategy_id || '--' },
  { label: '策略名称', value: props.result.strategy_name || '--' },
  { label: '标的', value: props.result.symbol || '--' },
  { label: '市场类型', value: props.result.inst_type || '--' },
  { label: '周期', value: props.result.timeframe || '--' },
  { label: '天数', value: formatPlainNumber(props.result.days) },
  { label: '初始资金', value: formatMoney(props.result.initial_capital) },
  { label: '最终权益', value: formatMoney(props.result.final_equity) },
  { label: '创建时间', value: props.result.created_at || '--' },
])

const enginePayload = computed(() => {
  const params = props.result.params ?? {}
  return {
    execution_model: props.result.execution_model ?? {},
    cost_model: props.result.cost_model ?? {},
    params_used_by_engine: pickEngineParams(params),
  }
})

const engineDetailRows = computed(() => readableParamRows(enginePayload.value))
const strategyReadableRows = computed(() => readableParamRows(props.result.params ?? {}))
const engineReadableRows = computed(() => readableRowsFromEngineSpecs(engineParamSpecs.value))

const engineParamSpecs = computed<EngineParamSpec[]>(() => {
  return engineParamSpecsFromSource({
    contractMode: props.result.contract_mode,
    costModel: props.result.cost_model ?? {},
    executionModel: props.result.execution_model ?? {},
    params: props.result.params ?? {},
  })
})

const resultIntegrity = computed<RuntimeSummary | null>(() => {
  const integrity = props.result.backtest_result_integrity
  if (!integrity || Array.isArray(integrity) || typeof integrity !== 'object') {
    return null
  }
  return Object.keys(integrity).length > 0 ? integrity : null
})

function openStrategyParams() {
  showParams.value = true
}

function openEngineParams() {
  showEngineParams.value = true
}
</script>

<style scoped>
.result-card {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  flex: 0 0 auto;
  min-width: 0;
  overflow: visible;
}
</style>
