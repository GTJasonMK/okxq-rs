import { computed, ref, type ComputedRef, type Ref } from 'vue'
import type { WatchedSymbol, WatchedSymbolSyncPlan } from '@/types'
import type { GuardianPlan, WatchedRow } from '@/types/dataCenter'
import {
  DEFAULT_UNIFIED_SYNC_DAYS,
  applyUnifiedSyncDays,
  normalizeFullSyncPlans,
  normalizeSyncDays,
} from '@/utils/syncPlans'
import {
  canOpenWatchRuleDialog,
  canSubmitWatchRuleDialog,
  defaultWatchRuleForm,
  normalizeInputSymbol,
  watchRuleFormFromRow,
  watchRuleSubmitButtonLabel,
  type WatchRuleFormState,
} from '@/utils/dataCenter'

type WatchRuleRow = Omit<WatchedRow, 'jobs' | 'jobSummary'>

type DataCenterRuleDialogSources = {
  watchedSymbols: Ref<WatchedSymbol[]>
  watchedRows: ComputedRef<WatchRuleRow[]>
  guardianPlans: Ref<GuardianPlan[]>
  adding: Ref<boolean>
  clearFeedback?: () => void
}

export function useDataCenterRuleDialog(sources: DataCenterRuleDialogSources) {
  const newSymbol = ref('')
  const ruleDialogOpen = ref(false)
  const pendingRuleSymbol = ref('')
  const newSyncSpot = ref(true)
  const newSyncSwap = ref(true)
  const newArchiveAll = ref(false)
  const newAutoSync = ref(true)
  const newSyncDays = ref(DEFAULT_UNIFIED_SYNC_DAYS)
  const newSyncPlans = ref<WatchedSymbolSyncPlan[]>(normalizeFullSyncPlans([]))

  const canOpenRuleDialog = computed(() => canOpenWatchRuleDialog(newSymbol.value))
  const canSubmitRuleDialog = computed(() => canSubmitWatchRuleDialog({
    pendingSymbol: pendingRuleSymbol.value,
    syncSpot: newSyncSpot.value,
    syncSwap: newSyncSwap.value,
    syncPlans: newSyncPlans.value,
  }))
  const addButtonLabel = computed(() => watchRuleSubmitButtonLabel(sources.adding.value, newAutoSync.value))

  function openRuleDialog() {
    const symbol = normalizeInputSymbol(newSymbol.value)
    if (!symbol) return false
    sources.clearFeedback?.()
    const existing = sources.watchedSymbols.value.find(item => item.symbol === symbol)
    const inventoryBacked = sources.watchedRows.value.find(item => item.symbol === symbol)
    pendingRuleSymbol.value = symbol
    if (existing) {
      loadRuleFormFromRow(existing)
    } else if (inventoryBacked?.inventory_only) {
      loadRuleFormFromRow(inventoryBacked)
    } else {
      resetRuleFormForNewSymbol()
    }
    ruleDialogOpen.value = true
    return true
  }

  function closeRuleDialog() {
    if (sources.adding.value) return
    ruleDialogOpen.value = false
  }

  function editSymbol(row: WatchedSymbol) {
    sources.clearFeedback?.()
    newSymbol.value = row.symbol
    pendingRuleSymbol.value = row.symbol
    loadRuleFormFromRow(row)
    ruleDialogOpen.value = true
  }

  function setNewSyncDays(value: number) {
    newSyncDays.value = normalizeSyncDays(value)
    newSyncPlans.value = applyUnifiedSyncDays(newSyncPlans.value, newSyncDays.value)
  }

  function resetAfterSaved() {
    newSymbol.value = ''
    pendingRuleSymbol.value = ''
    ruleDialogOpen.value = false
    resetRuleFormForNewSymbol()
  }

  function resetRuleFormForNewSymbol() {
    applyRuleFormState(defaultWatchRuleForm(sources.guardianPlans.value, DEFAULT_UNIFIED_SYNC_DAYS))
  }

  function loadRuleFormFromRow(row: WatchedSymbol) {
    applyRuleFormState(watchRuleFormFromRow(row, sources.guardianPlans.value, newSyncDays.value))
  }

  function applyRuleFormState(state: WatchRuleFormState) {
    newSyncSpot.value = state.syncSpot
    newSyncSwap.value = state.syncSwap
    newArchiveAll.value = state.archiveAll
    newAutoSync.value = state.autoSync
    newSyncDays.value = state.syncDays
    newSyncPlans.value = state.syncPlans
  }

  return {
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
    resetRuleFormForNewSymbol,
  }
}
