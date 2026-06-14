<template>
  <div v-if="show" class="rc-integrity" :class="integrityClass">
    <div class="integrity-head">
      <span class="integrity-title">结果检查</span>
      <span class="integrity-status">{{ integrityLabel }}</span>
    </div>
    <div v-if="integrityIssues.length > 0" class="integrity-issues">
      <span v-for="issue in integrityIssues" :key="issue" class="integrity-issue">{{ issue }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import {
  integrityIssueLabels,
  type RuntimeSummary,
} from '@/utils/backtestResultCard'

const props = defineProps<{
  integrity: RuntimeSummary | null
}>()

const integrityStatus = computed(() => {
  const status = props.integrity?.status
  return typeof status === 'string' ? status.trim() : ''
})

const show = computed(() =>
  Boolean(props.integrity && integrityStatus.value && integrityStatus.value !== 'ok')
)

const integrityLabel = computed(() => {
  const label = props.integrity?.label
  if (typeof label === 'string' && label.trim()) {
    return label.trim()
  }
  switch (integrityStatus.value) {
    case 'invalid':
      return '结果不可信'
    case 'warning':
      return '结果需要复核'
    case 'unverified':
      return '旧结果待复核'
    default:
      return '结果状态未知'
  }
})

const integrityClass = computed(() => {
  switch (integrityStatus.value) {
    case 'invalid':
      return 'integrity-invalid'
    case 'warning':
      return 'integrity-warning'
    case 'unverified':
      return 'integrity-unverified'
    default:
      return 'integrity-neutral'
  }
})

const integrityIssues = computed(() => {
  const issues = props.integrity?.issues
  if (!Array.isArray(issues)) return []
  return issues
    .filter((issue): issue is string => typeof issue === 'string' && issue.trim().length > 0)
    .map(issue => integrityIssueLabels[issue] ?? issue)
})
</script>

<style scoped>
.rc-integrity {
  padding: 8px 12px 9px;
  border-top: 1px solid var(--color-border);
}
.integrity-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  min-width: 0;
}
.integrity-title {
  color: var(--color-text-secondary);
  font-size: 12px;
  font-weight: 600;
}
.integrity-status {
  flex: 0 1 auto;
  padding: 2px 7px;
  overflow-wrap: anywhere;
  border: 1px solid var(--color-border);
  border-radius: 999px;
  font-size: 11px;
  line-height: 1.2;
}
.integrity-issues {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
  margin-top: 7px;
}
.integrity-issue {
  max-width: 100%;
  padding: 2px 6px;
  overflow-wrap: anywhere;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  color: var(--color-text-secondary);
  font-size: 11px;
  line-height: 1.25;
}
.rc-integrity.integrity-invalid .integrity-status,
.rc-integrity.integrity-invalid .integrity-issue {
  border-color: rgba(229, 62, 62, 0.28);
  color: var(--color-negative);
}
.rc-integrity.integrity-warning .integrity-status,
.rc-integrity.integrity-warning .integrity-issue,
.rc-integrity.integrity-unverified .integrity-status,
.rc-integrity.integrity-unverified .integrity-issue {
  border-color: rgba(183, 121, 31, 0.32);
  color: #b7791f;
}
</style>
