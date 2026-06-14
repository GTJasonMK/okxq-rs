<template>
  <div class="wsp-editor">
    <div class="wsp-head">
      <span class="wsp-title">K线采集规则</span>
      <div class="wsp-actions">
        <button type="button" class="wsp-link" @click="enableCommon">常用</button>
        <button type="button" class="wsp-link" @click="enableAll">全选</button>
        <button type="button" class="wsp-link danger" @click="disableAll">清空</button>
      </div>
    </div>

    <div class="wsp-controls">
      <label class="wsp-field">
        <span>同步天数</span>
        <input
          class="wsp-days"
          :value="normalizedDays"
          type="number"
          min="1"
          max="3650"
          step="1"
          @input="setUnifiedDays(($event.target as HTMLInputElement).value)"
        />
      </label>
    </div>

    <div class="wsp-grid">
      <div
        v-for="plan in localPlans"
        :key="plan.timeframe"
        class="wsp-row"
        :class="{ disabled: !plan.enabled, base: isDerivedBase(plan.timeframe) }"
      >
        <label class="wsp-toggle">
          <input
            :checked="plan.enabled"
            :disabled="isDerivedBase(plan.timeframe)"
            type="checkbox"
            @change="setEnabled(plan.timeframe, ($event.target as HTMLInputElement).checked)"
          />
          <span>{{ plan.timeframe }}</span>
        </label>
      </div>
    </div>

    <div class="wsp-summary">
      <span>{{ enabledCount }} 个周期</span>
      <span>{{ summaryText }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, watch } from 'vue'
import type { Timeframe, WatchedSymbolSyncPlan } from '@/types'
import {
  applyUnifiedSyncDays,
  BASE_SYNC_TIMEFRAME,
  ensureDerivedBaseSyncPlans,
  normalizeFullSyncPlans,
  normalizeSyncDays,
  normalizeSyncPlan,
  sameSyncPlans,
} from '@/utils/syncPlans'

const props = defineProps<{
  modelValue: WatchedSymbolSyncPlan[]
  syncDays: number
}>()

const emit = defineEmits<{
  'update:modelValue': [value: WatchedSymbolSyncPlan[]]
  'update:syncDays': [value: number]
}>()

const commonTimeframes = new Set<Timeframe>([BASE_SYNC_TIMEFRAME, '5m', '15m', '1H', '4H', '1D'])

const localPlans = computed(() => normalizePlans(props.modelValue))
const enabledPlans = computed(() => localPlans.value.filter(plan => plan.enabled))
const enabledCount = computed(() => enabledPlans.value.length)
const normalizedDays = computed(() => normalizeSyncDays(props.syncDays))
const summaryText = computed(() => {
  if (enabledPlans.value.length === 0) return '未选择采集周期'
  const targets = enabledPlans.value.map(plan => plan.timeframe).join(' / ')
  return `${targets} · ${normalizedDays.value}天 · 1m底座派生`
})

watch([() => props.modelValue, () => props.syncDays], ([plans]) => {
  const normalized = normalizePlans(plans)
  if (!sameSyncPlans(plans, normalized)) {
    emit('update:modelValue', normalized)
  }
}, { immediate: true, deep: true })

function setEnabled(timeframe: Timeframe, enabled: boolean) {
  if (isDerivedBase(timeframe)) return
  updatePlan(timeframe, plan => ({ ...plan, enabled }))
}

function setUnifiedDays(value: string) {
  const parsed = Math.round(Number(value))
  const nextDays = normalizeSyncDays(Number.isFinite(parsed) ? parsed : normalizedDays.value)
  const nextPlans = applyUnifiedSyncDays(localPlans.value, nextDays)
  emit('update:syncDays', nextDays)
  emit('update:modelValue', ensureDerivedBase(nextPlans, nextDays))
}

function enableCommon() {
  const next = localPlans.value.map(plan => ({
    ...plan,
    enabled: commonTimeframes.has(plan.timeframe),
  }))
  emit('update:modelValue', ensureDerivedBase(next))
}

