import { onMounted, ref } from 'vue'
import * as api from '@/api/journal'
import { useJournalStore } from '@/stores/journalStore'
import type { JournalEntry } from '@/types'
import { describeError } from '@/utils/logger'

export function useJournalView() {
  const store = useJournalStore()
  const error = ref<string | null>(null)
  const message = ref<string | null>(null)

  async function loadEntries() {
    error.value = null
    try {
      store.entries = await api.fetchEntries()
    } catch (e) {
      error.value = describeError(e)
    }
  }

  async function saveEntry(data: Partial<JournalEntry>) {
    error.value = null
    message.value = null
    try {
      if (store.activeEntry?.id) {
        await api.updateEntry(store.activeEntry.id, data)
        message.value = '日志已更新'
      } else {
        await api.createEntry(data)
        message.value = '日志已创建'
      }
      await loadEntries()
      store.activeEntry = null
    } catch (e) {
      error.value = describeError(e)
    }
  }

  function selectEntry(entry: JournalEntry) {
    error.value = null
    message.value = null
    store.activeEntry = entry
  }

  function startNew() {
    error.value = null
    message.value = null
    store.activeEntry = null
  }

  onMounted(() => {
    void loadEntries()
  })

  return {
    store,
    error,
    message,
    saveEntry,
    selectEntry,
    startNew,
  }
}
