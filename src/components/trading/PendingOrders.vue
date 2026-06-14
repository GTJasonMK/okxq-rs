<template>
  <div class="pending-orders">
    <div class="po-header">
      <span class="po-title">挂单 ({{ orders.length }})</span>
      <span class="po-mode" :class="{ live: resolvedMode === 'live', locked: modeLocked }">
        {{ modeLabel }}
      </span>
    </div>
    <div v-if="error" class="po-error">{{ error }}</div>
    <div class="table-wrap">
      <table v-if="pendingOrders.length > 0">
        <thead>
          <tr>
            <th>品种</th>
            <th>方向</th>
            <th>类型</th>
            <th class="num">数量</th>
            <th class="num">价格</th>
            <th>状态</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="o in pendingOrders" :key="o.ord_id">
            <td class="symbol-cell">{{ o.inst_id }}</td>
            <td>
              <span class="side-badge" :class="o.side">{{ o.side === 'buy' ? '买' : '卖' }}</span>
            </td>
            <td class="type-text">{{ o.ord_type === 'limit' ? '限价' : '市价' }}</td>
            <td class="num">{{ formatNullableSize(o.sz) }}</td>
            <td class="num">{{ orderPriceText(o) }}</td>
            <td>
              <span class="state-badge" :class="o.state">{{ stateLabel(o.state) }}</span>
            </td>
            <td class="action-cell">
              <button
                class="cancel-btn"
                :title="modeLocked ? '查看模式与默认交易模式不一致，撤单已锁定' : '撤单'"
                @click="cancel(o)"
                :disabled="modeLocked || cancelling.has(o.ord_id)"
              >
                {{ cancelling.has(o.ord_id) ? '...' : '撤单' }}
              </button>
            </td>
          </tr>
        </tbody>
      </table>
      <div v-else class="empty-text">暂无挂单</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { Order } from '@/types'
import { usePendingOrders } from '@/composables/usePendingOrders'
import { formatPrice } from '@/utils/format'

const props = defineProps<{
  orders: Order[]
  mode?: string
  modeLocked?: boolean
}>()
const emit = defineEmits<{ cancelled: [] }>()
const {
  cancelling,
  error,
  pendingOrders,
  resolvedMode,
  modeLocked,
  modeLabel,
  stateLabel,
  cancel,
} = usePendingOrders(props, {
  onCancelled: () => emit('cancelled'),
})

function formatNullableSize(value: number | null): string {
  return Number.isFinite(value) ? String(value) : '--'
}

function orderPriceText(order: Order): string {
  if (order.ord_type === 'market') return '市价'
  return Number.isFinite(order.px) ? formatPrice(order.px) : '--'
}
</script>

<style scoped>
.pending-orders {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  min-height: 0;
}
.po-header {
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.po-title { font-size: 13px; font-weight: 600; }
.po-mode {
  padding: 2px 6px;
  border: 1px solid rgba(38,166,154,0.3);
  border-radius: 4px;
  background: rgba(38,166,154,0.08);
  color: var(--color-positive);
  font-size: 11px;
  white-space: nowrap;
}
.po-mode.live {
  border-color: rgba(239,83,80,0.32);
  background: rgba(239,83,80,0.08);
  color: var(--color-negative);
}
.po-mode.locked {
  border-color: rgba(255,193,7,0.32);
  background: rgba(255,193,7,0.08);
  color: #d6a93b;
}
.po-error {
  margin: 8px 10px 0;
  padding: 7px 8px;
  border: 1px solid rgba(239,83,80,0.35);
  border-radius: 4px;
  background: rgba(239,83,80,0.08);
  color: var(--color-negative);
  font-size: 12px;
  line-height: 1.4;
}
.table-wrap {
  overflow: auto;
  min-height: 0;
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
.symbol-cell { font-weight: 600; }
.side-badge {
  display: inline-block;
  padding: 1px 6px;
  border-radius: 3px;
  font-size: 11px;
  font-weight: 500;
}
.side-badge.buy { background: rgba(38,166,154,0.15); color: var(--color-positive); }
.side-badge.sell { background: rgba(239,83,80,0.15); color: var(--color-negative); }
.state-badge {
  font-size: 11px;
  color: var(--color-text-secondary);
}
.state-badge.live { color: var(--color-accent); }
.state-badge.partially_filled { color: #ff9800; }
.action-cell { text-align: right; }
.cancel-btn {
  padding: 2px 8px;
  border: 1px solid var(--color-border);
  border-radius: 3px;
  background: none;
  color: var(--color-text-secondary);
  font-size: 11px;
  cursor: pointer;
}
.cancel-btn:hover { color: var(--color-negative); border-color: var(--color-negative); }
.cancel-btn:disabled { opacity: 0.4; cursor: not-allowed; }
.empty-text {
  padding: 24px;
  text-align: center;
  color: var(--color-text-tertiary);
  font-size: 13px;
}
.type-text { color: var(--color-text-secondary); }
</style>
