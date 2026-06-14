<template>
  <div class="tr-list">
    <div class="tr-header">
      <span class="tr-title">训练记录 ({{ runs.length }})</span>
      <button class="btn" @click="$emit('train')" :disabled="!hasDataset">训练模型</button>
    </div>
    <div class="tr-wrap">
      <table v-if="runs.length > 0">
        <thead>
          <tr>
            <th>ID</th>
            <th>数据集</th>
            <th class="num">R²</th>
            <th class="num">MSE</th>
            <th class="num">MAE</th>
            <th>时间</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(r, i) in runs" :key="i" @click="$emit('select', r)">
            <td class="id-cell">{{ String(r.id || '').slice(0, 8) }}</td>
            <td>{{ String((r as Record<string, unknown>).dataset_id || '--').slice(0, 8) }}</td>
            <td class="num">{{ fmtNum((r as Record<string, unknown>).r2) }}</td>
            <td class="num">{{ fmtNum((r as Record<string, unknown>).mse) }}</td>
            <td class="num">{{ fmtNum((r as Record<string, unknown>).mae) }}</td>
            <td class="time-cell">{{ fmtTime((r as Record<string, unknown>).created_at) }}</td>
          </tr>
        </tbody>
      </table>
      <div v-else class="empty-text">暂无训练记录</div>
    </div>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  runs: Array<Record<string, unknown>>
  hasDataset: boolean
}>()

defineEmits<{
  train: []
  select: [run: Record<string, unknown>]
}>()

function fmtNum(v: unknown): string {
  return typeof v === 'number' ? v.toFixed(4) : '--'
}

function fmtTime(ts: unknown): string {
  if (typeof ts === 'string') return new Date(ts).toLocaleString('zh-CN', { hour12: false })
  return '--'
}
</script>

<style scoped>
.tr-list {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
}
.tr-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border);
}
.tr-title { font-size: 13px; font-weight: 600; }
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
.tr-wrap { overflow-x: auto; }
table { width: 100%; border-collapse: collapse; font-size: 12px; }
th {
  text-align: left;
  padding: 6px 8px;
  color: var(--color-text-tertiary);
  font-weight: 500;
  font-size: 11px;
}
th.num { text-align: right; }
td {
  padding: 4px 8px;
  border-top: 1px solid var(--color-border);
  white-space: nowrap;
}
td.num { text-align: right; font-variant-numeric: tabular-nums; }
.id-cell { font-family: monospace; font-size: 11px; }
.time-cell { font-size: 11px; color: var(--color-text-secondary); }
tr:hover { background: var(--color-bg-hover); cursor: pointer; }
.empty-text {
  padding: 24px;
  text-align: center;
  color: var(--color-text-tertiary);
  font-size: 13px;
}
</style>
