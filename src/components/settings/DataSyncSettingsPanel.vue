<template>
  <div class="sync-settings">
    <div class="ss-header">
      <div class="ss-heading">
        <span class="ss-title">数据采集性能</span>
        <span class="ss-status">活跃任务 {{ config?.active_sync_jobs ?? 0 }}</span>
      </div>
      <div class="ss-actions">
        <button class="reset-btn" type="button" :disabled="saving || !config" @click="resetDefaults">
          恢复默认
        </button>
        <button class="save-btn" type="button" :disabled="saving || !config" @click="save">
          {{ saving ? '保存中...' : '保存参数' }}
        </button>
      </div>
    </div>

    <div v-if="config" class="ss-body">
      <div
        v-for="field in fields"
        :key="field.key"
        class="ss-field"
        :class="{ wide: field.wide }"
      >
        <div class="ss-field-head">
          <label :for="field.key">{{ field.label }}</label>
          <span>{{ field.min }} - {{ field.max }}</span>
        </div>
        <input
          :id="field.key"
          v-model.number="local[field.key]"
          class="ss-input"
          type="number"
          :min="field.min"
          :max="field.max"
          step="1"
          @blur="clampField(field.key)"
        />
      </div>
      <div class="ss-summary">
        <span>新任务生效</span>
        <span>单页 K 线 {{ candleBatchLimit }} 条</span>
      </div>
    </div>

    <div v-else class="ss-empty">参数读取中</div>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, watch } from 'vue'
import type { SyncRuntimeConfig, SyncRuntimeSettings } from '@/types/market'

type SettingKey = keyof SyncRuntimeSettings

const props = defineProps<{
  config: SyncRuntimeConfig | null
  saving?: boolean
}>()

const emit = defineEmits<{
  save: [settings: SyncRuntimeSettings]
}>()

const defaultSettings: SyncRuntimeSettings = {
  max_sync_batches: 2000,
  okx_page_pause_ms: 0,
  sync_job_concurrency: 2,
  window_fetch_concurrency: 8,
  window_fetch_batches_per_slice: 32,
  candle_upsert_transaction_chunk: 1000,
  okx_max_concurrency: 10,
  okx_public_rest_concurrency: 8,
  okx_private_rest_concurrency: 2,
  okx_trade_rest_concurrency: 2,
  okx_ws_control_concurrency: 1,
  okx_unknown_concurrency: 1,
}

const candleBatchLimit = 300
const local = reactive<SyncRuntimeSettings>({ ...defaultSettings })

const fields = computed(() => [
  field('sync_job_concurrency', '同步任务并发', true),
  field('window_fetch_concurrency', '窗口拉取并发', true),
  field('okx_max_concurrency', 'OKX 总并发', true),
  field('okx_public_rest_concurrency', 'OKX 公共 REST 并发', true),
  field('okx_private_rest_concurrency', 'OKX 私有 REST 并发'),
  field('okx_trade_rest_concurrency', 'OKX 交易 REST 并发'),
  field('okx_ws_control_concurrency', 'OKX WS 控制并发'),
  field('okx_unknown_concurrency', 'OKX 未分类并发'),
  field('max_sync_batches', '单次最大批次'),
  field('window_fetch_batches_per_slice', '每片批次数'),
  field('okx_page_pause_ms', '分页暂停 ms'),
  field('candle_upsert_transaction_chunk', '落库事务批量'),
])

watch(() => props.config?.settings, (settings) => {
  Object.assign(local, normalizeSettings(settings ?? defaultSettings))
}, { immediate: true, deep: true })

function field(key: SettingKey, label: string, wide = false) {
  const limit = props.config?.limits[key]
  return {
    key,
    label,
    wide,
    min: limit?.min ?? defaultLimit(key).min,
    max: limit?.max ?? defaultLimit(key).max,
  }
}

