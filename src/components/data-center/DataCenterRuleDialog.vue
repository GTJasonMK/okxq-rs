<template>
  <div v-if="open" class="dc-modal-backdrop" @click.self="$emit('close')">
    <section class="dc-modal" role="dialog" aria-modal="true" aria-labelledby="rule-dialog-title">
      <header class="dc-modal-head">
        <div>
          <h2 id="rule-dialog-title">采集参数</h2>
          <p>{{ pendingSymbol }} · 保存前确认数据目标和 K 线生成规则</p>
        </div>
        <button class="dc-icon-btn" type="button" aria-label="关闭" @click="$emit('close')">x</button>
      </header>

      <div class="dc-modal-body">
        <div class="dc-rule-options">
          <label class="dc-check">
            <input :checked="syncSpot" type="checkbox" @change="$emit('update:sync-spot', ($event.target as HTMLInputElement).checked)" />
            <span>现货</span>
          </label>
          <label class="dc-check">
            <input :checked="syncSwap" type="checkbox" @change="$emit('update:sync-swap', ($event.target as HTMLInputElement).checked)" />
            <span>永续</span>
          </label>
          <label class="dc-check wide">
            <input :checked="archiveAll" type="checkbox" @change="$emit('update:archive-all', ($event.target as HTMLInputElement).checked)" />
            <span>全部周期全量</span>
          </label>
          <label class="dc-check wide">
            <input :checked="autoSync" type="checkbox" @change="$emit('update:auto-sync', ($event.target as HTMLInputElement).checked)" />
            <span>立即同步</span>
          </label>
        </div>
        <WatchSyncPlanEditor
          :model-value="syncPlans"
          :sync-days="syncDays"
          @update:model-value="$emit('update:sync-plans', $event)"
          @update:sync-days="$emit('update:sync-days', $event)"
        />
        <DataSyncSettingsPanel
          ref="settingsPanel"
          class="dc-modal-sync-settings"
          :config="syncRuntimeConfig"
          :saving="savingSyncRuntime"
          @save="$emit('save-sync-runtime-config', $event)"
        />
        <div v-if="message || error" class="dc-modal-feedback" :class="{ error: !!error }">
          {{ error || message }}
        </div>
      </div>

      <footer class="dc-modal-foot">
        <button class="dc-btn" type="button" :disabled="adding" @click="$emit('close')">取消</button>
        <button class="dc-btn primary" type="button" :disabled="adding || savingSyncRuntime || !canSubmit" @click="submit">
          {{ addButtonLabel }}
        </button>
      </footer>
    </section>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import WatchSyncPlanEditor from '@/components/data/WatchSyncPlanEditor.vue'
import DataSyncSettingsPanel from '@/components/settings/DataSyncSettingsPanel.vue'
import type { SyncRuntimeConfig, SyncRuntimeSettings, WatchedSymbolSyncPlan } from '@/types'

defineProps<{
  open: boolean
  pendingSymbol: string
  syncSpot: boolean
  syncSwap: boolean
  archiveAll: boolean
  autoSync: boolean
  syncPlans: WatchedSymbolSyncPlan[]
  syncDays: number
  adding: boolean
  canSubmit: boolean
  addButtonLabel: string
  syncRuntimeConfig: SyncRuntimeConfig | null
  savingSyncRuntime: boolean
  message: string
  error: string
}>()

const emit = defineEmits<{
  close: []
  submit: [settings?: SyncRuntimeSettings]
  'save-sync-runtime-config': [settings: SyncRuntimeSettings]
  'update:sync-spot': [value: boolean]
  'update:sync-swap': [value: boolean]
  'update:archive-all': [value: boolean]
  'update:auto-sync': [value: boolean]
  'update:sync-plans': [value: WatchedSymbolSyncPlan[]]
  'update:sync-days': [value: number]
}>()

const settingsPanel = ref<{ currentSettings: () => SyncRuntimeSettings } | null>(null)

function submit() {
  emit('submit', settingsPanel.value?.currentSettings())
}
</script>
