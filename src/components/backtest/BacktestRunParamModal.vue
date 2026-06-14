<template>
  <BacktestParamModal
    allow-empty-boolean
    close-label="关闭运行回测参数弹窗"
    draft-empty-text="暂无策略参数"
    draft-section-title="策略参数"
    secondary-draft-empty-text="暂无回测引擎参数"
    secondary-draft-section-title="回测引擎参数"
    submit-label="开始回测"
    :draft-rows="strategyDraftRows"
    :running="running"
    :runtime-rows="runtimeRows"
    :secondary-draft-rows="engineDraftRows"
    :subtitle="strategy?.name || strategyId"
    title="运行回测参数"
    title-id="backtest-run-param-title"
    @close="emit('close')"
    @reset="resetDrafts"
    @submit="submit"
  >
    <template #before-primary>
      <div class="param-section run-account-section">
        <div class="param-section-title">回测账户</div>
        <label class="run-account-field">
          <span>初始资金</span>
          <input
            v-model="initialCapitalInput"
            class="run-initial-capital-input"
            min="0"
            name="backtest-initial-capital"
            placeholder="默认按策略配置"
            step="any"
            type="number"
          >
        </label>
        <div v-if="initialCapitalError" class="run-param-error">{{ initialCapitalError }}</div>
      </div>
      <div class="param-section run-date-section">
        <div class="param-section-title">回测日期</div>
        <div class="run-date-grid">
          <label class="run-date-field">
            <span>开始日期</span>
            <ThemeDateInput
              v-model="startDate"
              class="run-start-date-input"
              :max="endDate || undefined"
              placeholder="默认按策略窗口"
            />
          </label>
          <label class="run-date-field">
            <span>结束日期</span>
            <ThemeDateInput
              v-model="endDate"
              class="run-end-date-input"
              :min="startDate || undefined"
              placeholder="默认到当前时间"
            />
          </label>
        </div>
        <div v-if="dateError" class="run-param-error">{{ dateError }}</div>
      </div>
    </template>
  </BacktestParamModal>
</template>

<script setup lang="ts">
import {
  computed,
  ref,
  watch,
} from 'vue'
import BacktestParamModal from '@/components/backtest/BacktestParamModal.vue'
import ThemeDateInput from '@/components/shared/ThemeDateInput.vue'
import type { StrategyMeta } from '@/types'
import { formatMoney } from '@/utils/format'
import {
  buildParamsFromDraftRows,
  draftRowFromValue,
  draftRowsFromParams,
  engineParamSpecsFromSource,
  formatRatio,
  omitEngineParams,
  type AnyRecord,
  type ParamDraftRow,
} from '@/utils/backtestResultCard'

const props = defineProps<{
  running: boolean
  strategy: StrategyMeta | null
  strategyId: string
}>()

const emit = defineEmits<{
  close: []
  submit: [payload: Record<string, unknown>]
}>()

const strategyDraftRows = ref<ParamDraftRow[]>([])
const engineDraftRows = ref<ParamDraftRow[]>([])
const initialCapitalInput = ref<string | number>('')
const initialCapitalError = ref('')
const startDate = ref('')
const endDate = ref('')
const dateError = ref('')

const runtimeRows = computed(() => {
  const strategy = props.strategy
  const runtime = strategy?.runtime
  return [
    { label: '策略ID', value: strategy?.id || props.strategyId || '--' },
    { label: '策略名称', value: strategy?.name || '--' },
    { label: '标的', value: runtime?.symbol || '--' },
    { label: '市场类型', value: runtime?.inst_type || '--' },
    { label: '周期', value: runtime?.timeframe || '--' },
    { label: '初始资金', value: formatMoney(displayInitialCapital(runtime?.initial_capital)) },
    { label: '默认仓位', value: runtime ? formatRatio(runtime.position_size) : '--' },
  ]
})

