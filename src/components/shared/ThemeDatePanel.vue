<template>
  <div
    ref="panelRef"
    class="td-panel"
    :style="panelStyle"
    role="dialog"
    aria-label="选择日期"
    @keydown="emit('panelKeydown', $event)"
  >
    <div class="td-panel-head">
      <button type="button" class="td-nav" aria-label="上个月" @click="emit('shiftMonth', -1)">‹</button>
      <strong>{{ monthLabel }}</strong>
      <button type="button" class="td-nav" aria-label="下个月" @click="emit('shiftMonth', 1)">›</button>
    </div>
    <div class="td-weekdays">
      <span v-for="day in weekdays" :key="day">{{ day }}</span>
    </div>
    <div class="td-grid">
      <button
        v-for="day in calendarDays"
        :key="day.value"
        type="button"
        class="td-day"
        :class="{
          muted: !day.inMonth,
          today: day.today,
          selected: day.selected,
        }"
        :disabled="day.disabled"
        @click="emit('selectDate', day.value)"
      >
        {{ day.label }}
      </button>
    </div>
    <div class="td-actions">
      <button type="button" class="td-action" @click="emit('clearDate')">清空</button>
      <button type="button" class="td-action" :disabled="todayDisabled" @click="emit('selectToday')">今天</button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import type { ThemeDateCalendarDay } from '@/utils/themeDateInput'

defineProps<{
  calendarDays: ThemeDateCalendarDay[]
  monthLabel: string
  panelStyle: Record<string, string>
  todayDisabled: boolean
  weekdays: string[]
}>()

const emit = defineEmits<{
  clearDate: []
  panelKeydown: [event: KeyboardEvent]
  selectDate: [value: string]
  selectToday: []
  shiftMonth: [offset: number]
}>()

const panelRef = ref<HTMLElement | null>(null)
const panelElement = computed(() => panelRef.value)

defineExpose({
  panelElement,
})
</script>

<style scoped>
.td-panel {
  position: fixed;
  z-index: 3100;
  box-sizing: border-box;
  padding: 10px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: var(--color-bg-secondary);
  box-shadow: 0 18px 44px rgba(0, 0, 0, 0.42);
}

.td-panel-head {
  display: grid;
  grid-template-columns: 32px minmax(0, 1fr) 32px;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}

.td-panel-head strong {
  min-width: 0;
  color: var(--color-text-primary);
  font-size: 13px;
  font-weight: 700;
  text-align: center;
}

.td-nav,
.td-action {
  border: 1px solid var(--color-border);
  border-radius: 4px;
  background: var(--color-bg-primary);
  color: var(--color-text-secondary);
  cursor: pointer;
  font-family: inherit;
}

.td-nav {
  width: 32px;
  height: 30px;
  font-size: 18px;
  line-height: 1;
}

.td-nav:hover,
.td-action:hover:not(:disabled) {
  border-color: rgba(41, 98, 255, 0.45);
  color: var(--color-text-primary);
}

.td-weekdays,
.td-grid {
  display: grid;
  grid-template-columns: repeat(7, minmax(0, 1fr));
  gap: 4px;
}

.td-weekdays {
  margin-bottom: 5px;
}

.td-weekdays span {
  color: var(--color-text-tertiary);
  font-size: 11px;
  font-weight: 600;
  line-height: 22px;
  text-align: center;
}

.td-day {
  aspect-ratio: 1;
  min-width: 0;
  border: 1px solid transparent;
  border-radius: 4px;
  background: transparent;
  color: var(--color-text-secondary);
  cursor: pointer;
  font-family: inherit;
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}

.td-day:hover:not(:disabled) {
  border-color: rgba(41, 98, 255, 0.34);
  background: rgba(41, 98, 255, 0.12);
  color: var(--color-text-primary);
}

.td-day.muted {
  color: var(--color-text-tertiary);
  opacity: 0.62;
}

.td-day.today {
  border-color: rgba(38, 166, 154, 0.45);
  color: var(--color-positive);
}

.td-day.selected {
  border-color: var(--color-accent);
  background: var(--color-accent);
  color: #fff;
  font-weight: 700;
}

.td-day:disabled {
  cursor: not-allowed;
  opacity: 0.28;
}

.td-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 10px;
  padding-top: 10px;
  border-top: 1px solid var(--color-border);
}

.td-action {
  min-width: 56px;
  min-height: 28px;
  font-size: 12px;
  font-weight: 600;
}

.td-action:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}
</style>
