<template>
  <section class="vl-data-panel">
    <div class="vl-data-content" :class="`data-${activePanel}`">
      <LiveEquityHistory
        v-if="activePanel === 'equity-details'"
        :history="scopedEquityHistory"
        :mode="activeDataMode"
      />
      <LivePositionPanel
        v-else-if="activePanel === 'positions'"
        :history-orders="scopedOrders"
        :mode="activeDataMode"
        :positions="positions"
      />
	      <LiveExecutionPlanPanel
	        v-else-if="activePanel === 'planned-exits'"
	        :plans="executionPlans"
	      />
	      <div v-else class="vl-empty-panel">请选择要查看的数据页。</div>
	    </div>
	  </section>
</template>

<script setup lang="ts">
import LiveEquityHistory from '@/components/live/LiveEquityHistory.vue'
import LiveExecutionPlanPanel from '@/components/live/LiveExecutionPlanPanel.vue'
import LivePositionPanel from '@/components/live/LivePositionPanel.vue'
import type {
  LiveExecutionPlan,
  LiveOrder,
  LiveEquityHistory as LiveEquityHistoryData,
  Position,
  TradingMode,
} from '@/types'

type RuntimeDataPanel = 'equity-details' | 'positions' | 'planned-exits'

defineProps<{
  activePanel: RuntimeDataPanel
  activeDataMode: TradingMode
  executionPlans: LiveExecutionPlan[]
  positions: Position[]
  scopedEquityHistory: LiveEquityHistoryData | null
  scopedOrders: LiveOrder[]
}>()
</script>

<style scoped>
.vl-data-panel {
  position: relative;
  z-index: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}
.vl-data-content {
  flex: 1 1 auto;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}
.vl-data-content.data-equity-details,
.vl-data-content.data-planned-exits,
.vl-data-content.data-positions {
  overflow: auto;
  overscroll-behavior: contain;
}
.vl-data-content :deep(.le-panel),
.vl-data-content :deep(.lep-panel),
.vl-data-content :deep(.lp-panel) {
  min-height: 100%;
}
.vl-empty-panel {
  padding: 12px;
  color: var(--color-text-tertiary);
  font-size: 12px;
}

@media (max-width: 1100px) {
  .vl-data-panel {
    flex: 0 0 auto;
  }
}
</style>
