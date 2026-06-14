<template>
  <div class="rolling-panel">
    <div class="rp-table" v-if="metrics && metrics.length > 0">
      <table>
        <thead>
          <tr>
            <th>指标</th>
            <th class="num">均值</th>
            <th class="num">最小值</th>
            <th class="num">最大值</th>
            <th class="num">当前值</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="m in metrics" :key="m.name">
            <td>{{ m.name }}</td>
            <td class="num">{{ fmt(m.mean) }}</td>
            <td class="num" :class="pnlColor(m.min_val)">{{ fmt(m.min_val) }}</td>
            <td class="num" :class="pnlColor(m.max_val)">{{ fmt(m.max_val) }}</td>
            <td class="num" :class="pnlColor(m.current)">{{ fmt(m.current) }}</td>
          </tr>
        </tbody>
      </table>
    </div>
    <div v-else class="empty-text">暂无滚动指标数据</div>
    <!-- YTD 基准曲线 -->
    <div class="rp-ytd" v-if="benchmark && benchmark.length > 0">
      <div class="rp-ytd-title">YTD 基准对比</div>
      <BenchmarkChart :data="benchmark" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { pnlColor } from '@/utils/color'
import { formatPercent } from '@/utils/format'
import BenchmarkChart from './BenchmarkChart.vue'

defineProps<{
  metrics?: Array<{ name: string; mean: number; min_val: number; max_val: number; current: number }>
  benchmark?: Array<{ time: number; value: number }>
}>()

function fmt(v: number): string {
  if (!Number.isFinite(v)) return '--'
  if (Math.abs(v) < 1) return formatPercent(v)
  return v.toFixed(4)
}
</script>

<style scoped>
.rolling-panel { font-size: 12px; }
.rp-table table { width: 100%; border-collapse: collapse; }
.rp-table th {
  text-align: left;
  padding: 4px 6px;
  color: var(--color-text-tertiary);
  font-weight: 500;
  font-size: 10px;
}
.rp-table th.num { text-align: right; }
.rp-table td {
  padding: 3px 6px;
  border-top: 1px solid var(--color-border);
  font-variant-numeric: tabular-nums;
}
.rp-table td.num { text-align: right; }
.rp-ytd { margin-top: 12px; }
.rp-ytd-title { font-size: 12px; font-weight: 600; margin-bottom: 6px; }
.positive { color: var(--color-positive); }
.negative { color: var(--color-negative); }
.empty-text { padding: 24px; text-align: center; color: var(--color-text-tertiary); }
</style>
