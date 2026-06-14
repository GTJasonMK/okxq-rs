import { apiGet, apiPost, apiPut } from './client'
import type * as T from '@/types/journal'
import type { LocalApiParam } from '@/types/api'
import {
  arrayRecords,
  recordFrom,
} from './normalize'
import {
  normalizeEntry,
  normalizeEntryPayload,
  normalizeStats,
  normalizeTag,
} from './journal/normalize'

export function fetchEntries(params?: Record<string, LocalApiParam>) {
  return apiGet<unknown>('/api/journal/entries', params)
    .then(data => arrayRecords(data).map(normalizeEntry))
}

export function createEntry(data: Partial<T.JournalEntry>) {
  return apiPost<unknown>('/api/journal/entries', normalizeEntryPayload(data))
    .then(data => normalizeEntry(recordFrom(data)))
}

export function updateEntry(entryId: string, data: Partial<T.JournalEntry>) {
  return apiPut<unknown>(`/api/journal/entries/${entryId}`, normalizeEntryPayload(data))
    .then(data => normalizeEntry(recordFrom(data)))
}

export function fetchTags() {
  return apiGet<unknown>('/api/journal/tags').then(data => arrayRecords(data).map(normalizeTag))
}

export function fetchStats() {
  return apiGet<unknown>('/api/journal/stats').then(normalizeStats)
}
