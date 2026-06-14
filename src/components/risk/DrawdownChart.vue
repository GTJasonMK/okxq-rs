<template>
  <div ref="containerRef" class="dd-chart"></div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useAreaSeriesChart } from '@/composables/useAreaSeriesChart'

const props = defineProps<{ drawdown: unknown }>()
const containerRef = ref<HTMLDivElement>()

useAreaSeriesChart({
  containerRef,
  height: 200,
  seriesOptions: {
    lineColor: '#ef5350',
    topColor: 'rgba(239,83,80,0.3)',
    bottomColor: 'rgba(239,83,80,0.02)',
    lineWidth: 1.5,
  },
  data: drawdownSeries,
  watchSource: () => props.drawdown,
})

function drawdownSeries() {
  const drawdown = props.drawdown as { series?: Array<{ time: number; value: number }> } | null
  return drawdown?.series ?? []
}
</script>

<style scoped>
.dd-chart { width: 100%; height: 200px; }
</style>
