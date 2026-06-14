<template>
  <div class="order-form">
    <OrderFormHeader
      :mode-label="modeLabel"
      :mode-locked="modeLocked"
      :resolved-mode="resolvedMode"
    />
    <div class="of-body">
      <OrderFormInstrumentSection
        v-model:manual-symbol="manualSymbolModel"
        v-model:market-type="marketTypeModel"
        v-model:selected-scope="selectedScopeModel"
        :can-hide-manual-symbol-input="canHideManualSymbolInput"
        :form-inst-id="form.inst_id"
        :loading-scopes="loadingScopes"
        :market-type-options="marketTypeOptions"
        :order-inst-id="orderInstId"
        :scope-options="scopeOptions"
        :show-manual-symbol-input="showManualSymbolInput"
        @hide-manual-symbol-editor="hideManualSymbolEditor"
        @refresh-contract-meta="refreshContractMeta"
        @show-manual-symbol-editor="showManualSymbolEditor"
      />
      <OrderFormContractPanel
        v-if="isContract"
        v-model:position-side="positionSideModel"
        v-model:td-mode="tdModeModel"
        :contract-intent-options="contractIntentOptions"
        :contract-meta-error="contractMetaError"
        :contract-meta-loading="contractMetaLoading"
        :current-leverage="currentLeverage"
        :form="form"
        :is-long-short-mode="isLongShortMode"
        :leverage-applying="leverageApplying"
        :leverage-message="leverageMessage"
        :mode-locked="modeLocked"
        :position-mode-label="positionModeLabel"
        :position-side-options="positionSideOptions"
        :td-mode-options="tdModeOptions"
        @apply-contract-intent="applyContractIntent"
        @apply-leverage="() => applyLeverage()"
        @refresh-contract-meta="refreshContractMeta"
      />
      <OrderFormTradeFields
        :can-submit="canSubmit"
        :error="error"
        :form="form"
        :is-contract="isContract"
        :order-action-label="orderActionLabel"
        :order-types="orderTypes"
        :show-side-toggle="showSideToggle"
        :side-options="sideOptions"
        :submit-label="submitLabel"
        :submitting="submitting"
        @submit="submit"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { useOrderForm } from '@/composables/useOrderForm'
import type { TradingMode } from '@/types'
import OrderFormContractPanel from './order-form/OrderFormContractPanel.vue'
import OrderFormHeader from './order-form/OrderFormHeader.vue'
import OrderFormInstrumentSection from './order-form/OrderFormInstrumentSection.vue'
import OrderFormTradeFields from './order-form/OrderFormTradeFields.vue'
import './order-form/styles.css'

const props = defineProps<{
  mode?: TradingMode
  modeLocked?: boolean
}>()
const emit = defineEmits<{ submitted: [] }>()
const {
  resolvedMode,
  modeLocked,
  form,
  submitting,
  leverageApplying,
  contractMetaLoading,
  loadingScopes,
  error,
  contractMetaError,
  leverageMessage,
  selectedScopeModel,
  manualSymbolModel,
  showManualSymbolInput,
  canHideManualSymbolInput,
  scopeOptions,
  marketTypeOptions,
  marketTypeModel,
  orderTypes,
  sideOptions,
  showSideToggle,
  tdModeOptions,
  tdModeModel,
  positionSideOptions,
  positionSideModel,
  contractIntentOptions,
  canSubmit,
  isContract,
  isLongShortMode,
  currentLeverage,
  orderInstId,
  modeLabel,
  positionModeLabel,
  orderActionLabel,
  submitLabel,
  refreshContractMeta,
  applyLeverage,
  applyContractIntent,
  showManualSymbolEditor,
  hideManualSymbolEditor,
  submit,
} = useOrderForm({
  mode: () => props.mode,
  modeLocked: () => props.modeLocked === true,
  onSubmitted: () => emit('submitted'),
})
</script>
