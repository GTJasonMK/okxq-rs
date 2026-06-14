<template>
  <section v-if="active" class="dc-panel">
    <div class="dc-panel-head">
      <div>
        <h2>数据守护</h2>
        <p>守护器按关注规则扫描缺口，自动复用或创建后台同步任务。</p>
      </div>
      <div class="dc-panel-actions">
        <button class="dc-btn" type="button" :disabled="loading" @click="$emit('load-data')">
          {{ loading ? '刷新中' : '刷新状态' }}
        </button>
        <button class="dc-btn primary" type="button" :disabled="guardianRunning" @click="$emit('run-guardian')">
          {{ guardianRunning ? '请求中' : '立即扫描' }}
        </button>
      </div>
    </div>
    <div v-if="message || error" class="dc-feedback" :class="{ error: !!error }">
      {{ error || message }}
    </div>
    <div class="dc-kpi-grid">
      <div class="dc-kpi">
        <span>守护器</span>
        <strong>{{ status?.enabled ? '已启用' : '未启用' }}</strong>
      </div>
      <div class="dc-kpi">
        <span>扫描状态</span>
        <strong>{{ status?.active ? '扫描中' : '空闲' }}</strong>
      </div>
      <div class="dc-kpi">
        <span>已接管规则</span>
        <strong>{{ status?.watched_count ?? watchedSymbolsCount }}</strong>
      </div>
      <div class="dc-kpi">
        <span>队列</span>
        <strong>{{ status?.backfill_queue_size ?? activeJobsCount }}</strong>
      </div>
    </div>
    <div class="dc-detail-grid">
      <div class="dc-detail">
        <span>策略摘要</span>
        <strong>{{ status?.policy_summary || managedPlanLabels }}</strong>
      </div>
      <div class="dc-detail">
        <span>滚动周期</span>
        <strong>{{ formatList(status?.rolling_window_timeframes) }}</strong>
      </div>
      <div class="dc-detail">
        <span>全量周期</span>
        <strong>{{ formatList(status?.full_backfill_timeframes) }}</strong>
      </div>
      <div class="dc-detail">
        <span>当前目标</span>
        <strong>{{ currentTarget }}</strong>
      </div>
      <div class="dc-detail">
        <span>上次成功</span>
        <strong>{{ formatDateTimeValue(status?.last_successful_run_at) }}</strong>
      </div>
      <div class="dc-detail">
        <span>上次完成</span>
        <strong>{{ formatDateTimeValue(status?.last_run_finished_at) }}</strong>
      </div>
    </div>
    <section v-if="queuePreview.length" class="dc-queue">
      <h3>运行队列</h3>
      <div v-for="job in queuePreview" :key="job.task_id" class="dc-queue-row">
        <span>{{ job.inst_id }} · {{ job.inst_type }} · {{ job.timeframe }}</span>
        <strong>{{ formatJobStatus(job) }}</strong>
      </div>
    </section>
    <div v-if="errors.length" class="dc-error-list">
      <div v-for="item in errors" :key="item">{{ item }}</div>
    </div>
  </section>
</template>

<script setup lang="ts">
import type { SyncJob } from '@/types'
import type { GuardianStatus } from '@/types/dataCenter'
import {
  formatDateTimeValue,
  formatJobStatus,
  formatList,
} from '@/utils/dataCenter'

defineProps<{
  active: boolean
  status: GuardianStatus | null
  queuePreview: SyncJob[]
  errors: string[]
  currentTarget: string
  loading: boolean
  guardianRunning: boolean
  watchedSymbolsCount: number
  activeJobsCount: number
  managedPlanLabels: string
  message: string
  error: string
}>()

defineEmits<{
  'load-data': []
  'run-guardian': []
}>()
</script>
