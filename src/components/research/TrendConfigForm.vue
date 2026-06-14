<template>
  <div class="tcf">
    <div class="tcf-header">
      <span class="tcf-title">趋势研究配置</span>
      <button class="btn" @click="handleSave">保存</button>
    </div>
    <div class="tcf-body">
      <div class="tcf-field">
        <label>品种</label>
        <input v-model="local.symbol" class="tcf-input" />
      </div>
      <div class="tcf-row">
        <div class="tcf-field">
          <label>市场</label>
          <ThemeSelect
            :model-value="local.inst_type"
            :options="RESEARCH_MARKET_TYPE_OPTIONS"
            @update:model-value="local.inst_type = $event as Extract<InstType, 'SPOT' | 'SWAP'>"
          />
        </div>
        <div class="tcf-field">
          <label>周期</label>
          <ThemeSelect
            :model-value="local.timeframe"
            :options="RESEARCH_TIMEFRAME_OPTIONS"
            @update:model-value="local.timeframe = $event as Timeframe"
          />
        </div>
      </div>
      <div class="tcf-field">
        <label>K线数量</label>
        <input v-model.number="local.bar_count" class="tcf-input" type="number" />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { reactive, watch } from 'vue'
import type { InstType, Timeframe } from '@/types'
import ThemeSelect from '@/components/shared/ThemeSelect.vue'
import {
  RESEARCH_MARKET_TYPE_OPTIONS,
  RESEARCH_TIMEFRAME_OPTIONS,
} from '@/utils/researchScope'

const props = defineProps<{
  config: {
    symbol: string
    inst_type: Extract<InstType, 'SPOT' | 'SWAP'>
    timeframe: Timeframe
    bar_count: number
  }
}>()
const emit = defineEmits<{ save: [config: Record<string, unknown>] }>()

const local = reactive({ ...props.config })

watch(() => props.config, (v) => { Object.assign(local, v) })

function handleSave() {
  emit('save', { ...local })
}
</script>

<style scoped>
.tcf {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
}
.tcf-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border);
}
.tcf-title { font-size: 13px; font-weight: 600; }
.btn {
  padding: 4px 12px;
  background: var(--color-accent);
  border: none;
  border-radius: 4px;
  color: #fff;
  font-size: 12px;
  cursor: pointer;
}
.tcf-body { padding: 12px; }
.tcf-row { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
.tcf-field { margin-bottom: 8px; }
.tcf-field label { display: block; font-size: 11px; color: var(--color-text-tertiary); margin-bottom: 3px; }
.tcf-input {
  width: 100%;
  padding: 5px 8px;
  background: var(--color-bg-primary);
  border: 1px solid var(--color-border);
  border-radius: 4px;
  color: var(--color-text-primary);
  font-size: 12px;
}
</style>