watch(() => [props.strategy, props.strategyId] as const, resetDrafts, { immediate: true })

function resetDrafts() {
  const runtime = props.strategy?.runtime
  const params = runtime?.params ?? {}
  dateError.value = ''
  initialCapitalError.value = ''
  initialCapitalInput.value = runtimeCapitalInput(runtime?.initial_capital)
  startDate.value = ''
  endDate.value = ''
  strategyDraftRows.value = draftRowsFromParams(omitEngineParams(params))
  engineDraftRows.value = engineParamSpecsFromSource({
    params,
    runtime: runtime ? ({ ...runtime } as AnyRecord) : {},
  }).map(spec => ({
    ...draftRowFromValue({
      key: spec.key,
      label: spec.label,
      value: spec.value,
      depth: 0,
    }, spec.kind),
    options: spec.options,
  }))
}

function submit() {
  dateError.value = ''
  initialCapitalError.value = ''
  if (startDate.value && endDate.value && endDate.value < startDate.value) {
    dateError.value = '结束日期必须晚于开始日期'
    return
  }
  const initialCapital = parseInitialCapitalInput()
  if (initialCapital === null) return
  const strategyParams = buildParamsFromDraftRows(strategyDraftRows.value, 'strict')
  if (!strategyParams) return
  const engineParams = buildParamsFromDraftRows(engineDraftRows.value, 'skip-empty')
  if (!engineParams) return
  const payload: Record<string, unknown> = {
    params: {
      ...strategyParams,
      ...engineParams,
    },
  }
  if (initialCapital !== undefined) payload.initial_capital = initialCapital
  if (startDate.value) payload.start_date = startDate.value
  if (endDate.value) payload.end_date = endDate.value
  emit('submit', payload)
}

function runtimeCapitalInput(value: unknown) {
  return typeof value === 'number' && Number.isFinite(value) && value > 0 ? String(value) : ''
}

function parseRuntimeCapitalInput() {
  const value = Number(initialCapitalInput.value)
  return Number.isFinite(value) && value > 0 ? value : null
}

function displayInitialCapital(defaultValue: unknown) {
  const raw = String(initialCapitalInput.value ?? '').trim()
  if (raw) return parseRuntimeCapitalInput()
  return typeof defaultValue === 'number' && Number.isFinite(defaultValue) && defaultValue > 0
    ? defaultValue
    : null
}

function parseInitialCapitalInput() {
  const raw = String(initialCapitalInput.value ?? '').trim()
  if (!raw) return undefined
  const value = Number(raw)
  if (!Number.isFinite(value) || value <= 0) {
    initialCapitalError.value = '初始资金必须大于 0'
    return null
  }
  return value
}
</script>

<style scoped>
.run-account-section,
.run-date-section {
  border-bottom: 1px solid var(--color-border);
  padding: 12px 16px;
}
.run-account-section .param-section-title,
.run-date-section .param-section-title {
  margin-bottom: 8px;
  color: var(--color-text-secondary);
  font-size: 12px;
  font-weight: 600;
}
.run-date-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
}
.run-account-field,
.run-date-field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
}
.run-account-field span,
.run-date-field span {
  color: var(--color-text-tertiary);
  font-size: 11px;
  line-height: 1.2;
}
.run-initial-capital-input {
  box-sizing: border-box;
  width: min(280px, 100%);
  min-width: 0;
  padding: 6px 8px;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  background: rgba(0, 0, 0, 0.14);
  color: var(--color-text-primary);
  font-size: 12px;
  line-height: 1.35;
  outline: none;
}
.run-initial-capital-input:focus {
  border-color: rgba(41, 98, 255, 0.56);
}
.run-param-error {
  margin-top: 8px;
  color: var(--color-negative);
  font-size: 11px;
  line-height: 1.35;
}

@media (max-width: 720px) {
  .run-date-grid {
    grid-template-columns: 1fr;
  }
}
</style>
