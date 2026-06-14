<template>
  <div class="oc-header">
    <div class="oc-heading">
      <span class="oc-title">OKX API 配置</span>
      <span class="oc-current" :class="currentModeClass">{{ currentModeLabel }}</span>
    </div>
    <div class="oc-actions">
      <button class="test-btn" type="button" :disabled="testing || saving" @click="emit('test')">
        {{ testing ? '测试中...' : '测试配置' }}
      </button>
      <button class="save-btn" type="button" :disabled="saving || testing" @click="emit('save')">
        {{ saving ? '保存中...' : '保存配置' }}
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  currentModeClass: string
  currentModeLabel: string
  saving?: boolean
  testing?: boolean
}>()

const emit = defineEmits<{
  save: []
  test: []
}>()
</script>
