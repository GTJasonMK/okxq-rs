<template>
  <div class="rc-metrics">
    <div class="metric">
      <span class="metric-label">收益率</span>
      <span class="metric-value" :class="pnlColor(result.total_return_pct)">
        {{ result.total_return_pct?.toFixed(2) }}%
      </span>
    </div>
    <div class="metric">
      <span class="metric-label">夏普比</span>
      <span class="metric-value">{{ result.sharpe_ratio?.toFixed(2) }}</span>
    </div>
    <div class="metric">
      <span class="metric-label">最大回撤</span>
      <span class="metric-value negative">{{ result.max_drawdown_pct?.toFixed(2) }}%</span>
    </div>
    <div class="metric">
      <span class="metric-label">胜率</span>
      <span class="metric-value">{{ result.win_rate_pct?.toFixed(1) }}%</span>
    </div>
    <div class="metric">
      <span class="metric-label">平仓笔数</span>
      <span class="metric-value">{{ result.total_trades }}</span>
    </div>
    <div class="metric">
      <span class="metric-label">盈亏比</span>
      <span class="metric-value">{{ result.profit_factor?.toFixed(2) }}</span>
    </div>
    <div class="metric">
      <span class="metric-label">初始资金</span>
      <span class="metric-value">{{ formatMoney(result.initial_capital) }}</span>
    </div>
    <div class="metric">
      <span class="metric-label">最终权益</span>
      <span class="metric-value">{{ formatMoney(result.final_equity) }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { BacktestResult } from '@/types'
import { pnlColor } from '@/utils/color'
import { formatMoney } from '@/utils/format'

defineProps<{
  result: BacktestResult
}>()
</script>

<style scoped>
.rc-metrics {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(112px, 1fr));
  gap: 7px 14px;
  padding: 10px 12px;
}
.metric {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}
.metric-label {
  color: var(--color-text-tertiary);
  font-size: 11px;
}
.metric-value {
  font-size: 14px;
  font-variant-numeric: tabular-nums;
  font-weight: 600;
  line-height: 1.2;
}
.positive { color: var(--color-positive); }
.negative { color: var(--color-negative); }
</style>
