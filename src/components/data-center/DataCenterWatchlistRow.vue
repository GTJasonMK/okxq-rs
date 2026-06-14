<template>
  <article class="dc-row">
    <div class="dc-row-head">
      <div>
        <div class="dc-symbol">{{ row.symbol }}</div>
        <div class="dc-meta">
          <span>{{ row.base_ccy || row.symbol.split('-')[0] }}</span>
          <span>{{ ruleModeLabel(row) }}</span>
          <span>{{ rowPlanSummary(row, enabledPlans) }}</span>
          <span v-if="!row.inventory_only">添加 {{ formatTime(row.created_at) }}</span>
          <span v-else>来源 数据库库存</span>
        </div>
      </div>
      <div class="dc-row-actions">
        <button class="dc-btn small" @click="emit('open-market', row.symbol)">行情</button>
        <button class="dc-btn small" @click="emit('edit-symbol', row)">
          {{ row.inventory_only ? '接管规则' : '编辑规则' }}
        </button>
        <button
          v-if="!row.inventory_only"
          class="dc-btn small"
          :disabled="repairingSymbol === row.symbol"
          @click="emit('repair-symbol', row)"
        >
          {{ repairingSymbol === row.symbol ? '补齐中' : '补齐' }}
        </button>
        <button
          v-if="!row.inventory_only"
          class="dc-btn small danger"
          :disabled="deletingSymbol === row.symbol"
          @click="emit('delete-symbol', row.symbol)"
        >
          {{ deletingSymbol === row.symbol ? '删除中' : '删除' }}
        </button>
      </div>
    </div>

    <div class="dc-markets">
      <div class="dc-market" :class="{ disabled: !row.sync_spot }">
        <div class="dc-market-head">
          <span>SPOT</span>
          <strong>{{ row.spot_inst_id }}</strong>
        </div>
        <div v-if="row.sync_spot" class="dc-coverage">
          <div v-for="plan in planRows('SPOT')" :key="plan.timeframe" class="dc-plan-item">
            <span class="dc-chip" :class="plan.status">
              {{ plan.timeframe }} · {{ plan.policyLabel }} · {{ plan.label }}
            </span>
            <button
              v-if="canRepairPlan(plan)"
              class="dc-btn small"
              type="button"
              :disabled="gapRepairingKey === planRepairKey(plan)"
              :title="`精确补齐 ${row.spot_inst_id} ${plan.timeframe} 当前范围内缺失 K 线`"
              @click="emitRepairGap(plan)"
            >
              {{ gapRepairingKey === planRepairKey(plan) ? '提交中' : '精确补齐' }}
            </button>
          </div>
        </div>
        <div v-else class="dc-muted">未启用现货同步</div>
      </div>

      <div class="dc-market" :class="{ disabled: !row.sync_swap }">
        <div class="dc-market-head">
          <span>SWAP</span>
          <strong>{{ row.swap_inst_id }}</strong>
        </div>
        <div v-if="row.sync_swap" class="dc-coverage">
          <div v-for="plan in planRows('SWAP')" :key="plan.timeframe" class="dc-plan-item">
            <span class="dc-chip" :class="plan.status">
              {{ plan.timeframe }} · {{ plan.policyLabel }} · {{ plan.label }}
            </span>
            <button
              v-if="canRepairPlan(plan)"
              class="dc-btn small"
              type="button"
              :disabled="gapRepairingKey === planRepairKey(plan)"
              :title="`精确补齐 ${row.swap_inst_id} ${plan.timeframe} 当前范围内缺失 K 线`"
              @click="emitRepairGap(plan)"
            >
              {{ gapRepairingKey === planRepairKey(plan) ? '提交中' : '精确补齐' }}
            </button>
          </div>
        </div>
        <div v-else class="dc-muted">未启用永续同步</div>
      </div>
    </div>

    <div class="dc-jobs">
      <span v-if="row.jobSummary.total === 0" class="dc-muted">当前没有同步任务</span>
      <div v-else class="dc-job-progress" :class="{ failed: row.jobSummary.failed > 0 && row.jobSummary.active === 0 }">
        <div class="dc-job-progress-head">
          <span>{{ row.jobSummary.statusLabel }}</span>
          <strong>{{ row.jobSummary.progress }}%</strong>
        </div>
        <div class="dc-job-progress-track" :class="{ segmented: row.jobSummary.segments.length > 1 }">
          <span
            v-if="row.jobSummary.segments.length <= 1"
            class="dc-job-progress-fill"
            :style="{ width: `${row.jobSummary.progress}%` }"
          ></span>
          <template v-else>
            <span
              v-for="segment in row.jobSummary.segments"
              :key="segment.key"
              class="dc-job-progress-segment"
              :class="[segment.key, { active: segment.active }]"
              :style="{ flexGrow: segment.weight, '--segment-progress': `${segment.progress}%` }"
              :title="`${segment.label} ${segment.text}`"
            >
              <i></i>
            </span>
          </template>
        </div>
        <div v-if="row.jobSummary.segments.length > 1" class="dc-job-progress-stages">
          <span v-for="segment in row.jobSummary.segments" :key="segment.key" :class="{ active: segment.active }">
            {{ segment.label }} {{ segment.text }}
          </span>
        </div>
        <div class="dc-job-progress-main">
          <span class="dc-job-phase">{{ row.jobSummary.phaseLabel }}</span>
          <strong>{{ row.jobSummary.primaryText }}</strong>
        </div>
        <div class="dc-job-progress-meta">
          <span>{{ row.jobSummary.taskText }}</span>
          <span v-if="row.jobSummary.secondaryText">{{ row.jobSummary.secondaryText }}</span>
          <button v-if="row.jobSummary.active" class="dc-job-cancel" @click="emit('cancel-row-active-jobs', row)">
            取消运行任务
          </button>
        </div>
      </div>
    </div>
  </article>
