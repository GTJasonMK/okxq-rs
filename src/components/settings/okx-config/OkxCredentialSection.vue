<template>
  <section class="oc-section">
    <div class="oc-section-head">
      <div class="oc-title-row">
        <h3>{{ title }}</h3>
        <span v-if="active" class="active-chip" :class="{ live: mode === 'live' }">当前启用</span>
      </div>
      <span class="status-badge" :class="credentials.is_configured ? 'ok' : 'missing'">
        {{ credentials.is_configured ? '已配置' : '未配置' }}
      </span>
    </div>

    <div class="oc-fields">
      <div v-for="field in fields" :key="field.key" class="oc-field">
        <div class="oc-field-head">
          <label :for="inputId(field.id)">{{ field.label }}</label>
          <span v-if="credentials.masked[field.key]" class="oc-mask">
            当前 {{ credentials.masked[field.key] }}
          </span>
        </div>
        <input
          :id="inputId(field.id)"
          class="oc-input"
          :value="credentials[field.key]"
          :type="field.secret ? 'password' : undefined"
          :autocomplete="field.secret ? 'new-password' : 'off'"
          spellcheck="false"
          :placeholder="placeholderForCredential(credentials, defaultPlaceholder(field.placeholder))"
          @input="onInput(field.key, $event)"
        />
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import type {
  OkxCredentialKey,
  OkxCredentialState,
} from '@/utils/okxConfigForm'
import { placeholderForCredential } from '@/utils/okxConfigForm'

type CredentialField = {
  id: string
  key: OkxCredentialKey
  label: string
  placeholder: string
  secret: boolean
}

const props = defineProps<{
  active: boolean
  credentials: OkxCredentialState
  mode: 'demo' | 'live'
  title: string
}>()

const emit = defineEmits<{
  updateField: [key: OkxCredentialKey, value: string]
}>()

const fields: CredentialField[] = [
  { id: 'api-key', key: 'api_key', label: 'API Key', placeholder: 'API Key', secret: false },
  { id: 'secret-key', key: 'secret_key', label: 'Secret Key', placeholder: 'Secret Key', secret: true },
  { id: 'passphrase', key: 'passphrase', label: 'Passphrase', placeholder: 'Passphrase', secret: true },
]

function inputId(id: string) {
  return `okx-${props.mode}-${id}`
}

function defaultPlaceholder(label: string) {
  return `OKX ${props.mode === 'demo' ? 'Demo' : 'Live'} ${label}`
}

function onInput(key: OkxCredentialKey, event: Event) {
  emit('updateField', key, (event.target as HTMLInputElement).value)
}
</script>
