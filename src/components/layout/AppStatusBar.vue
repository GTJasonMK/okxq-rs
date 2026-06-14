<template>
  <footer class="app-statusbar">
    <div class="status-group">
      <span class="status-item">OKXQ v0.1.0</span>
      <span class="status-item">{{ currentTime }}</span>
      <span class="status-item connection" :class="{ connected: systemStore.connected }">
        {{ systemStore.connected ? '本地 API 已连接' : '本地 API 未连接' }}
      </span>
      <span class="status-item mode" :class="{ live: systemStore.tradingMode === 'live' }">
        默认：{{ systemStore.tradingModeLabel }}
      </span>
    </div>
    <div class="status-group right">
      <span class="status-item path" :title="dataPath">
        数据：{{ dataPathLabel }}
      </span>
    </div>
  </footer>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useSystemStore } from '@/stores/systemStore'

const systemStore = useSystemStore()
const currentTime = ref(formatClock(new Date()))
let timer = 0

const dataPath = computed(() => readNestedString(systemStore.status, ['data', 'database_path'])
  || readNestedString(systemStore.status, ['paths', 'data_dir'])
  || '未加载')
const dataPathLabel = computed(() => compactPath(dataPath.value))

onMounted(() => {
  timer = window.setInterval(() => {
    currentTime.value = formatClock(new Date())
  }, 1000)
})

onUnmounted(() => {
  if (timer) window.clearInterval(timer)
})

function formatClock(value: Date) {
  return new Intl.DateTimeFormat('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  }).format(value)
}

function compactPath(value: string) {
  if (!value || value === '未加载') return value
  const normalized = value.replace(/\\/g, '/')
  const parts = normalized.split('/').filter(Boolean)
  if (parts.length <= 3) return value
  return `.../${parts.slice(-3).join('/')}`
}

function readNestedString(source: Record<string, unknown>, keys: string[]) {
  let current: unknown = source
  for (const key of keys) {
    if (!current || typeof current !== 'object' || Array.isArray(current)) return ''
    current = (current as Record<string, unknown>)[key]
  }
  return typeof current === 'string' ? current : ''
}
</script>

<style scoped>
.app-statusbar {
  height: 24px;
  min-height: 24px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 0 12px;
  border-top: 1px solid var(--color-border);
  background: var(--color-bg-secondary);
  font-size: 11px;
  color: var(--color-text-tertiary);
  overflow: hidden;
}
.status-group {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
}
.status-group.right {
  justify-content: flex-end;
  flex: 1;
}
.status-item {
  white-space: nowrap;
}
.connection {
  color: var(--color-negative);
}
.connection.connected,
.mode {
  color: var(--color-positive);
}
.mode.live {
  color: var(--color-negative);
}
.path {
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: min(42vw, 520px);
}
</style>