</template>

<script setup lang="ts">
import type { InstType, WatchedSymbol, WatchedSymbolSyncPlan } from '@/types'
import type { ExactGapRepairPayload, PlanRow, WatchedRow } from '@/types/dataCenter'
import {
  formatTime,
  rowPlanSummary,
  ruleModeLabel,
} from '@/utils/dataCenter'

const props = defineProps<{
  row: WatchedRow
  enabledPlans: WatchedSymbolSyncPlan[]
  repairingSymbol: string
  gapRepairingKey: string
  deletingSymbol: string
}>()

const emit = defineEmits<{
  'open-market': [symbol: string]
  'edit-symbol': [row: WatchedSymbol]
  'repair-symbol': [row: WatchedSymbol]
  'repair-gap': [payload: ExactGapRepairPayload]
  'delete-symbol': [symbol: string]
  'cancel-row-active-jobs': [row: WatchedRow]
}>()

function planRows(instType: InstType) {
  return props.row.planRowsByInstType?.[instType] ?? []
}

function canRepairPlan(plan: PlanRow) {
  return (
    plan.gap_count > 0 &&
    !['queued', 'running'].includes(plan.status) &&
    isValidTimestamp(plan.start_ts) &&
    isValidTimestamp(plan.end_ts) &&
    Number(plan.end_ts) >= Number(plan.start_ts)
  )
}

function planRepairKey(plan: PlanRow) {
  return `${plan.inst_id}:${plan.inst_type}:${plan.timeframe}`
}

function emitRepairGap(plan: PlanRow) {
  if (!canRepairPlan(plan)) return
  emit('repair-gap', {
    inst_id: plan.inst_id,
    inst_type: plan.inst_type,
    timeframe: plan.timeframe,
    start_ts: Number(plan.start_ts),
    end_ts: Number(plan.end_ts),
  })
}

function isValidTimestamp(value: unknown) {
  return typeof value === 'number' && Number.isFinite(value) && value > 0
}
</script>
