<template>
  <header class="dc-command-bar">
    <div class="dc-command-left">
      <h1 class="dc-title">数据中心</h1>
      <nav class="dc-tabs" role="tablist" aria-label="数据中心分页">
        <button
          v-for="tab in tabs"
          :key="tab.key"
          class="dc-tab"
          :class="{ active: activeTab === tab.key }"
          type="button"
          role="tab"
          :aria-selected="activeTab === tab.key"
          :title="tab.description"
          @click="$emit('set-tab', tab.key)"
        >
          {{ tab.label }}
        </button>
      </nav>
    </div>
    <p class="dc-hint">{{ activeTabHint }}</p>
  </header>
</template>

<script setup lang="ts">
import type { DataCenterTab, DataCenterTabItem } from '@/types/dataCenter'

defineProps<{
  tabs: DataCenterTabItem[]
  activeTab: DataCenterTab
  activeTabHint: string
}>()

defineEmits<{
  'set-tab': [tab: DataCenterTab]
}>()
</script>
