<template>
  <div class="sp-list">
    <div v-for="profile in profiles" :key="profile.id" class="sp-card">
      <div class="sp-card-header">
        <span class="sp-card-name">{{ profile.name }}</span>
        <div class="sp-card-actions">
          <button class="icon-btn run" @click="$emit('run', profile)">▶ 扫描</button>
          <button class="icon-btn danger" @click="$emit('delete', profile)">✕</button>
        </div>
      </div>
      <div class="sp-card-conds">
        <span v-for="conditionId in profile.conditions" :key="conditionId" class="badge">{{ conditionId }}</span>
      </div>
      <div class="sp-card-meta">
        {{ profile.inst_type }} · {{ profile.timeframe }}
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { ScannerProfile } from '@/types'

defineProps<{ profiles: ScannerProfile[] }>()
defineEmits<{
  run: [profile: ScannerProfile]
  delete: [profile: ScannerProfile]
}>()
</script>

<style scoped>
.sp-list { max-height: 300px; overflow-y: auto; }
.sp-card {
  padding: 10px 12px;
  border-bottom: 1px solid var(--color-border);
}
.sp-card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 4px;
}
.sp-card-name { font-size: 13px; font-weight: 600; }
.sp-card-actions { display: flex; gap: 4px; }
.sp-card-conds { display: flex; gap: 4px; margin-bottom: 2px; }
.badge {
  padding: 0 5px;
  background: rgba(41,98,255,0.12);
  color: var(--color-accent);
  border-radius: 3px;
  font-size: 10px;
}
.sp-card-meta { font-size: 11px; color: var(--color-text-tertiary); }
.icon-btn {
  background: none;
  border: 1px solid var(--color-border);
  padding: 3px 8px;
  border-radius: 3px;
  color: var(--color-text-secondary);
  font-size: 11px;
  cursor: pointer;
}
.icon-btn:hover { background: var(--color-bg-hover); }
.icon-btn.run { color: var(--color-positive); border-color: var(--color-positive); }
.icon-btn.danger { color: var(--color-negative); border-color: var(--color-negative); }
</style>
