import type { SyncJob } from '@/types'

export function syncJobSortTime(job: SyncJob) {
  const value = job.updated_at || job.finished_at || job.created_at
  return parseDateTimeMs(value)
}

export function parseDateTimeMs(value?: string | null) {
  if (!value) return 0
  const normalized = normalizeDateTime(value)
  if (normalized) {
    const normalizedParsed = Date.parse(normalized)
    if (Number.isFinite(normalizedParsed)) return normalizedParsed
  }
  const parsed = Date.parse(value)
  if (Number.isFinite(parsed)) return parsed
  return 0
}

function normalizeDateTime(value: string) {
  const trimmed = value.trim()
  if (!trimmed) return ''
  const normalized = trimmed
    .replace(' ', 'T')
    .replace(/\.(\d{3})\d+(?=(Z|[+-]\d{2}:?\d{2})$)/, '.$1')
    .replace(/([+-]\d{2})(\d{2})$/, '$1:$2')
  if (/(Z|[+-]\d{2}:?\d{2})$/.test(normalized)) return normalized
  if (/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/.test(normalized)) return `${normalized}Z`
  return normalized
}
