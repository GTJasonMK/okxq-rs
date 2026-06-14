<template>
  <section class="dc-toolbar">
    <div class="dc-toolbar-main">
      <div class="dc-add">
        <input
          :value="newSymbol"
          class="dc-input"
          placeholder="BTC / BTC-USDT"
          @input="handleSymbolInput"
          @keydown.enter="emit('open-rule-dialog')"
        />
        <button
          class="dc-btn primary"
          :disabled="adding || !canOpenRuleDialog"
          @click="emit('open-rule-dialog')"
        >
          保存
        </button>
      </div>
      <div class="dc-actions">
        <button class="dc-btn" :disabled="loading" @click="emit('load-page-data')">
          {{ loading ? '刷新中' : '刷新状态' }}
        </button>
        <button class="dc-btn" :disabled="guardianRunning" @click="emit('run-guardian')">
          {{ guardianRunning ? '请求中' : '按关注规则补齐' }}
        </button>
      </div>
    </div>
    <div class="dc-stats">
      <span><strong>{{ visibleSymbolsCount }}</strong> 数据库标的</span>
      <span><strong>{{ watchedSymbolsCount }}</strong> 已接管规则</span>
      <span><strong>{{ enabledInstrumentCount }}</strong> 数据目标</span>
      <span><strong>{{ activeJobsCount }}</strong> 活跃任务</span>
      <span><strong>{{ managedPlanLabels }}</strong> 规则周期</span>
    </div>
    <div v-if="message || error" class="dc-feedback" :class="{ error: !!error }">
      {{ error || message }}
    </div>
  </section>
</template>

<script setup lang="ts">
defineProps<{
  newSymbol: string
  adding: boolean
  canOpenRuleDialog: boolean
  loading: boolean
  guardianRunning: boolean
  visibleSymbolsCount: number
  watchedSymbolsCount: number
  enabledInstrumentCount: number
  activeJobsCount: number
  managedPlanLabels: string
  message: string
  error: string
}>()

const emit = defineEmits<{
  'update:new-symbol': [value: string]
  'open-rule-dialog': []
  'load-page-data': []
  'run-guardian': []
}>()

function handleSymbolInput(event: Event) {
  emit('update:new-symbol', (event.target as HTMLInputElement).value.trim())
}
</script>
