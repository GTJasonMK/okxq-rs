export function formatCount(value: number) {
  return new Intl.NumberFormat('zh-CN').format(Math.max(0, Math.round(value || 0)))
}

export function formatList(value?: string[] | null) {
  if (!Array.isArray(value) || value.length === 0) return '--'
  return value.filter(Boolean).join(' / ') || '--'
}

export function formatTime(value?: string) {
  if (!value) return '--'
  const parsed = new Date(value)
  if (Number.isNaN(parsed.getTime())) return '--'
  return parsed.toLocaleString('zh-CN', { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' })
}

export function formatDateTimeValue(value?: string | number | null) {
  if (value === null || value === undefined || value === '') return '--'
  const timestamp = typeof value === 'number' && value > 0 && value < 10_000_000_000
    ? value * 1000
    : value
  const parsed = new Date(timestamp)
  if (Number.isNaN(parsed.getTime()) || parsed.getTime() <= 0) return '--'
  return parsed.toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })
}

export function formatOptionalDateTime(value?: string | number | null) {
  const formatted = formatDateTimeValue(value)
  return formatted === '--' ? '' : formatted
}
