<template>
  <div class="account-summary">
    <div class="summary-card main">
      <span class="summary-label">账户总资产</span>
      <span class="summary-value large">{{ formatAssetUsd(totalAccountEquityUsd) }}</span>
    </div>
    <div class="summary-card">
      <span class="summary-label">USDT 可用</span>
      <span class="summary-value">{{ formatAssetUsd(usdtAvailableUsd) }}</span>
    </div>
    <div class="summary-card">
      <span class="summary-label">资产币种数</span>
      <span class="summary-value">{{ assetCount }}</span>
    </div>
    <div class="summary-card">
      <span class="summary-label">持仓保证金</span>
      <span class="summary-value">{{ formatMoney(positionMargin) }}</span>
    </div>
    <div class="summary-card">
      <span class="summary-label">未实现盈亏</span>
      <span class="summary-value" :class="pnlColor(unrealizedPnl)">{{ formatMoney(unrealizedPnl) }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { AccountInfo, Position } from '@/types'
import { formatMoney } from '@/utils/format'
import { pnlColor } from '@/utils/color'
import {
  accountTotalEquityUsd,
  assetAvailableUsd,
  formatAssetUsd,
  hasVisibleAssetBalance,
} from '@/utils/accountAssets'

const props = defineProps<{
  account: AccountInfo | null
  positions: Position[]
}>()

const assetCount = computed(() =>
  props.account?.details.filter(hasVisibleAssetBalance).length ?? 0
)
const totalAccountEquityUsd = computed(() => accountTotalEquityUsd(props.account))
const usdtAvailableUsd = computed(() => {
  const usdt = props.account?.details.find(asset => asset.ccy === 'USDT')
  return usdt ? assetAvailableUsd(usdt) : 0
})
const unrealizedPnl = computed(() =>
  sumKnown(props.positions.map(position => position.upl))
)
const positionMargin = computed(() =>
  sumKnown(props.positions.map(position => position.margin))
)

function sumKnown(values: Array<number | null>) {
  let found = false
  let total = 0
  for (const value of values) {
    if (!Number.isFinite(value)) continue
    found = true
    total += value as number
  }
  return found ? total : null
}

</script>

<style scoped>
.account-summary {
  display: flex;
  gap: 8px;
  margin-bottom: 8px;
}
.summary-card {
  flex: 1;
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  padding: 10px 14px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.summary-card.main {
  background: linear-gradient(135deg, var(--color-bg-sidebar), var(--color-bg-secondary));
  border-color: var(--color-accent);
}
.summary-label {
  font-size: 11px;
  color: var(--color-text-tertiary);
}
.summary-value {
  font-size: 15px;
  font-weight: 600;
  color: var(--color-text-primary);
}
.summary-value.large { font-size: 20px; }
.positive { color: var(--color-positive); }
.negative { color: var(--color-negative); }
</style>
