<template>
  <div class="tf-panel">
    <div class="tf-header">
      <span class="tf-title">因子计算</span>
      <button class="btn" @click="$emit('compute')" :disabled="computing">{{ computing ? '计算中...' : '计算因子' }}</button>
    </div>
    <div class="tf-list" v-if="factors.length > 0">
      <div v-for="f in factors" :key="f.name" class="tf-item">
        <span class="tf-name">{{ f.name }}</span>
        <span class="tf-value" :class="pnlColor(f.value)">{{ fmtVal(f.value) }}</span>
      </div>
    </div>
    <div v-else class="empty-text">{{ computing ? '正在计算因子...' : '点击"计算因子"开始' }}</div>
  </div>
</template>

<script setup lang="ts">
import { pnlColor } from '@/utils/color'
import { formatPercent } from '@/utils/format'

defineProps<{
  factors: Array<{ name: string; value: number }>
  computing: boolean
}>()

defineEmits<{ compute: [] }>()

function fmtVal(v: number): string {
  if (Math.abs(v) < 1) return formatPercent(v)
  return v.toFixed(4)
}
</script>

<style scoped>
.tf-panel {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
}
.tf-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border);
}
.tf-title { font-size: 13px; font-weight: 600; }
.btn {
  padding: 4px 12px;
  background: var(--color-accent);
  border: none;
  border-radius: 4px;
  color: #fff;
  font-size: 12px;
  cursor: pointer;
}
.btn:disabled { opacity: 0.5; cursor: not-allowed; }
.tf-list { padding: 8px 12px; }
.tf-item {
  display: flex;
  justify-content: space-between;
  padding: 4px 0;
  border-bottom: 1px solid var(--color-border);
}
.tf-item:last-child { border-bottom: none; }
.tf-name { font-size: 12px; color: var(--color-text-secondary); }
.tf-value { font-size: 13px; font-weight: 600; font-variant-numeric: tabular-nums; }
.positive { color: var(--color-positive); }
.negative { color: var(--color-negative); }
.empty-text {
  padding: 20px;
  text-align: center;
  color: var(--color-text-tertiary);
  font-size: 13px;
}
</style>
