<template>
  <div class="scanner-results">
    <div class="sr-header">
      <span class="sr-title">扫描结果 ({{ results.length }})</span>
    </div>
    <div class="sr-table-wrap">
      <table v-if="results.length > 0">
        <thead>
          <tr>
            <th>品种</th>
            <th class="num">得分</th>
            <th>匹配条件</th>
            <th>时间</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="r in results" :key="r.id">
            <td class="symbol-cell">{{ r.symbol }}</td>
            <td class="num">
              <span class="score" :class="scoreClass(r.score)">{{ r.score?.toFixed(1) }}</span>
            </td>
            <td>
              <span class="badge" v-for="c in r.matched_conditions" :key="c">{{ c }}</span>
            </td>
            <td class="time-cell">{{ formatScanTime(r.scanned_at) }}</td>
          </tr>
        </tbody>
      </table>
      <div v-else class="empty-text">选择配置并运行扫描</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { ScannerResult } from '@/types'

defineProps<{ results: ScannerResult[] }>()

function scoreClass(s: number): string {
  if (s >= 80) return 'high'
  if (s >= 60) return 'med'
  return 'low'
}

function formatScanTime(ts: string): string {
  return new Date(ts).toLocaleString('zh-CN', { hour12: false })
}
</script>

<style scoped>
.scanner-results {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
}
.sr-header {
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border);
}
.sr-title { font-size: 13px; font-weight: 600; }
.sr-table-wrap { overflow-x: auto; }
table { width: 100%; border-collapse: collapse; font-size: 12px; }
th {
  text-align: left;
  padding: 6px 10px;
  color: var(--color-text-tertiary);
  font-weight: 500;
  font-size: 11px;
}
th.num { text-align: right; }
td { padding: 5px 10px; border-top: 1px solid var(--color-border); }
td.num { text-align: right; }
.symbol-cell { font-weight: 600; }
.score { font-weight: 700; }
.score.high { color: var(--color-positive); }
.score.med { color: #ff9800; }
.score.low { color: var(--color-text-secondary); }
.badge {
  display: inline-block;
  padding: 0 5px;
  margin: 1px 2px;
  background: rgba(41,98,255,0.12);
  color: var(--color-accent);
  border-radius: 3px;
  font-size: 10px;
}
.time-cell { font-size: 11px; color: var(--color-text-secondary); }
.empty-text {
  padding: 32px;
  text-align: center;
  color: var(--color-text-tertiary);
  font-size: 13px;
}
</style>
