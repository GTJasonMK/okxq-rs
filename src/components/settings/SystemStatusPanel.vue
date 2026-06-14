<template>
  <div class="sys-status">
    <div class="ss-header">
      <span class="ss-title">系统状态</span>
      <button class="refresh-btn" @click="$emit('refresh')">刷新</button>
    </div>
    <div class="ss-body">
      <div class="ss-row">
        <span class="ss-label">当前模式配置</span>
        <span class="ss-value">
          <span class="dot" :class="okxConfigured ? 'ok' : 'err'"></span>
          {{ okxConfigured ? '已配置' : '未配置' }}
        </span>
      </div>
      <div class="ss-row">
        <span class="ss-label">交易模式</span>
        <span class="ss-value">{{ tradingModeLabel }}</span>
      </div>
      <div class="ss-row">
        <span class="ss-label">模拟盘配置</span>
        <span class="ss-value">
          <span class="dot" :class="demoConfigured ? 'ok' : 'err'"></span>
          {{ demoConfigured ? '已配置' : '未配置' }}
        </span>
      </div>
      <div class="ss-row">
        <span class="ss-label">实盘配置</span>
        <span class="ss-value">
          <span class="dot" :class="liveConfigured ? 'ok' : 'err'"></span>
          {{ liveConfigured ? '已配置' : '未配置' }}
        </span>
      </div>
      <div class="ss-row" v-if="health">
        <span class="ss-label">健康检查</span>
        <span class="ss-value">{{ healthStatus }}</span>
      </div>
      <div class="ss-row">
        <span class="ss-label">运行时间</span>
        <span class="ss-value">{{ systemInfo.uptime || '--' }}</span>
      </div>
      <div class="ss-row">
        <span class="ss-label">受管币种</span>
        <span class="ss-value">{{ dataInfo.symbol_count ?? 0 }}</span>
      </div>
      <div class="ss-row">
        <span class="ss-label">受管市场</span>
        <span class="ss-value">{{ dataInfo.market_count ?? 0 }}</span>
      </div>
      <div class="ss-row">
        <span class="ss-label">受管K线</span>
        <span class="ss-value">{{ dataInfo.candle_count ?? 0 }}</span>
      </div>
      <div class="ss-row">
        <span class="ss-label">库内K线</span>
        <span class="ss-value">{{ dataInfo.db_candle_count ?? dataInfo.candle_count ?? 0 }}</span>
      </div>
      <div class="ss-row">
        <span class="ss-label">数据库</span>
        <span class="ss-value">{{ dataInfo.db_size || '--' }}</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{ status: Record<string, unknown> | null; health: unknown }>()
defineEmits<{ refresh: [] }>()

function recordAt(value: unknown, key: string): Record<string, unknown> {
  const item = value && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)[key]
    : null
  return item && typeof item === 'object' && !Array.isArray(item)
    ? item as Record<string, unknown>
    : {}
}

const okxInfo = computed(() => recordAt(props.status, 'okx'))
const systemInfo = computed(() => recordAt(props.status, 'system'))
const dataInfo = computed(() => recordAt(props.status, 'data'))
const okxConfigured = computed(() => okxInfo.value.api_configured === true)
const demoConfigured = computed(() => okxInfo.value.demo_configured === true)
const liveConfigured = computed(() => okxInfo.value.live_configured === true)
const tradingModeLabel = computed(() => {
  const mode = String(okxInfo.value.mode || '')
  if (mode === 'live') return '实盘模式'
  if (mode === 'simulated' || mode === 'demo') return '模拟模式'
  return '--'
})
const healthStatus = computed(() => {
  const value = props.health
  if (value && typeof value === 'object' && !Array.isArray(value)) {
    return String((value as Record<string, unknown>).status || 'ok')
  }
  return 'ok'
})
</script>

<style scoped>
.sys-status {
  background: var(--color-bg-secondary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
}
.ss-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border);
}
.ss-title { font-size: 13px; font-weight: 600; }
.refresh-btn {
  padding: 3px 10px;
  background: var(--color-bg-primary);
  border: 1px solid var(--color-border);
  border-radius: 3px;
  color: var(--color-text-secondary);
  font-size: 11px;
  cursor: pointer;
}
.ss-body { padding: 12px; }
.ss-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 6px 0;
  border-bottom: 1px solid var(--color-border);
}
.ss-row:last-child { border-bottom: none; }
.ss-label { font-size: 12px; color: var(--color-text-secondary); }
.ss-value { font-size: 12px; font-weight: 500; display: flex; align-items: center; gap: 6px; }
.dot { width: 6px; height: 6px; border-radius: 50%; display: inline-block; }
.dot.ok { background: var(--color-positive); }
.dot.err { background: var(--color-negative); }
</style>
