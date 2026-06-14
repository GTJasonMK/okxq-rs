<template>
  <div class="view-dashboard">
    <h2 class="dv-title">仪表盘</h2>
    <div v-if="error" class="dv-error">{{ error }}</div>
    <div class="dv-sync" :class="{ ok: privateRealtimeConnected, bad: privateRealtimeError }">
      OKX 私有账户状态通道：
      <span v-if="privateRealtimeConnected">{{ privateRealtimeMode === 'live' ? '实盘' : '模拟盘' }} WebSocket 订阅已启用</span>
      <span v-else-if="privateRealtimeError">WebSocket 异常：{{ privateRealtimeError }}</span>
      <span v-else>连接中...</span>
      <span class="dv-mode">当前账户：{{ viewModeLabel }}</span>
    </div>
    <AccountSummary :account="trading.account" :positions="trading.positions" />
    <div class="dv-cards">
      <div class="dv-card">
        <div class="dv-card-header">账户概览</div>
        <div class="dv-card-body">
          <div class="dv-stat">
            <span class="dv-stat-label">账户总资产</span>
            <span class="dv-stat-value large">{{ formatAssetUsd(totalAccountEquityUsd) }}</span>
          </div>
          <div class="dv-stat">
            <span class="dv-stat-label">USDT 可用</span>
            <span class="dv-stat-value">{{ formatAssetUsd(usdtAvailableUsd) }}</span>
          </div>
          <div class="dv-stat">
            <span class="dv-stat-label">资产币种数</span>
            <span class="dv-stat-value">{{ accountAssetCount }}</span>
          </div>
          <div class="dv-stat">
            <span class="dv-stat-label">合约持仓数</span>
            <span class="dv-stat-value">{{ trading.positions.length }}</span>
          </div>
        </div>
      </div>
      <div class="dv-card">
        <div class="dv-card-header">盈亏概览</div>
        <div class="dv-card-body">
          <div class="dv-stat">
            <span class="dv-stat-label">总盈亏</span>
            <span class="dv-stat-value large" :class="pnlColor(totalPnl)">{{ formatMoney(totalPnl) }}</span>
          </div>
          <div class="dv-stat">
            <span class="dv-stat-label">未实现盈亏</span>
            <span class="dv-stat-value" :class="pnlColor(unrealizedPnl)">{{ formatMoney(unrealizedPnl) }}</span>
          </div>
        </div>
      </div>
      <div class="dv-card">
        <div class="dv-card-header">成交统计</div>
        <div class="dv-card-body">
          <div class="dv-stat">
            <span class="dv-stat-label">今日成交</span>
            <span class="dv-stat-value">{{ trading.fills.length }}</span>
          </div>
          <div class="dv-stat">
            <span class="dv-stat-label">挂单数</span>
            <span class="dv-stat-value">{{ trading.orders.filter(o => o.state === 'live').length }}</span>
          </div>
        </div>
      </div>
    </div>
    <AssetHoldingsTable :assets="trading.account?.details ?? []" />
    <div class="dv-section">
      <div class="dv-card-header">最近持仓</div>
      <PositionTable :positions="trading.positions.slice(0, 5)" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useDashboardView } from '@/composables/useDashboardView'
import { formatMoney } from '@/utils/format'
import { pnlColor } from '@/utils/color'
import {
  accountTotalEquityUsd,
  assetAvailableUsd,
  formatAssetUsd,
  hasVisibleAssetBalance,
} from '@/utils/accountAssets'
import AccountSummary from '@/components/trading/AccountSummary.vue'
import AssetHoldingsTable from '@/components/trading/AssetHoldingsTable.vue'
import PositionTable from '@/components/trading/PositionTable.vue'

defineOptions({ name: 'DashboardView' })

const {
  trading,
  error,
  viewModeLabel,
  privateRealtimeConnected,
  privateRealtimeError,
  privateRealtimeMode,
  unrealizedPnl,
  totalPnl,
} = useDashboardView()

const accountAssetCount = computed(() =>
  trading.account?.details.filter(hasVisibleAssetBalance).length ?? 0
)

const totalAccountEquityUsd = computed(() => accountTotalEquityUsd(trading.account))

const usdtAvailableUsd = computed(() => {
  const usdt = trading.account?.details.find(asset => asset.ccy === 'USDT')
  return usdt ? assetAvailableUsd(usdt) : 0
})
</script>

<style scoped>
.view-dashboard { display: flex; flex-direction: column; gap: 12px; }
.dv-title { font-size: 16px; font-weight: 600; margin: 0 0 4px; }
.dv-error {
  padding: 8px 10px;
  border: 1px solid rgba(239,83,80,0.35);
  border-radius: 6px;
  background: rgba(239,83,80,0.08);
  color: var(--color-negative);
  font-size: 12px;
}
.dv-sync {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  padding: 7px 10px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: rgba(255,255,255,0.02);
  color: var(--color-text-secondary);
  font-size: 12px;
}
.dv-mode {
  margin-left: auto;
  color: var(--color-text-tertiary);
}
.dv-sync.ok {
  border-color: rgba(38,166,154,0.35);
  background: rgba(38,166,154,0.08);
  color: var(--color-positive);
}
.dv-sync.bad {
  border-color: rgba(239,83,80,0.35);
  background: rgba(239,83,80,0.08);
  color: var(--color-negative);
}
.dv-cards { display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; }
.dv-card {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
}
.dv-card-header {
  padding: 8px 12px;
  font-size: 13px;
  font-weight: 600;
  border-bottom: 1px solid var(--color-border);
}
.dv-card-body { padding: 10px 14px; display: flex; flex-direction: column; gap: 8px; }
.dv-stat { display: flex; justify-content: space-between; align-items: center; }
.dv-stat-label { font-size: 12px; color: var(--color-text-tertiary); }
.dv-stat-value { font-size: 15px; font-weight: 600; }
.dv-stat-value.large { font-size: 20px; }
.positive { color: var(--color-positive); }
.negative { color: var(--color-negative); }
.dv-section {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
}
</style>