function enableAll() {
  emit('update:modelValue', localPlans.value.map(plan => ({ ...plan, enabled: true })))
}

function disableAll() {
  const next = localPlans.value.map(plan => ({ ...plan, enabled: false }))
  emit('update:modelValue', ensureDerivedBase(next))
}

function updatePlan(timeframe: Timeframe, updater: (plan: WatchedSymbolSyncPlan) => WatchedSymbolSyncPlan) {
  const next = localPlans.value.map(plan => (
    plan.timeframe === timeframe ? normalizeSyncPlan(updater(plan)) : plan
  ))
  emit('update:modelValue', ensureDerivedBase(next))
}

function normalizePlans(plans: WatchedSymbolSyncPlan[]): WatchedSymbolSyncPlan[] {
  const normalized = applyUnifiedSyncDays(normalizeFullSyncPlans(plans), normalizedDays.value)
  return ensureDerivedBase(normalized)
}

function ensureDerivedBase(plans: WatchedSymbolSyncPlan[], syncDays = normalizedDays.value): WatchedSymbolSyncPlan[] {
  return ensureDerivedBaseSyncPlans(plans, syncDays)
}

function isDerivedBase(timeframe: Timeframe) {
  return timeframe === BASE_SYNC_TIMEFRAME
}
</script>

<style scoped>
.wsp-editor {
  display: flex;
  flex-direction: column;
  gap: 8px;
  width: 100%;
  min-width: 0;
  padding-top: 10px;
  border-top: 1px solid var(--color-border);
}

.wsp-head,
.wsp-actions,
.wsp-summary {
  display: flex;
  align-items: center;
  gap: 8px;
}

.wsp-head {
  justify-content: space-between;
}

.wsp-controls {
  display: grid;
  grid-template-columns: minmax(150px, 0.5fr) minmax(240px, 1fr);
  gap: 8px;
}

.wsp-field {
  display: flex;
  flex-direction: column;
  gap: 5px;
  min-width: 0;
  color: var(--color-text-tertiary);
  font-size: 11px;
  font-weight: 600;
}

.wsp-title {
  color: var(--color-text-secondary);
  font-size: 12px;
  font-weight: 600;
}

.wsp-link {
  border: 0;
  background: transparent;
  color: var(--color-accent);
  cursor: pointer;
  font-size: 12px;
}

.wsp-link.danger {
  color: var(--color-negative);
}

.wsp-grid {
  display: grid;
  grid-template-columns: repeat(6, minmax(76px, 1fr));
  gap: 8px;
}

.wsp-row {
  display: flex;
  align-items: center;
  min-width: 0;
  min-height: 34px;
  padding: 6px 8px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: var(--color-bg-primary);
}

.wsp-row.disabled {
  opacity: 0.58;
}

.wsp-row.base {
  border-color: color-mix(in srgb, var(--color-accent) 45%, var(--color-border));
  background: color-mix(in srgb, var(--color-accent) 8%, var(--color-bg-primary));
  opacity: 1;
}

.wsp-toggle {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  color: var(--color-text-secondary);
  font-size: 12px;
  font-weight: 600;
}

.wsp-toggle input {
  accent-color: var(--color-accent);
}

.wsp-days {
  width: 100%;
  min-width: 0;
  height: 26px;
  padding: 4px 7px;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  background: var(--color-bg-secondary);
  color: var(--color-text-primary);
  font-size: 12px;
  outline: none;
}

.wsp-days:focus {
  border-color: var(--color-accent);
}

.wsp-summary {
  flex-wrap: wrap;
  color: var(--color-text-tertiary);
  font-size: 12px;
}

@media (max-width: 1320px) {
  .wsp-grid {
    grid-template-columns: repeat(4, minmax(76px, 1fr));
  }
}

@media (max-width: 980px) {
  .wsp-controls {
    grid-template-columns: 1fr;
  }

  .wsp-grid {
    grid-template-columns: repeat(3, minmax(76px, 1fr));
  }
}

@media (max-width: 640px) {
  .wsp-grid {
    grid-template-columns: repeat(2, minmax(76px, 1fr));
  }
}
</style>
