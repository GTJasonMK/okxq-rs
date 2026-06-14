<template>
  <div
    ref="rootRef"
    class="theme-date-input"
    :class="[`size-${size}`, { open, disabled }]"
  >
    <button
      ref="triggerRef"
      type="button"
      class="td-trigger"
      :disabled="disabled"
      :aria-expanded="open"
      aria-haspopup="dialog"
      @click="toggle"
      @keydown="onTriggerKeydown"
    >
      <span class="td-calendar-icon" aria-hidden="true" />
      <span class="td-label" :class="{ placeholder: !modelValue }">{{ displayLabel }}</span>
    </button>
    <button
      v-if="modelValue && !disabled"
      type="button"
      class="td-clear"
      aria-label="清空日期"
      @click.stop="clearDate"
    >
      ×
    </button>

    <Teleport to="body">
      <ThemeDatePanel
        v-if="open"
        ref="panelRef"
        :calendar-days="calendarDays"
        :month-label="monthLabel"
        :panel-style="panelStyle"
        :today-disabled="todayDisabled"
        :weekdays="weekdays"
        @clear-date="clearDate"
        @panel-keydown="onPanelKeydown"
        @select-date="selectDate"
        @select-today="selectToday"
        @shift-month="shiftMonth"
      />
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import {
  buildCalendarDays,
  formatDateValue,
  monthStart,
  parseDateValue,
} from '@/utils/themeDateInput'
import ThemeDatePanel from './ThemeDatePanel.vue'

const props = withDefaults(defineProps<{
  modelValue?: string
  placeholder?: string
  disabled?: boolean
  min?: string
  max?: string
  size?: 'sm' | 'md'
}>(), {
  modelValue: '',
  placeholder: '选择日期',
  disabled: false,
  min: '',
  max: '',
  size: 'md',
})

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

const rootRef = ref<HTMLElement | null>(null)
const triggerRef = ref<HTMLButtonElement | null>(null)
const panelRef = ref<{ panelElement: HTMLElement | null } | null>(null)
const open = ref(false)
const panelStyle = ref<Record<string, string>>({})
const currentMonth = ref(monthStart(parseDateValue(props.modelValue) ?? new Date()))
const weekdays = ['一', '二', '三', '四', '五', '六', '日']
const todayValue = formatDateValue(new Date())

const displayLabel = computed(() => {
  if (!props.modelValue) return props.placeholder
  const parsed = parseDateValue(props.modelValue)
  if (!parsed) return props.modelValue
  return parsed.toLocaleDateString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
  })
})

const monthLabel = computed(() =>
  currentMonth.value.toLocaleDateString('zh-CN', {
    year: 'numeric',
    month: 'long',
  })
)

const calendarDays = computed(() => {
  return buildCalendarDays(currentMonth.value, props.modelValue, todayValue, isDateDisabled)
})

const todayDisabled = computed(() => isDateDisabled(todayValue))

function toggle() {
  if (props.disabled) return
  if (open.value) closePanel()
  else openPanel()
}

function openPanel() {
  if (props.disabled || open.value) return
  currentMonth.value = monthStart(parseDateValue(props.modelValue) ?? new Date())
  open.value = true
  nextTick(updatePanelPosition)
  window.addEventListener('resize', updatePanelPosition)
  window.addEventListener('scroll', updatePanelPosition, true)
  document.addEventListener('pointerdown', onDocumentPointerDown, true)
}

function closePanel() {
  if (!open.value) return
  open.value = false
  window.removeEventListener('resize', updatePanelPosition)
  window.removeEventListener('scroll', updatePanelPosition, true)
  document.removeEventListener('pointerdown', onDocumentPointerDown, true)
}

function onDocumentPointerDown(event: PointerEvent) {
  const target = event.target as Node | null
  if (!target) return
  if (rootRef.value?.contains(target) || panelRef.value?.panelElement?.contains(target)) return
  closePanel()
}

