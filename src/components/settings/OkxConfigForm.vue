<template>
  <div class="okx-config">
    <OkxConfigHeader
      :current-mode-class="currentModeClass"
      :current-mode-label="currentModeLabel"
      :saving="saving"
      :testing="testing"
      @save="handleSave"
      @test="handleTest"
    />
    <div class="oc-body">
      <OkxModeSelector
        v-model="local.use_simulated"
        :demo-configured="local.demo.is_configured"
        :live-configured="local.live.is_configured"
      />
      <OkxProxySection
        v-model="local.proxy_url"
        :current-proxy-label="currentProxyLabel"
      />
      <OkxCredentialSection
        :active="local.use_simulated"
        :credentials="local.demo"
        mode="demo"
        title="模拟盘凭证"
        @update-field="(key, value) => updateCredentialField(local.demo, key, value)"
      />
      <OkxCredentialSection
        :active="!local.use_simulated"
        :credentials="local.live"
        mode="live"
        title="实盘凭证"
        @update-field="(key, value) => updateCredentialField(local.live, key, value)"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, watch } from 'vue'
import type { OkxConfig, OkxConfigSaveRequest } from '@/types/system'
import type { OkxCredentialKey, OkxCredentialState } from '@/utils/okxConfigForm'
import {
  applyOkxConfigToDraft,
  createOkxConfigDraft,
  okxConfigSavePayload,
  okxProxyLabel,
} from '@/utils/okxConfigForm'
import OkxConfigHeader from './okx-config/OkxConfigHeader.vue'
import OkxCredentialSection from './okx-config/OkxCredentialSection.vue'
import OkxModeSelector from './okx-config/OkxModeSelector.vue'
import OkxProxySection from './okx-config/OkxProxySection.vue'
import './okx-config/styles.css'

const props = defineProps<{
  config: OkxConfig | null
  saving?: boolean
  testing?: boolean
}>()

const emit = defineEmits<{
  save: [config: OkxConfigSaveRequest]
  test: [config: OkxConfigSaveRequest]
}>()

const local = reactive(createOkxConfigDraft())

watch(() => props.config, config => {
  applyOkxConfigToDraft(local, config)
}, { immediate: true })

const currentModeLabel = computed(() => local.use_simulated ? '当前：模拟盘' : '当前：实盘')
const currentModeClass = computed(() => local.use_simulated ? 'demo' : 'live')
const currentProxyLabel = computed(() => okxProxyLabel(local))

function handleSave() {
  emit('save', okxConfigSavePayload(local))
}

function handleTest() {
  emit('test', okxConfigSavePayload(local))
}

function updateCredentialField(
  credentials: OkxCredentialState,
  key: OkxCredentialKey,
  value: string,
) {
  credentials[key] = value
}
</script>
