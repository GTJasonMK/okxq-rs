<template>
  <section class="oc-section">
    <div class="oc-section-head">
      <h3>网络代理</h3>
      <span class="status-badge" :class="currentProxyLabel ? 'ok' : 'missing'">
        {{ currentProxyLabel ? '已启用' : '直连' }}
      </span>
    </div>
    <div class="oc-fields">
      <div class="oc-field">
        <div class="oc-field-head">
          <label for="okx-proxy-url">OKX 代理</label>
          <span v-if="currentProxyLabel" class="oc-mask">当前 {{ currentProxyLabel }}</span>
        </div>
        <input
          id="okx-proxy-url"
          v-model="proxyUrl"
          class="oc-input"
          autocomplete="off"
          spellcheck="false"
          placeholder="http://127.0.0.1:7897"
        />
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  currentProxyLabel: string
  modelValue: string
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

const proxyUrl = computed({
  get: () => props.modelValue,
  set: value => emit('update:modelValue', value),
})
</script>
