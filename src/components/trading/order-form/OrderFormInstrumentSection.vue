<template>
  <div class="of-row two">
    <div class="of-field">
      <label>市场类型</label>
      <ThemeSelect
        v-model="marketTypeValue"
        class="of-select"
        :options="marketTypeOptions"
      />
    </div>
    <div class="of-field">
      <div class="of-label-line">
        <label>交易品种</label>
        <button
          v-if="!showManualSymbolInput"
          class="of-link-btn"
          type="button"
          @click="emit('showManualSymbolEditor')"
        >
          手动输入
        </button>
        <button
          v-else-if="canHideManualSymbolInput"
          class="of-link-btn"
          type="button"
          @click="emit('hideManualSymbolEditor')"
        >
          收起
        </button>
      </div>
      <ThemeSelect
        v-model="selectedScopeValue"
        class="of-select"
        :options="scopeOptions"
        :disabled="loadingScopes || scopeOptions.length === 0"
        :placeholder="loadingScopes ? '读取中...' : '无关注品种'"
      />
      <span v-if="orderInstId" class="of-help">下单：{{ orderInstId }}</span>
    </div>
  </div>
  <div v-if="showManualSymbolInput" class="of-field manual-symbol-field">
    <label>自定义 instId</label>
    <input
      v-model="manualSymbolValue"
      placeholder="BTC-USDT-SWAP"
      class="of-input"
      @blur="emit('refreshContractMeta')"
    />
    <span v-if="orderInstId && orderInstId !== formInstId" class="of-help">
      实际下单品种：{{ orderInstId }}
    </span>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import ThemeSelect from '@/components/shared/ThemeSelect.vue'
import type { SelectOption } from './types'

const props = defineProps<{
  canHideManualSymbolInput: boolean
  formInstId: string
  loadingScopes: boolean
  manualSymbol: string
  marketType: string
  marketTypeOptions: SelectOption[]
  orderInstId: string
  scopeOptions: SelectOption[]
  selectedScope: string
  showManualSymbolInput: boolean
}>()

const emit = defineEmits<{
  hideManualSymbolEditor: []
  refreshContractMeta: []
  showManualSymbolEditor: []
  'update:manualSymbol': [value: string]
  'update:marketType': [value: string]
  'update:selectedScope': [value: string]
}>()

const marketTypeValue = computed({
  get: () => props.marketType,
  set: value => emit('update:marketType', value),
})

const selectedScopeValue = computed({
  get: () => props.selectedScope,
  set: value => emit('update:selectedScope', value),
})

const manualSymbolValue = computed({
  get: () => props.manualSymbol,
  set: value => emit('update:manualSymbol', value),
})
</script>
