<template>
  <section v-if="active" class="dc-panel">
    <div class="dc-panel-head">
      <div>
        <h2>秒级采集</h2>
        <p>采集器接收实时成交与盘口中价，写入逐笔成交和 1 秒特征柱。</p>
      </div>
      <div class="dc-panel-actions">
        <button class="dc-btn" type="button" :disabled="loading" @click="$emit('load-status')">
          {{ loading ? '刷新中' : '刷新状态' }}
        </button>
        <button
          v-if="!status?.running"
          class="dc-btn primary"
          type="button"
          :disabled="mutating"
          @click="$emit('start')"
        >
          {{ mutating ? '处理中' : '启动采集' }}
        </button>
        <button
          v-else
          class="dc-btn danger"
          type="button"
          :disabled="mutating"
          @click="$emit('stop')"
        >
          {{ mutating ? '处理中' : '停止采集' }}
        </button>
      </div>
    </div>
    <div v-if="message || error" class="dc-feedback" :class="{ error: !!error }">
      {{ error || message }}
    </div>
    <div class="dc-kpi-grid">
      <div class="dc-kpi">
        <span>状态</span>
        <strong>{{ status?.running ? '运行中' : '未运行' }}</strong>
      </div>
      <div class="dc-kpi">
        <span>白名单</span>
        <strong>{{ status?.active_symbols?.length ?? 0 }}</strong>
      </div>
      <div class="dc-kpi">
        <span>成交</span>
        <strong>{{ formatCount(status?.total_trades_received ?? 0) }}</strong>
      </div>
      <div class="dc-kpi">
        <span>1 秒柱</span>
        <strong>{{ formatCount(status?.total_bars_written ?? 0) }}</strong>
      </div>
    </div>
    <div class="dc-detail-grid">
      <div class="dc-detail">
        <span>采集标的</span>
        <strong>{{ formatList(status?.active_symbols) }}</strong>
      </div>
      <div class="dc-detail">
        <span>最近成交</span>
        <strong>{{ formatDateTimeValue(status?.last_trade_ts) }}</strong>
      </div>
    </div>
    <div v-if="status?.errors?.length" class="dc-error-list">
      <div v-for="item in status.errors" :key="item">{{ item }}</div>
    </div>
  </section>
</template>

<script setup lang="ts">
import type { TickCollectorStatus } from '@/types/dataCenter'
import { formatCount, formatDateTimeValue, formatList } from '@/utils/dataCenter'

defineProps<{
  active: boolean
  status: TickCollectorStatus | null
  loading: boolean
  mutating: boolean
  message: string
  error: string
}>()

defineEmits<{
  'load-status': []
  start: []
  stop: []
}>()
</script>