function defaultLimit(key: SettingKey) {
  const limits: Record<SettingKey, { min: number; max: number }> = {
    max_sync_batches: { min: 1, max: 20_000 },
    okx_page_pause_ms: { min: 0, max: 5_000 },
    sync_job_concurrency: { min: 1, max: 16 },
    window_fetch_concurrency: { min: 1, max: 32 },
    window_fetch_batches_per_slice: { min: 1, max: 256 },
    candle_upsert_transaction_chunk: { min: 100, max: 10_000 },
    okx_max_concurrency: { min: 1, max: 64 },
    okx_public_rest_concurrency: { min: 1, max: 64 },
    okx_private_rest_concurrency: { min: 1, max: 32 },
    okx_trade_rest_concurrency: { min: 1, max: 16 },
    okx_ws_control_concurrency: { min: 1, max: 8 },
    okx_unknown_concurrency: { min: 1, max: 16 },
  }
  return limits[key]
}

function clampField(key: SettingKey) {
  const limit = props.config?.limits[key] ?? defaultLimit(key)
  const value = Math.round(Number(local[key]))
  local[key] = Math.max(limit.min, Math.min(limit.max, Number.isFinite(value) ? value : defaultSettings[key]))
}

function normalizeSettings(settings: SyncRuntimeSettings): SyncRuntimeSettings {
  const next = { ...settings }
  for (const key of Object.keys(next) as SettingKey[]) {
    const limit = props.config?.limits[key] ?? defaultLimit(key)
    const value = Math.round(Number(next[key]))
    next[key] = Math.max(limit.min, Math.min(limit.max, Number.isFinite(value) ? value : defaultSettings[key]))
  }
  return next
}

function save() {
  emit('save', currentSettings())
}

function resetDefaults() {
  if (!props.config) return
  Object.assign(local, normalizeSettings(props.config.defaults))
}

function currentSettings(): SyncRuntimeSettings {
  for (const key of Object.keys(local) as SettingKey[]) clampField(key)
  return { ...local }
}

defineExpose({ currentSettings })
</script>

<style scoped>
.sync-settings {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
}

.ss-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border);
}

.ss-heading,
.ss-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.ss-title {
  font-size: 13px;
  font-weight: 600;
}

.ss-status {
  padding: 2px 7px;
  border: 1px solid rgba(38,166,154,0.35);
  border-radius: 999px;
  background: rgba(38,166,154,0.08);
  color: var(--color-positive);
  font-size: 11px;
  white-space: nowrap;
}

.save-btn,
.reset-btn {
  padding: 4px 12px;
  border-radius: 4px;
  font-size: 12px;
  cursor: pointer;
}

.save-btn {
  border: none;
  background: var(--color-accent);
  color: #fff;
}

.reset-btn {
  border: 1px solid var(--color-border);
  background: var(--color-bg-primary);
  color: var(--color-text-secondary);
}

.save-btn:disabled,
.reset-btn:disabled {
  cursor: not-allowed;
  opacity: 0.65;
}

.ss-body {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
  padding: 12px;
}

.ss-field {
  min-width: 0;
}

.ss-field.wide {
  grid-column: span 1;
}

.ss-field-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 3px;
}

.ss-field-head label {
  color: var(--color-text-tertiary);
  font-size: 11px;
}

.ss-field-head span {
  color: var(--color-text-tertiary);
  font-family: var(--font-mono, monospace);
  font-size: 10px;
}

.ss-input {
  width: 100%;
  height: 28px;
  padding: 5px 8px;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  background: var(--color-bg-primary);
  color: var(--color-text-primary);
  font-size: 12px;
}

.ss-input:focus {
  border-color: var(--color-accent);
  outline: none;
}

.ss-summary {
  grid-column: 1 / -1;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding-top: 2px;
  color: var(--color-text-tertiary);
  font-size: 11px;
}

.ss-empty {
  padding: 16px 12px;
  color: var(--color-text-tertiary);
  font-size: 12px;
}

@media (max-width: 760px) {
  .ss-header {
    align-items: flex-start;
    flex-direction: column;
  }

  .ss-body {
    grid-template-columns: 1fr;
  }
}
</style>
