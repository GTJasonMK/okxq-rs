import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { JournalEntry } from '@/types'

export const useJournalStore = defineStore('journal', () => {
  const entries = ref<JournalEntry[]>([])
  const tags = ref<string[]>([])
  const stats = ref<Record<string, unknown> | null>(null)
  const activeEntry = ref<JournalEntry | null>(null)
  const loading = ref(false)

  return { entries, tags, stats, activeEntry, loading }
})
