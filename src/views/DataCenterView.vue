<template>
  <div class="data-center-view">
    <DataCenterHeader
      :tabs="dataTabs"
      :active-tab="activeTab"
      :active-tab-hint="activeTabHint"
      @set-tab="setActiveTab"
    />

    <DataCenterWatchlistPanel
      :active="activeTab === 'watchlist'"
      :new-symbol="newSymbol"
      :adding="adding"
      :can-open-rule-dialog="canOpenRuleDialog"
      :loading="loading"
      :guardian-running="guardianRunning"
      :visible-symbols-count="watchedRowSources.length"
      :watched-symbols-count="watchedSymbols.length"
      :enabled-instrument-count="enabledInstrumentCount"
      :active-jobs-count="activeJobs.length"
      :managed-plan-labels="managedPlanLabels"
      :message="message"
      :error="error"
      :watched-row-sources="watchedRowSources"
      :sync-jobs="syncJobs"
      :enabled-plans="enabledPlans"
      :repairing-symbol="repairingSymbol"
      :gap-repairing-key="gapRepairingKey"
      :deleting-symbol="deletingSymbol"
      @update:new-symbol="newSymbol = $event"
      @open-rule-dialog="openRuleDialog"
      @load-page-data="loadPageData"
      @run-guardian="runGuardian"
      @open-market="openMarket"
      @edit-symbol="editSymbol"
      @repair-symbol="repairSymbol"
      @repair-gap="repairInventoryGap"
      @delete-symbol="deleteSymbol"
      @cancel-row-active-jobs="cancelRowActiveJobs"
    />

    <DataCenterCollectionPanel
      :active="activeTab === 'collection'"
      :status="tickCollectorStatus"
      :loading="collectionLoading"
      :mutating="collectionMutating"
      :message="message"
      :error="error"
      @load-status="loadCollectionStatus"
      @start="startCollection"
      @stop="stopCollection"
    />

    <DataCenterInventoryPanel
      :active="activeTab === 'inventory'"
      :rows="inventoryRows"
      :summary="inventorySummary"
      :table-totals="inventoryTableTotals"
      :loading="inventoryLoading"
      :rebuilding="inventoryRebuilding"
      :rebuild-progress="inventoryRebuildProgress"
      :active-jobs-count="activeJobs.length"
      :message="message"
      :error="error"
      :gap-repairing-key="gapRepairingKey"
      @load-inventory="loadInventory"
      @rebuild-cache="rebuildInventoryCache"
      @open-market="openMarket"
      @repair-gap="repairInventoryGap"
    />

    <DataCenterGuardianPanel
      :active="activeTab === 'guardian'"
      :status="guardianStatus"
      :queue-preview="guardianQueuePreview"
      :errors="guardianErrors"
      :current-target="guardianCurrentTarget"
      :loading="guardianStatusLoading"
      :guardian-running="guardianRunning"
      :watched-symbols-count="watchedSymbols.length"
      :active-jobs-count="activeJobs.length"
      :managed-plan-labels="managedPlanLabels"
      :message="message"
      :error="error"
      @load-data="loadGuardianData"
      @run-guardian="runGuardian"
    />

    <DataCenterRuleDialog
      :open="ruleDialogOpen"
      :pending-symbol="pendingRuleSymbol"
      :sync-spot="newSyncSpot"
      :sync-swap="newSyncSwap"
      :archive-all="newArchiveAll"
      :auto-sync="newAutoSync"
      :sync-plans="newSyncPlans"
      :sync-days="newSyncDays"
      :adding="adding"
      :can-submit="canSubmitRuleDialog"
      :add-button-label="addButtonLabel"
      :sync-runtime-config="syncRuntimeConfig"
      :saving-sync-runtime="savingSyncRuntime"
      :message="message"
      :error="error"
      @close="closeRuleDialog"
      @submit="submitRuleDialog"
      @save-sync-runtime-config="saveSyncRuntimeConfig"
      @update:sync-spot="newSyncSpot = $event"
      @update:sync-swap="newSyncSwap = $event"
      @update:archive-all="newArchiveAll = $event"
      @update:auto-sync="newAutoSync = $event"
      @update:sync-plans="newSyncPlans = $event"
      @update:sync-days="setNewSyncDays"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import DataCenterCollectionPanel from '@/components/data-center/DataCenterCollectionPanel.vue'
