<template>
  <Teleport to="body">
    <div
      class="param-modal-backdrop"
      role="dialog"
      aria-modal="true"
      :aria-labelledby="titleId"
      @click.self="emit('close')"
    >
      <div class="param-modal">
        <div class="param-modal-head">
          <div>
            <span :id="titleId" class="param-modal-title">{{ title }}</span>
            <span class="param-modal-subtitle">{{ subtitle }}</span>
          </div>
          <button type="button" class="param-close" :aria-label="closeLabel" @click="emit('close')">×</button>
        </div>
        <div class="param-modal-body">
          <div v-if="runtimeRows.length > 0" class="param-section">
            <div class="param-section-title">运行参数</div>
            <BacktestParamRuntimeGrid :rows="runtimeRows" />
          </div>
          <slot name="before-primary" />
          <div v-if="showPrimarySection" class="param-section">
            <div class="param-section-title">{{ draftSectionTitle }}</div>
            <BacktestParamReadableList
              v-if="readonly"
              :empty-text="draftEmptyText"
              :rows="primaryReadonlyRows"
            />
            <BacktestParamEditorList
              v-else
              :boolean-select-options="booleanSelectOptions"
              :empty-text="draftEmptyText"
              name-prefix="param"
              :rows="draftRows"
            />
          </div>
          <div v-if="showSecondarySection && !readonly" class="param-section">
            <div class="param-section-title">{{ secondaryDraftSectionTitle }}</div>
            <BacktestParamEditorList
              :boolean-select-options="booleanSelectOptions"
              :empty-text="secondaryDraftEmptyText"
              name-prefix="secondary-param"
              :rows="secondaryDraftRows"
            />
          </div>
          <div v-if="detailSectionTitle" class="param-section">
            <div class="param-section-title">{{ detailSectionTitle }}</div>
            <BacktestParamReadableList empty-text="暂无引擎字段" :rows="detailRows" />
          </div>
        </div>
        <div class="param-modal-actions">
          <button v-if="showReset && !readonly" type="button" class="param-secondary-btn" @click="emit('reset')">重置</button>
          <button type="button" class="param-secondary-btn" @click="emit('close')">{{ closeButtonText }}</button>
          <button v-if="!readonly" type="button" class="param-submit-btn" :disabled="running" @click="emit('submit')">
            {{ submitLabel }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { ParamDraftRow, ReadableParamRow } from '@/utils/backtestResultCard'
import BacktestParamEditorList from './BacktestParamEditorList.vue'
import BacktestParamReadableList from './BacktestParamReadableList.vue'
import BacktestParamRuntimeGrid from './BacktestParamRuntimeGrid.vue'

type RuntimeParamRow = {
  label: string
  value: string
}

const props = withDefaults(defineProps<{
  allowEmptyBoolean?: boolean
  closeLabel: string
  detailRows?: ReadableParamRow[]
  detailSectionTitle?: string
  draftEmptyText?: string
  draftRows?: ParamDraftRow[]
  draftSectionTitle?: string
  readonly?: boolean
  readonlyRows?: ReadableParamRow[]
  running?: boolean
  runtimeRows?: RuntimeParamRow[]
  secondaryDraftEmptyText?: string
  secondaryDraftRows?: ParamDraftRow[]
  secondaryDraftSectionTitle?: string
  showReset?: boolean
  submitLabel?: string
  subtitle: string
  title: string
  titleId: string
}>(), {
  allowEmptyBoolean: false,
  detailRows: () => [],
  detailSectionTitle: '',
  draftEmptyText: '暂无参数',
  draftRows: () => [],
  draftSectionTitle: '',
  readonly: false,
  readonlyRows: () => [],
  running: false,
  runtimeRows: () => [],
  secondaryDraftEmptyText: '暂无参数',
  secondaryDraftRows: () => [],
  secondaryDraftSectionTitle: '',
  showReset: true,
  submitLabel: '应用并重新回测',
})

const emit = defineEmits<{
  close: []
  reset: []
  submit: []
}>()

const showPrimarySection = computed(() =>
  Boolean(props.draftSectionTitle || props.draftRows.length > 0 || props.readonlyRows.length > 0)
)
const showSecondarySection = computed(() =>
  Boolean(props.secondaryDraftSectionTitle || props.secondaryDraftRows.length > 0)
)
const primaryReadonlyRows = computed<ReadableParamRow[]>(() => {
  if (props.readonlyRows.length > 0) return props.readonlyRows
  return props.draftRows.map(row => ({
    key: row.key,
    label: row.label,
    value: row.value || row.input || '--',
    depth: row.depth,
    group: row.group,
    multiline: row.multiline,
  }))
})
const closeButtonText = computed(() => props.readonly ? '关闭' : '取消')
const booleanSelectOptions = computed(() => [
  ...(props.allowEmptyBoolean ? [{ value: '', label: '未设置' }] : []),
  { value: 'true', label: '是' },
  { value: 'false', label: '否' },
])
</script>

<style scoped>
.param-modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 3000;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(4, 6, 12, 0.72);
  padding: 24px;
}
.param-modal {
  width: min(760px, 100%);
  max-height: min(78vh, 760px);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: var(--color-bg-secondary);
  box-shadow: 0 24px 70px rgba(0, 0, 0, 0.38);
}
.param-modal-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 14px;
  border-bottom: 1px solid var(--color-border);
  padding: 14px 16px 12px;
}
.param-modal-body {
  min-height: 0;
  overflow: auto;
}
.param-modal-title {
  display: block;
  color: var(--color-text-primary);
  font-size: 15px;
  font-weight: 700;
  line-height: 1.3;
}
.param-modal-subtitle {
  display: block;
  margin-top: 3px;
  color: var(--color-text-tertiary);
  font-size: 11px;
  line-height: 1.35;
  overflow-wrap: anywhere;
}
.param-close {
  flex: 0 0 auto;
  width: 28px;
  height: 28px;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  background: var(--color-bg-primary);
  color: var(--color-text-secondary);
  cursor: pointer;
  font-size: 18px;
  line-height: 24px;
}
.param-close:hover {
  color: var(--color-text-primary);
  border-color: rgba(41, 98, 255, 0.45);
}
.param-section {
  border-bottom: 1px solid var(--color-border);
  padding: 12px 16px;
}
.param-modal-body .param-section:last-child {
  min-height: 0;
  overflow: auto;
  border-bottom: 0;
}
.param-section-title {
  margin-bottom: 8px;
  color: var(--color-text-secondary);
  font-size: 12px;
  font-weight: 600;
}
.param-modal-actions {
  display: flex;
  flex: 0 0 auto;
  justify-content: flex-end;
  gap: 8px;
  border-top: 1px solid var(--color-border);
  padding: 10px 16px 12px;
}
.param-secondary-btn,
.param-submit-btn {
  border: 1px solid var(--color-border);
  border-radius: 4px;
  cursor: pointer;
  font-size: 12px;
  font-weight: 600;
  line-height: 1.2;
  padding: 7px 12px;
}
.param-secondary-btn {
  background: transparent;
  color: var(--color-text-secondary);
}
.param-secondary-btn:hover {
  color: var(--color-text-primary);
}
.param-submit-btn {
  border-color: rgba(41, 98, 255, 0.45);
  background: var(--color-accent);
  color: #fff;
}
.param-submit-btn:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}
@media (max-width: 640px) {
  .param-modal-backdrop {
    align-items: stretch;
    padding: 12px;
  }
  .param-modal {
    max-height: calc(100vh - 24px);
  }
  .param-modal-actions {
    flex-wrap: wrap;
  }
  .param-secondary-btn,
  .param-submit-btn {
    flex: 1 1 120px;
  }
}
</style>
