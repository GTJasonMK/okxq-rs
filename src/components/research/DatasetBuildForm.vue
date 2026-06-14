<template>
  <div class="dp-build">
    <div class="dp-field">
      <label>品种</label>
      <input v-model="form.inst_id" class="dp-input" placeholder="BTC-USDT" />
    </div>
    <div class="dp-row">
      <div class="dp-field">
        <label>市场</label>
        <ThemeSelect
          :model-value="form.inst_type"
          :options="RESEARCH_MARKET_TYPE_OPTIONS"
          size="sm"
          @update:model-value="form.inst_type = $event as Extract<InstType, 'SPOT' | 'SWAP'>"
        />
      </div>
      <div class="dp-field">
        <label>周期</label>
        <ThemeSelect
          :model-value="form.timeframe"
          :options="RESEARCH_TIMEFRAME_OPTIONS"
          size="sm"
          @update:model-value="form.timeframe = $event as Timeframe"
        />
      </div>
    </div>
    <div class="dp-field">
      <label>K线数量</label>
      <input v-model.number="form.bar_count" class="dp-input" type="number" />
    </div>
    <button class="btn" @click="handleBuild">构建数据集</button>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import * as marketApi from '@/api/market'
import ThemeSelect from '@/components/shared/ThemeSelect.vue'
import type { InstType, Timeframe } from '@/types'
import {
  RESEARCH_MARKET_TYPE_OPTIONS,
  RESEARCH_TIMEFRAME_OPTIONS,
} from '@/utils/researchScope'

const emit = defineEmits<{ build: [params: Record<string, unknown>] }>()

const form = ref({
  inst_id: '',
  inst_type: 'SWAP' as Extract<InstType, 'SPOT' | 'SWAP'>,
  timeframe: '1H' as Timeframe,
  bar_count: 1000,
})

async function loadDefaultScope() {
  const scope = await marketApi.fetchDefaultWatchScope({
    symbol: form.value.inst_id,
    inst_type: form.value.inst_type,
  })
  if (!scope) return
  form.value.inst_id = scope.symbol
  form.value.inst_type = scope.inst_type
}

function handleBuild() {
  emit('build', { ...form.value })
}

onMounted(() => {
  loadDefaultScope()
})
</script>

<style scoped>
.dp-build { padding: 10px 12px; border-bottom: 1px solid var(--color-border); }
.dp-row { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; }
.dp-field { margin-bottom: 6px; }
.dp-field label { display: block; font-size: 11px; color: var(--color-text-tertiary); margin-bottom: 2px; }
.dp-input {
  width: 100%;
  padding: 4px 8px;
  background: var(--color-bg-primary);
  border: 1px solid var(--color-border);
  border-radius: 3px;
  color: var(--color-text-primary);
  font-size: 12px;
}
.btn {
  margin-top: 6px;
  padding: 4px 12px;
  background: var(--color-accent);
  border: none;
  border-radius: 4px;
  color: #fff;
  font-size: 12px;
  cursor: pointer;
}
</style>