import DataCenterGuardianPanel from '@/components/data-center/DataCenterGuardianPanel.vue'
import DataCenterHeader from '@/components/data-center/DataCenterHeader.vue'
import DataCenterInventoryPanel from '@/components/data-center/DataCenterInventoryPanel.vue'
import DataCenterRuleDialog from '@/components/data-center/DataCenterRuleDialog.vue'
import DataCenterWatchlistPanel from '@/components/data-center/DataCenterWatchlistPanel.vue'
import { useDataCenterCollection } from '@/composables/useDataCenterCollection'
import { useDataCenterGuardian } from '@/composables/useDataCenterGuardian'
import { useDataCenterInventory } from '@/composables/useDataCenterInventory'
import { useDataCenterOperations } from '@/composables/useDataCenterOperations'
import { useDataCenterPageData } from '@/composables/useDataCenterPageData'
import { useDataCenterRuleDialog } from '@/composables/useDataCenterRuleDialog'
import { useDataCenterShell } from '@/composables/useDataCenterShell'
import { useDataCenterSyncJobs } from '@/composables/useDataCenterSyncJobs'
import { useDataCenterTabs } from '@/composables/useDataCenterTabs'
import { useDataCenterWatchlistActions } from '@/composables/useDataCenterWatchlistActions'
import type { SyncRuntimeConfig } from '@/types'
import {
  DATA_CENTER_TABS,
  countEnabledInstruments,
  createWatchedRowSourcesBuilder,
  enabledSyncPlansFromGuardian,
  managedPlanLabelText,
} from '@/utils/dataCenter'
import '@/styles/data-center.css'

defineOptions({ name: 'DataCenterView' })

const dataTabs = DATA_CENTER_TABS
const router = useRouter()
const route = useRoute()

const adding = ref(false)
const message = ref('')
const error = ref('')
const syncRuntimeConfig = ref<SyncRuntimeConfig | null>(null)

const {
  activeTab,
  activeTabHint,
  resolvePreferredTab,
  setActiveTab,
  syncRouteTab,
} = useDataCenterTabs({
  tabs: dataTabs,
  route,
  router,
  loadTabData,
})

const {
  guardianPlans,
  guardianStatus,
  guardianStatusLoading,
  guardianQueuePreview,
  guardianErrors,
  guardianCurrentTarget,
  loadGuardianData,
  refreshGuardianStatus,
  applyGuardianConfig,
} = useDataCenterGuardian({
  error,
  clearFeedback,
})

const {
  tickCollectorStatus,
  collectionLoading,
  collectionMutating,
  loadCollectionStatus,
  startCollection,
  stopCollection,
} = useDataCenterCollection({
  message,
  error,
  clearFeedback,
})

const {
  inventoryRows,
  inventorySummary,
  inventoryLoading,
  inventoryRebuilding,
  inventoryRebuildProgress,
  syncRecords,
  syncRecordsByScope,
  inventoryTableTotals,
  loadInventory,
  refreshInventoryData,
  rebuildInventoryCache,
  applyInventoryPayload,
  replaceSyncRecordScopes,
} = useDataCenterInventory({
  message,
  error,
  clearFeedback,
})

const {
  syncJobs,
  activeJobs,
  applyFetchedSyncJobs,
  trackSubmittedJobs,
  hasPendingSyncJobObserve,
  shouldRefreshSyncJobSource,
} = useDataCenterSyncJobs({
  syncRecords,
  refreshObservedSource: refreshObservedSyncJobSource,
})

const {
  watchedSymbols,
  loading,
  loadPageData,
  refreshSyncProgressData,
  refreshSyncJobProgressData,
} = useDataCenterPageData({
  syncRecords,
  syncRuntimeConfig,
  applyInventoryPayload,
  replaceSyncRecordScopes,
  applyFetchedSyncJobs,
  applyGuardianConfig,
  error,
})

