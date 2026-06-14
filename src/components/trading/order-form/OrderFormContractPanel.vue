<template>
  <div class="contract-panel">
    <div class="contract-header">
      <span>合约参数</span>
      <button class="mini-btn" type="button" :disabled="contractMetaLoading" @click="emit('refreshContractMeta')">
        {{ contractMetaLoading ? '刷新中...' : '刷新' }}
      </button>
    </div>
    <div class="contract-meta">
      <span>持仓模式：{{ positionModeLabel }}</span>
      <span v-if="currentLeverage > 0">当前杠杆：{{ currentLeverage }}x</span>
    </div>
    <div class="contract-intents">
      <button
        v-for="option in contractIntentOptions"
        :key="option.value"
        type="button"
        class="intent-btn"
        :class="{
          active: option.side === form.side &&
            (!isLongShortMode || option.pos_side === form.pos_side) &&
            option.reduce_only === form.reduce_only,
        }"
        @click="emit('applyContractIntent', option.value)"
      >
        {{ option.label }}
      </button>
    </div>
    <div class="of-row two">
      <div class="of-field">
        <label>保证金模式</label>
        <ThemeSelect
          v-model="tdModeValue"
          class="of-select"
          :options="tdModeOptions"
        />
      </div>
      <div class="of-field">
        <label>杠杆倍数</label>
        <input v-model.number="form.lever" type="number" step="1" min="1" max="125" class="of-input" />
      </div>
    </div>
    <div v-if="isLongShortMode" class="of-field">
      <label>持仓方向</label>
      <ThemeSelect
        v-model="positionSideValue"
        class="of-select"
        :options="positionSideOptions"
      />
    </div>
    <div v-else class="of-help">
      单向持仓账户不传 posSide；买入增加多头/减少空头，卖出增加空头/减少多头。
    </div>
    <div class="contract-actions">
      <label class="of-check">
        <input v-model="form.sync_leverage" type="checkbox" />
        <span>下单前同步杠杆</span>
      </label>
      <label class="of-check">
        <input v-model="form.reduce_only" type="checkbox" />
        <span>只减仓</span>
      </label>
      <button
        class="mini-btn primary"
        type="button"
        :disabled="modeLocked || leverageApplying || form.lever < 1"
        @click="emit('applyLeverage')"
      >
        {{ leverageApplying ? '设置中...' : '设置杠杆' }}
      </button>
    </div>
    <div v-if="leverageMessage" class="of-success">{{ leverageMessage }}</div>
    <div v-if="contractMetaError" class="of-warn">{{ contractMetaError }}</div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import ThemeSelect from '@/components/shared/ThemeSelect.vue'
import type {
  ContractIntentOption,
  ContractOrderIntent,
  OrderFormDraft,
  SelectOption,
} from './types'

const props = defineProps<{
  contractIntentOptions: ContractIntentOption[]
  contractMetaError: string | null
  contractMetaLoading: boolean
  currentLeverage: number
  form: OrderFormDraft
  isLongShortMode: boolean
  leverageApplying: boolean
  leverageMessage: string | null
  modeLocked: boolean
  positionModeLabel: string
  positionSide: string
  positionSideOptions: SelectOption[]
  tdMode: string
  tdModeOptions: SelectOption[]
}>()

const emit = defineEmits<{
  applyContractIntent: [value: ContractOrderIntent]
  applyLeverage: []
  refreshContractMeta: []
  'update:positionSide': [value: string]
  'update:tdMode': [value: string]
}>()

const tdModeValue = computed({
  get: () => props.tdMode,
  set: value => emit('update:tdMode', value),
})

const positionSideValue = computed({
  get: () => props.positionSide,
  set: value => emit('update:positionSide', value),
})
</script>
