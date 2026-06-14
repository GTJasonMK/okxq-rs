<template>
  <div class="view-trading">
    <div class="vt-header">
      <h2 class="vt-title">交易中心</h2>
      <button class="refresh-btn" @click="refreshAll()" :disabled="loading">
        {{ loading ? '刷新中...' : '刷新数据' }}
      </button>
    </div>
    <div v-if="error" class="vt-error">{{ error }}</div>
    <div v-if="message" class="vt-message">{{ message }}</div>
    <div class="vt-sync" :class="{ ok: privateRealtimeConnected, bad: privateRealtimeError }">
      OKX 私有订单状态通道：
      <span v-if="privateRealtimeConnected">{{ privateRealtimeMode === 'live' ? '实盘' : '模拟盘' }} WebSocket 订阅已启用</span>
      <span v-else-if="privateRealtimeError">WebSocket 异常：{{ privateRealtimeError }}</span>
      <span v-else>连接中...</span>
    </div>
    <div v-if="viewModeLocked" class="vt-warning">
      当前查看的是{{ viewModeLabel }}数据，系统默认交易模式为{{ systemStore.tradingModeLabel }}。下单和撤单等写操作已锁定到默认模式。
    </div>
    <AccountSummary :account="store.account" :positions="store.positions" />
    <div class="vt-grid">
      <div class="vt-left">
        <div class="vt-action-tabs" role="tablist" aria-label="交易操作">
          <button
            v-for="tab in actionTabs"
            :key="tab.key"
            type="button"
            class="vt-action-tab"
            :class="{ active: activeActionTab === tab.key }"
            role="tab"
            :aria-selected="activeActionTab === tab.key"
            @click="activeActionTab = tab.key"
          >
            <span>{{ tab.label }}</span>
            <span v-if="tab.count !== null" class="vt-action-count">{{ tab.count }}</span>
          </button>
        </div>
        <OrderForm
          v-show="activeActionTab === 'order'"
          :mode="viewMode"
          :mode-locked="viewModeLocked"
          @submitted="handleOrderSubmitted"
        />
        <PendingOrders
          v-show="activeActionTab === 'orders'"
          :orders="store.orders"
          :mode="viewMode"
          :mode-locked="viewModeLocked"
          @cancelled="handleOrderCancelled"
        />
      </div>
      <div class="vt-right">
        <div class="vt-info-tabs" role="tablist" aria-label="交易信息">
          <button
            v-for="tab in infoTabs"
            :key="tab.key"
            type="button"
            class="vt-info-tab"
            :class="{ active: activeInfoTab === tab.key }"
            role="tab"
            :aria-selected="activeInfoTab === tab.key"
            @click="activeInfoTab = tab.key"
          >
            <span>{{ tab.label }}</span>
            <span class="vt-info-count">{{ tab.count }}</span>
          </button>
        </div>
        <PositionTable
          v-if="activeInfoTab === 'positions'"
          class="vt-positions-card"
          :positions="store.positions"
          :mode-locked="viewModeLocked"
          :closing-position-keys="closingPositionKeys"
          @close="handleClosePosition"
        />
        <AssetHoldingsTable
          v-else-if="activeInfoTab === 'assets'"
          class="vt-assets-card"
          :assets="store.account?.details ?? []"
        />
        <FillHistory
          v-else
          class="vt-fills-card"
          :fills="store.fills"
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useTradingView } from '@/composables/useTradingView'
import AccountSummary from '@/components/trading/AccountSummary.vue'
import AssetHoldingsTable from '@/components/trading/AssetHoldingsTable.vue'
import PositionTable from '@/components/trading/PositionTable.vue'
import OrderForm from '@/components/trading/OrderForm.vue'
import PendingOrders from '@/components/trading/PendingOrders.vue'
import FillHistory from '@/components/trading/FillHistory.vue'

defineOptions({ name: 'TradingView' })

const {
  store,
  systemStore,
  loading,
  error,
  message,
  privateRealtimeConnected,
  privateRealtimeError,
  privateRealtimeMode,
  closingPositionKeys,
  viewMode,
  viewModeLabel,
  viewModeLocked,
  refreshAll,
  handleOrderSubmitted,
  handleOrderCancelled,
  handleClosePosition,
} = useTradingView()

type InfoTabKey = 'positions' | 'assets' | 'fills'
type ActionTabKey = 'order' | 'orders'