const {
  guardianRunning,
  gapRepairingKey,
  repairInventoryGap,
  runGuardian,
} = useDataCenterOperations({
  activeTab,
  message,
  error,
  clearFeedback,
  refreshActiveGapRepairSource,
  loadPageData,
  loadGuardianData,
  trackSubmittedJobs,
})

const enabledInstrumentCount = computed(() => countEnabledInstruments(watchedSymbols.value))
const enabledPlans = computed(() => enabledSyncPlansFromGuardian(guardianPlans.value))
const managedPlanLabels = computed(() => managedPlanLabelText(enabledPlans.value))
const buildWatchedRowSources = createWatchedRowSourcesBuilder()
const watchedRowSources = computed(() => (
  buildWatchedRowSources(
    watchedSymbols.value,
    [],
    enabledPlans.value,
    inventoryRows.value,
    syncRecordsByScope.value,
  )
))
const watchedRuleRows = computed(() => (
  watchedRowSources.value.map(source => source.row)
))

const {
  newSymbol,
  ruleDialogOpen,
  pendingRuleSymbol,
  newSyncSpot,
  newSyncSwap,
  newArchiveAll,
  newAutoSync,
  newSyncDays,
  newSyncPlans,
  canOpenRuleDialog,
  canSubmitRuleDialog,
  addButtonLabel,
  openRuleDialog,
  closeRuleDialog,
  editSymbol,
  setNewSyncDays,
  resetAfterSaved,
} = useDataCenterRuleDialog({
  watchedSymbols,
  watchedRows: watchedRuleRows,
  guardianPlans,
  adding,
  clearFeedback,
})

const {
  savingSyncRuntime,
  repairingSymbol,
  deletingSymbol,
  saveSyncRuntimeConfig,
  submitRuleDialog,
  repairSymbol,
  cancelRowActiveJobs,
  deleteSymbol,
} = useDataCenterWatchlistActions({
  message,
  error,
  adding,
  syncRuntimeConfig,
  watchedRows: watchedRuleRows,
  rule: {
    newSymbol,
    pendingRuleSymbol,
    syncSpot: newSyncSpot,
    syncSwap: newSyncSwap,
    archiveAll: newArchiveAll,
    autoSync: newAutoSync,
    syncDays: newSyncDays,
    syncPlans: newSyncPlans,
    canSubmit: canSubmitRuleDialog,
    resetAfterSaved,
  },
  clearFeedback,
  loadPageData,
  trackSubmittedJobs,
})

const { openMarket } = useDataCenterShell({
  route,
  router,
  symbolInput: newSymbol,
  activeTab,
  activeJobs,
  guardianStatus,
  resolvePreferredTab,
  syncRouteTab,
  hasPendingSyncJobObserve,
  shouldRefreshSyncJobSource,
  refreshSyncProgressData,
  refreshSyncJobProgressData,
  refreshInventoryData,
  refreshGuardianStatus,
})

function clearFeedback() {
  message.value = ''
  error.value = ''
}

async function loadTabData(tab = activeTab.value) {
  if (tab === 'watchlist') {
    await loadPageData()
    return
  }
  if (tab === 'collection') {
    await loadCollectionStatus()
    return
  }
  if (tab === 'inventory') {
    await loadInventory()
    return
  }
  await loadGuardianData()
}

async function refreshActiveGapRepairSource() {
  if (activeTab.value === 'inventory') {
    await refreshInventoryData()
    return
  }
  if (activeTab.value === 'watchlist') {
    await refreshSyncProgressData()
    return
  }
  if (activeTab.value === 'guardian') {
    await refreshGuardianStatus()
  }
}

async function refreshObservedSyncJobSource() {
  if (activeTab.value === 'guardian') {
    await refreshGuardianStatus()
    return
  }
  if (activeTab.value === 'inventory') {
    await refreshInventoryData()
    return
  }
  await refreshSyncProgressData()
}
</script>
