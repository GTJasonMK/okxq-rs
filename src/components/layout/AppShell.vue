<template>
  <div class="app-shell" :class="{ 'sidebar-collapsed': uiStore.sidebarCollapsed }">
    <AppSidebar />
    <div class="app-main">
      <AppTopBar />
      <main class="app-content">
        <router-view v-slot="{ Component }">
          <keep-alive :include="cachedViews" :max="8">
            <component :is="Component" />
          </keep-alive>
        </router-view>
      </main>
      <AppStatusBar />
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted } from 'vue'
import { useUiStore } from '@/stores/uiStore'
import { useSystemStore } from '@/stores/systemStore'
import { useGlobalNotifications } from '@/composables/useGlobalNotifications'
import AppSidebar from './AppSidebar.vue'
import AppTopBar from './AppTopBar.vue'
import AppStatusBar from './AppStatusBar.vue'

const uiStore = useUiStore()
const systemStore = useSystemStore()
const cachedViews = [
  'DashboardView',
  'MarketView',
  'DataCenterView',
  'TradingView',
  'BacktestView',
  'ResearchView',
  'TrendResearchView',
]

useGlobalNotifications()

onMounted(() => {
  void systemStore.checkConnection(1)
  void systemStore.loadConfig()
})
</script>

<style scoped>
.app-shell {
  display: flex;
  height: 100vh;
  background: var(--color-bg-primary);
  color: var(--color-text-primary);
}
.app-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
}
.app-content {
  flex: 1;
  overflow-y: auto;
  padding: 16px 24px;
}
</style>
