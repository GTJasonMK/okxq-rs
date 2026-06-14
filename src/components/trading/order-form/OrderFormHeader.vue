<template>
  <div class="of-header">
    <span class="of-title">下单</span>
    <span class="of-mode" :class="{ live: resolvedMode === 'live', locked: modeLocked }">
      {{ modeLabel }}
    </span>
  </div>
</template>

<script setup lang="ts">
import type { TradingMode } from '@/types'

defineProps<{
  modeLabel: string
  modeLocked: boolean
  resolvedMode: TradingMode
}>()
</script>
