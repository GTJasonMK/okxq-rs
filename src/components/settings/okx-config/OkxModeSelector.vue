<template>
  <section class="oc-section">
    <div class="oc-section-head">
      <h3>当前启用模式</h3>
    </div>
    <div class="mode-options" role="radiogroup" aria-label="当前启用模式">
      <label class="mode-option" :class="{ active: modelValue }">
        <input
          type="radio"
          :checked="modelValue"
          :value="true"
          @change="emit('update:modelValue', true)"
        />
        <span class="mode-label">模拟盘</span>
        <span class="mode-meta" :class="demoConfigured ? 'ok' : 'missing'">
          {{ demoConfigured ? '已配置' : '未配置' }}
        </span>
      </label>
      <label class="mode-option live" :class="{ active: !modelValue }">
        <input
          type="radio"
          :checked="!modelValue"
          :value="false"
          @change="emit('update:modelValue', false)"
        />
        <span class="mode-label">实盘</span>
        <span class="mode-meta" :class="liveConfigured ? 'ok' : 'missing'">
          {{ liveConfigured ? '已配置' : '未配置' }}
        </span>
      </label>
    </div>
  </section>
</template>

<script setup lang="ts">
defineProps<{
  demoConfigured: boolean
  liveConfigured: boolean
  modelValue: boolean
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
}>()
</script>
