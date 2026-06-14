<template>
  <section class="vl-run-summary-panel">
    <div class="vl-run-summary-title">运行摘要</div>
    <div class="vl-kpi-grid">
      <div
        v-for="item in items"
        :key="item.label"
        class="vl-kpi-card"
        :class="item.kind"
      >
        <span>{{ item.label }}</span>
        <strong :title="item.value">{{ item.value }}</strong>
        <em :title="item.detail">{{ item.detail }}</em>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import type { LiveStrategyKpi } from '@/utils/liveStrategyDisplay'

defineProps<{
  items: LiveStrategyKpi[]
}>()
</script>

<style scoped>
.vl-run-summary-panel {
  min-width: 0;
  padding: 8px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: var(--color-bg-secondary);
}
.vl-run-summary-title {
  margin-bottom: 8px;
  color: var(--color-text-primary);
  font-size: 13px;
  font-weight: 700;
}
.vl-kpi-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 6px;
}
.vl-kpi-card {
  min-width: 0;
  padding: 8px;
  border: 1px solid rgba(148,163,184,0.18);
  border-radius: 6px;
  background: rgba(148,163,184,0.05);
  font-size: 11px;
  line-height: 1.35;
}
.vl-kpi-card span,
.vl-kpi-card em {
  display: block;
  min-width: 0;
  overflow: hidden;
  color: var(--color-text-tertiary);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.vl-kpi-card strong {
  display: block;
  min-width: 0;
  margin: 2px 0;
  overflow: hidden;
  color: var(--color-text-primary);
  font-size: 15px;
  font-weight: 700;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.vl-kpi-card.ready strong,
.vl-kpi-card.positive strong { color: var(--color-positive); }
.vl-kpi-card.negative strong,
.vl-kpi-card.blocked strong { color: var(--color-negative); }
.vl-kpi-card.warning strong { color: var(--color-warning); }

@media (max-width: 1100px) {
  .vl-kpi-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (max-width: 720px) {
  .vl-kpi-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
</style>
