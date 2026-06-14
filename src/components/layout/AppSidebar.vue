<template>
  <nav class="app-sidebar">
    <div class="sidebar-brand">
      <span class="brand-text">OKXQ</span>
    </div>
    <ul class="sidebar-nav">
      <li v-for="group in sidebarNavGroups" :key="group.key" class="sidebar-section" :class="{ active: isGroupActive(group) }">
        <div class="section-title">
          <span>{{ group.label }}</span>
        </div>
        <ul class="section-items">
          <li v-for="item in group.items" :key="item.path">
            <router-link
              :to="item.path"
              class="nav-item"
              :class="{ active: isActive(item.path) }"
              :title="uiStore.sidebarCollapsed ? item.label : undefined"
              :aria-label="item.label"
            >
              <span class="nav-icon">{{ item.icon }}</span>
              <span class="nav-label">{{ item.label }}</span>
            </router-link>
          </li>
        </ul>
      </li>
    </ul>
    <div class="sidebar-footer">
      <button class="collapse-btn" :title="collapseLabel" :aria-label="collapseLabel" @click="uiStore.toggleSidebar">
        {{ uiStore.sidebarCollapsed ? '▶' : '◀' }}
      </button>
    </div>
  </nav>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import { useUiStore } from '@/stores/uiStore'
import { sidebarNavGroups, type SidebarNavGroup } from '@/config/navigation'

const route = useRoute()
const uiStore = useUiStore()
const collapseLabel = computed(() => uiStore.sidebarCollapsed ? '展开侧栏' : '收起侧栏')

function isActive(path: string): boolean {
  if (path === '/') return route.path === '/'
  return route.path.startsWith(path)
}

function isGroupActive(group: SidebarNavGroup): boolean {
  return group.items.some((item) => isActive(item.path))
}
</script>

<style scoped>
.app-sidebar {
  width: 220px;
  min-width: 220px;
  background: var(--color-bg-sidebar);
  border-right: 1px solid var(--color-border);
  display: flex;
  flex-direction: column;
  transition: width 0.2s, min-width 0.2s;
}
.sidebar-collapsed .app-sidebar {
  width: 56px;
  min-width: 56px;
}
.sidebar-brand {
  height: 48px;
  display: flex;
  align-items: center;
  padding: 0 16px;
  border-bottom: 1px solid var(--color-border);
}
.brand-text {
  font-size: 18px;
  font-weight: 700;
  color: var(--color-accent);
}
.sidebar-nav {
  flex: 1;
  list-style: none;
  margin: 0;
  padding: 8px 0;
  overflow-y: auto;
}
.sidebar-section {
  margin: 0;
  padding: 0;
}
.sidebar-section + .sidebar-section {
  margin-top: 8px;
  padding-top: 8px;
  border-top: 1px solid var(--color-border);
}
.section-title {
  padding: 0 16px 6px;
  font-size: 12px;
  font-weight: 600;
  color: var(--color-text-secondary);
}
.sidebar-section.active .section-title {
  color: var(--color-accent);
}
.section-items {
  list-style: none;
  margin: 0;
  padding: 0;
}
.nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 16px;
  color: var(--color-text-secondary);
  text-decoration: none;
  font-size: 14px;
  transition: background 0.15s, color 0.15s;
}
.nav-item:hover {
  background: var(--color-bg-hover);
  color: var(--color-text-primary);
}
.nav-item.active {
  background: var(--color-bg-active);
  color: var(--color-accent);
}
.nav-icon { font-size: 16px; width: 20px; text-align: center; }
.sidebar-collapsed .section-title { display: none; }
.sidebar-collapsed .nav-label { display: none; }
.sidebar-collapsed .sidebar-section + .sidebar-section {
  margin-top: 0;
  padding-top: 0;
  border-top: none;
}
.sidebar-footer {
  padding: 8px;
  border-top: 1px solid var(--color-border);
}
.collapse-btn {
  width: 100%;
  padding: 6px;
  background: none;
  border: none;
  color: var(--color-text-secondary);
  cursor: pointer;
  font-size: 12px;
}
.collapse-btn:hover { color: var(--color-text-primary); }
</style>
