<template>
  <div class="asset-holdings">
    <div class="ah-header">
      <span class="ah-title">剩余资产 ({{ visibleAssets.length }})</span>
      <span class="ah-subtitle">按 OKX 资产页口径显示 USD</span>
    </div>
    <div class="table-wrap">
      <table v-if="visibleAssets.length > 0">
        <thead>
          <tr>
            <th>币种</th>
            <th class="num">权益</th>
            <th class="num">占用</th>
            <th class="num">可用</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="asset in visibleAssets" :key="asset.ccy">
            <td class="asset-cell">{{ asset.ccy }}</td>
            <td class="num balance-cell">{{ formatAssetUsd(assetEquityUsd(asset)) }}</td>
            <td class="num">{{ formatAssetUsd(assetFrozenUsd(asset)) }}</td>
            <td class="num">{{ formatAssetUsd(assetAvailableUsd(asset)) }}</td>
          </tr>
        </tbody>
      </table>
      <div v-else class="empty-text">暂无账户资产</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { AccountAsset } from '@/types'
import {
  assetAvailableUsd,
  assetEquityUsd,
  assetFrozenUsd,
  formatAssetUsd,
  hasVisibleAssetBalance,
} from '@/utils/accountAssets'

const props = defineProps<{
  assets: AccountAsset[]
}>()

const visibleAssets = computed(() =>
  [...props.assets]
    .filter(hasVisibleAssetBalance)
    .sort(compareAssets),
)

function compareAssets(left: AccountAsset, right: AccountAsset): number {
  const equityDiff = Math.abs(assetEquityUsd(right) ?? 0) - Math.abs(assetEquityUsd(left) ?? 0)
  if (equityDiff !== 0) return equityDiff
  const leftStableRank = stableRank(left.ccy)
  const rightStableRank = stableRank(right.ccy)
  if (leftStableRank !== rightStableRank) return leftStableRank - rightStableRank
  return left.ccy.localeCompare(right.ccy)
}

function stableRank(ccy: string): number {
  if (ccy === 'USDT') return 0
  if (ccy === 'USDC') return 1
  return 2
}

</script>

<style scoped>
.asset-holdings {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  min-height: 0;
}
.ah-header {
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.ah-title { font-size: 13px; font-weight: 600; }
.ah-subtitle {
  font-size: 11px;
  color: var(--color-text-tertiary);
  white-space: nowrap;
}
.table-wrap {
  overflow: auto;
  min-height: 0;
  max-height: 220px;
}
table { width: 100%; border-collapse: collapse; font-size: 12px; }
th {
  text-align: left;
  padding: 6px 10px;
  color: var(--color-text-tertiary);
  font-weight: 500;
  font-size: 11px;
  white-space: nowrap;
}
th.num { text-align: right; }
td {
  padding: 5px 10px;
  border-top: 1px solid var(--color-border);
  white-space: nowrap;
}
td.num { text-align: right; font-variant-numeric: tabular-nums; }
.asset-cell { font-weight: 600; }
.balance-cell { font-weight: 600; color: var(--color-text-primary); }
.empty-text {
  padding: 24px;
  text-align: center;
  color: var(--color-text-tertiary);
  font-size: 13px;
}
</style>