function updatePanelPosition() {
  const trigger = triggerRef.value
  if (!trigger) return
  const rect = trigger.getBoundingClientRect()
  const viewportWidth = window.innerWidth
  const viewportHeight = window.innerHeight
  const width = Math.min(304, Math.max(260, viewportWidth - 12))
  const height = 344
  const left = Math.min(Math.max(6, rect.left), Math.max(6, viewportWidth - width - 6))
  const below = viewportHeight - rect.bottom
  const top = below >= height || below >= rect.top
    ? Math.min(viewportHeight - 6, rect.bottom + 4)
    : Math.max(6, rect.top - height - 4)
  panelStyle.value = {
    left: `${Math.round(left)}px`,
    top: `${Math.round(top)}px`,
    width: `${Math.round(width)}px`,
  }
}

function shiftMonth(offset: number) {
  currentMonth.value = new Date(currentMonth.value.getFullYear(), currentMonth.value.getMonth() + offset, 1)
}

function selectDate(value: string) {
  if (isDateDisabled(value)) return
  emit('update:modelValue', value)
  closePanel()
  nextTick(() => triggerRef.value?.focus())
}

function selectToday() {
  selectDate(todayValue)
}

function clearDate() {
  emit('update:modelValue', '')
  closePanel()
  nextTick(() => triggerRef.value?.focus())
}

function onTriggerKeydown(event: KeyboardEvent) {
  if (event.key === 'Enter' || event.key === ' ') {
    event.preventDefault()
    toggle()
    return
  }
  if (event.key === 'Escape') {
    event.preventDefault()
    closePanel()
  }
}

function onPanelKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault()
    closePanel()
    nextTick(() => triggerRef.value?.focus())
  }
}

function isDateDisabled(value: string) {
  if (props.min && value < props.min) return true
  if (props.max && value > props.max) return true
  return false
}

watch(() => props.modelValue, value => {
  const parsed = parseDateValue(value)
  if (parsed) currentMonth.value = monthStart(parsed)
})

onBeforeUnmount(closePanel)
</script>

<style scoped>
.theme-date-input {
  position: relative;
  width: 100%;
  min-width: 0;
}

.td-trigger {
  display: flex;
  width: 100%;
  min-width: 0;
  align-items: center;
  gap: 8px;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  background: var(--color-bg-primary);
  color: var(--color-text-primary);
  cursor: pointer;
  font-family: inherit;
  outline: none;
  padding-right: 32px;
  text-align: left;
  transition: border-color 0.15s, background 0.15s, box-shadow 0.15s;
}

.size-md .td-trigger {
  min-height: 32px;
  padding: 6px 32px 6px 9px;
  font-size: 12px;
}

.size-sm .td-trigger {
  min-height: 28px;
  padding: 5px 30px 5px 8px;
  font-size: 12px;
}

.td-trigger:hover {
  border-color: rgba(255, 255, 255, 0.18);
  background: rgba(255, 255, 255, 0.035);
}

.td-trigger:focus-visible,
.theme-date-input.open .td-trigger {
  border-color: var(--color-accent);
  box-shadow: 0 0 0 2px rgba(41, 98, 255, 0.16);
}

.td-trigger:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.td-calendar-icon {
  position: relative;
  flex: 0 0 auto;
  width: 14px;
  height: 14px;
  border: 1px solid currentColor;
  border-radius: 3px;
  color: var(--color-text-tertiary);
}

.td-calendar-icon::before {
  content: "";
  position: absolute;
  top: 3px;
  left: 0;
  right: 0;
  border-top: 1px solid currentColor;
}

.td-calendar-icon::after {
  content: "";
  position: absolute;
  top: -3px;
  left: 3px;
  width: 8px;
  height: 4px;
  border-left: 1px solid currentColor;
  border-right: 1px solid currentColor;
}

.td-label {
  min-width: 0;
  overflow: hidden;
  color: var(--color-text-primary);
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.td-label.placeholder {
  color: var(--color-text-tertiary);
  font-weight: 500;
}

.td-clear {
  position: absolute;
  top: 50%;
  right: 6px;
  width: 22px;
  height: 22px;
  border: 0;
  border-radius: 4px;
  background: transparent;
  color: var(--color-text-tertiary);
  cursor: pointer;
  font-size: 16px;
  line-height: 20px;
  transform: translateY(-50%);
}

.td-clear:hover {
  background: rgba(255, 255, 255, 0.06);
  color: var(--color-text-primary);
}

</style>
