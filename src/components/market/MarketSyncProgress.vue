<template>
  <div v-if="progress.visible" class="vm-sync-progress">
    <div class="vm-sync-progress-head">
      <span>{{ progress.statusLabel }}</span>
      <strong>{{ progress.progress }}%</strong>
    </div>
    <div class="vm-sync-progress-track" :class="{ segmented: progress.segments.length > 1 }">
      <span
        v-if="progress.segments.length <= 1"
        class="vm-sync-progress-fill"
        :style="{ width: `${progress.progress}%` }"
      ></span>
      <template v-else>
        <span
          v-for="segment in progress.segments"
          :key="segment.key"
          class="vm-sync-progress-segment"
          :class="[segment.key, { active: segment.active }]"
          :style="{ flexGrow: segment.weight, '--segment-progress': `${segment.progress}%` }"
          :title="`${segment.label} ${segment.text}`"
        >
          <i></i>
        </span>
      </template>
    </div>
    <div v-if="progress.segments.length > 1" class="vm-sync-progress-stages">
      <span
        v-for="segment in progress.segments"
        :key="segment.key"
        :class="{ active: segment.active }"
      >
        {{ segment.label }} {{ segment.text }}
      </span>
    </div>
    <div class="vm-sync-progress-main">
      <span class="vm-sync-phase">{{ progress.phaseLabel }}</span>
      <strong>{{ progress.primaryText }}</strong>
    </div>
    <div class="vm-sync-progress-meta">
      <span>{{ progress.taskText }}</span>
      <span v-if="progress.secondaryText">{{ progress.secondaryText }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { RepairProgress } from '@/types/marketView'

defineProps<{
  progress: RepairProgress
}>()
</script>

<style scoped>
.vm-sync-progress {
  padding: 8px 10px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background: var(--color-bg-secondary);
}

.vm-sync-progress-head,
.vm-sync-progress-meta {
  display: flex;
  align-items: center;
  gap: 10px;
}

.vm-sync-progress-head {
  justify-content: space-between;
  color: var(--color-text-secondary);
  font-size: 12px;
  font-weight: 600;
}

.vm-sync-progress-head strong {
  color: var(--color-text-primary);
}

.vm-sync-progress-track {
  height: 6px;
  margin: 7px 0;
  overflow: hidden;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.08);
}

.vm-sync-progress-fill {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: var(--color-accent);
  transition: width 180ms ease;
}

.vm-sync-progress-track.segmented {
  display: flex;
  gap: 2px;
  background: transparent;
}

.vm-sync-progress-segment {
  position: relative;
  display: block;
  height: 100%;
  min-width: 18px;
  overflow: hidden;
  border-radius: inherit;
  background: rgba(255, 255, 255, 0.08);
}

.vm-sync-progress-segment i {
  display: block;
  width: var(--segment-progress);
  height: 100%;
  border-radius: inherit;
  background: var(--color-accent);
  transition: width 180ms ease;
}

.vm-sync-progress-segment.save i {
  background: #17a2b8;
}

.vm-sync-progress-segment.derive i {
  background: #22a06b;
}

.vm-sync-progress-segment.active {
  background: rgba(255, 255, 255, 0.13);
}

.vm-sync-progress-stages {
  display: flex;
  flex-wrap: wrap;
  gap: 6px 12px;
  margin: -1px 0 6px;
  color: var(--color-text-tertiary);
  font-size: 11px;
}

.vm-sync-progress-stages span.active {
  color: var(--color-text-secondary);
}

.vm-sync-progress-meta {
  flex-wrap: wrap;
  color: var(--color-text-tertiary);
  font-size: 12px;
}

.vm-sync-progress-main {
  display: flex;
  align-items: baseline;
  gap: 8px;
  margin: -1px 0 6px;
  color: var(--color-text-secondary);
  font-size: 12px;
}

.vm-sync-progress-main strong {
  color: var(--color-text-primary);
  font-size: 13px;
}

.vm-sync-phase {
  min-width: 52px;
  color: var(--color-text-tertiary);
}
</style>