const activeActionTab = ref<ActionTabKey>('order')
const activeInfoTab = ref<InfoTabKey>('positions')
const actionTabs = computed(() => [
  { key: 'order' as const, label: '下单', count: null },
  { key: 'orders' as const, label: '挂单', count: store.orders.length },
])
const infoTabs = computed(() => [
  { key: 'positions' as const, label: '持仓', count: store.positions.length },
  { key: 'assets' as const, label: '资产', count: store.account?.details?.length ?? 0 },
  { key: 'fills' as const, label: '成交', count: store.fills.length },
])
</script>

<style scoped>
.view-trading {
  display: flex;
  flex-direction: column;
  height: 100%;
}
.vt-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}
.vt-title { font-size: 16px; font-weight: 600; margin: 0; }
.refresh-btn {
  padding: 4px 12px;
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 4px;
  color: var(--color-text-secondary);
  font-size: 12px;
  cursor: pointer;
}
.refresh-btn:hover { color: var(--color-text-primary); border-color: var(--color-accent); }
.refresh-btn:disabled { opacity: 0.5; cursor: not-allowed; }
.vt-message,
.vt-error {
  padding: 8px 10px;
  border-radius: 6px;
  font-size: 12px;
  margin-bottom: 8px;
}
.vt-message {
  border: 1px solid rgba(38,166,154,0.35);
  background: rgba(38,166,154,0.08);
  color: var(--color-positive);
}
.vt-error {
  border: 1px solid rgba(239,83,80,0.35);
  background: rgba(239,83,80,0.08);
  color: var(--color-negative);
}
.vt-sync {
  padding: 7px 10px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: rgba(255,255,255,0.02);
  color: var(--color-text-secondary);
  font-size: 12px;
  margin-bottom: 8px;
}
.vt-sync.ok {
  border-color: rgba(38,166,154,0.35);
  background: rgba(38,166,154,0.08);
  color: var(--color-positive);
}
.vt-sync.bad {
  border-color: rgba(239,83,80,0.35);
  background: rgba(239,83,80,0.08);
  color: var(--color-negative);
}
.vt-warning {
  padding: 8px 10px;
  border: 1px solid rgba(255, 193, 7, 0.35);
  border-radius: 6px;
  background: rgba(255, 193, 7, 0.08);
  color: #d6a93b;
  font-size: 12px;
  margin-bottom: 8px;
}
.vt-grid {
  flex: 1;
  display: grid;
  grid-template-columns: minmax(360px, 400px) minmax(0, 1fr);
  gap: 10px;
  min-height: 0;
}
.vt-left {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-height: 0;
  overflow: hidden;
}
.vt-right {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-height: 0;
  overflow: hidden;
}
.vt-right > * {
  min-width: 0;
}
.vt-action-tabs,
.vt-info-tabs {
  display: flex;
  gap: 6px;
  padding: 4px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: rgba(255,255,255,0.025);
  flex: 0 0 auto;
}
.vt-action-tab,
.vt-info-tab {
  flex: 1;
  min-width: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 7px 10px;
  border: 1px solid transparent;
  border-radius: 5px;
  background: transparent;
  color: var(--color-text-secondary);
  font-size: 12px;
  cursor: pointer;
}
.vt-action-tab:hover,
.vt-info-tab:hover {
  color: var(--color-text-primary);
  border-color: rgba(255,255,255,0.12);
}
.vt-action-tab.active,
.vt-info-tab.active {
  border-color: rgba(41,98,255,0.45);
  background: rgba(41,98,255,0.12);
  color: var(--color-accent);
}
.vt-action-count,
.vt-info-count {
  min-width: 18px;
  padding: 1px 5px;
  border-radius: 999px;
  background: rgba(255,255,255,0.08);
  color: var(--color-text-tertiary);
  font-size: 11px;
  line-height: 1.3;
  font-variant-numeric: tabular-nums;
}
.vt-action-tab.active .vt-action-count,
.vt-info-tab.active .vt-info-count {
  background: rgba(41,98,255,0.18);
  color: var(--color-accent);
}
.vt-left > .order-form,
.vt-left > .pending-orders {
  flex: 1 1 auto;
  min-height: 0;
}
.vt-assets-card,
.vt-positions-card,
.vt-fills-card {
  flex: 1 1 auto;
  min-height: 0;
}
@media (max-width: 1480px) {
  .vt-right {
    overflow: visible;
  }
}
@media (max-width: 1180px) {
  .vt-grid {
    grid-template-columns: 1fr;
  }
}
</style>
