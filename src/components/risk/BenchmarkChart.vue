<template>
  <div ref="containerRef" class="benchmark-chart"></div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useAreaSeriesChart } from '@/composables/useAreaSeriesChart'

const props = defineProps<{ data: Array<{ time: number; value: number }> }>()
const containerRef = ref<HTMLDivElement>()

useAreaSeriesChart({
  containerRef,
  height: 140,
  seriesOptions: {
    lineColor: '#2962ff',
    topColor: 'rgba(41,98,255,0.3)',
    bottomColor: 'rgba(41,98,255,0.02)',
    lineWidth: 1.5,
  },
  data: () => props.data,
  watchSource: () => props.data,
  fitContent: 'always',
})
</script>

<style scoped>
.benchmark-chart { width: 100%; height: 140px; }
</style>
