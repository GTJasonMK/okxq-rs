<template>
  <div class="model-metrics" v-if="run">
    <div class="mm-header">
      <span class="mm-title">模型评估</span>
    </div>
    <div class="mm-cards">
      <div class="mm-card">
        <div class="mm-label">R²</div>
        <div class="mm-value">{{ fmtNum(run.r2) }}</div>
      </div>
      <div class="mm-card">
        <div class="mm-label">MSE</div>
        <div class="mm-value">{{ fmtNum(run.mse) }}</div>
      </div>
      <div class="mm-card">
        <div class="mm-label">MAE</div>
        <div class="mm-value">{{ fmtNum(run.mae) }}</div>
      </div>
      <div class="mm-card">
        <div class="mm-label">方向准确率</div>
        <div class="mm-value">{{ fmtPct(run.direction_accuracy) }}</div>
      </div>
    </div>
  </div>
  <div class="model-metrics empty" v-else>
    <div class="mm-header"><span class="mm-title">模型评估</span></div>
    <div class="empty-text">选择一个训练结果查看详情</div>
  </div>
</template>

<script setup lang="ts">
import { formatPercent } from '@/utils/format'

defineProps<{ run: Record<string, unknown> | null }>()

function fmtNum(v: unknown): string {
  return typeof v === 'number' ? v.toFixed(4) : '--'
}

function fmtPct(v: unknown): string {
  return typeof v === 'number' ? formatPercent(v) : '--'
}
</script>

<style scoped>
.model-metrics {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
}
.mm-header {
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border);
}
.mm-title { font-size: 13px; font-weight: 600; }
.mm-cards { display: grid; grid-template-columns: repeat(4, 1fr); gap: 8px; padding: 10px 12px; }
.mm-card {
  text-align: center;
  padding: 8px;
  background: var(--color-bg-primary);
  border-radius: 4px;
}
.mm-label { font-size: 10px; color: var(--color-text-tertiary); margin-bottom: 4px; }
.mm-value { font-size: 16px; font-weight: 700; font-variant-numeric: tabular-nums; }
.empty-text {
  padding: 20px;
  text-align: center;
  color: var(--color-text-tertiary);
  font-size: 13px;
}
</style>
