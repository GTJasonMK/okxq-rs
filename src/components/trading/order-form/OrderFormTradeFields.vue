<template>
  <div v-if="showSideToggle" class="side-toggle">
    <button
      v-for="option in sideOptions"
      :key="option.value"
      class="side-btn"
      :class="[option.value, { active: form.side === option.value }]"
      type="button"
      @click="form.side = option.value"
    >{{ option.label }}</button>
  </div>
  <div class="type-toggle">
    <button
      v-for="type in orderTypes"
      :key="type.value"
      class="type-btn"
      :class="{ active: form.ord_type === type.value }"
      type="button"
      @click="form.ord_type = type.value"
    >{{ type.label }}</button>
  </div>
  <div class="of-field">
    <label>数量</label>
    <input v-model.number="form.sz" type="number" step="0.01" min="0" placeholder="0.01" class="of-input" />
  </div>
  <div v-if="form.ord_type === 'limit'" class="of-field">
    <label>限价</label>
    <input v-model.number="form.px" type="number" step="0.01" min="0" placeholder="0.00" class="of-input" />
  </div>
  <div v-if="isContract" class="order-preview">
    <span>本次操作：{{ orderActionLabel }}</span>
    <span>{{ form.td_mode === 'cross' ? '全仓' : '逐仓' }} · {{ form.lever }}x</span>
  </div>
  <button class="submit-btn" :class="form.side" :disabled="!canSubmit || submitting" @click="emit('submit')">
    {{ submitting ? '提交中...' : submitLabel }}
  </button>
  <div v-if="error" class="of-error">{{ error }}</div>
</template>

<script setup lang="ts">
import type { OrderFormDraft, OrderTypeOption, SideOption } from './types'

defineProps<{
  canSubmit: boolean
  error: string | null
  form: OrderFormDraft
  isContract: boolean
  orderActionLabel: string
  orderTypes: OrderTypeOption[]
  showSideToggle: boolean
  sideOptions: SideOption[]
  submitLabel: string
  submitting: boolean
}>()

const emit = defineEmits<{
  submit: []
}>()
</script>
