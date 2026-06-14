<template>
  <header class="app-topbar">
    <h2 class="page-title">{{ pageTitle }}</h2>
    <div class="topbar-actions">
      <span class="connection-status" :class="{ connected: systemStore.connected }">
        {{ systemStore.connected ? '已连接' : '未连接' }}
      </span>
    </div>
  </header>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import { useSystemStore } from '@/stores/systemStore'

const route = useRoute()
const systemStore = useSystemStore()

const pageTitle = computed(() => (route.meta.title as string) || 'OKXQ')
</script>

<style scoped>
.app-topbar {
  height: 48px;
  min-height: 48px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 24px;
  border-bottom: 1px solid var(--color-border);
  background: var(--color-bg-primary);
}
.page-title {
  font-size: 16px;
  font-weight: 600;
  margin: 0;
}
.topbar-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}
.connection-status {
  font-size: 12px;
  padding: 2px 8px;
  border-radius: 4px;
  background: var(--color-negative);
  color: #fff;
}
.connection-status.connected {
  background: var(--color-positive);
}
</style>
