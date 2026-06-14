export function modeLabel(mode: string) {
  return mode === 'live' ? '实盘' : '模拟盘'
}

export function shortRunId(runId: string) {
  const value = runId.trim()
  if (!value) return '—'
  return value.length > 18 ? `${value.slice(0, 18)}…` : value
}

export function formatRuntimeRefreshTime(timestamp: number) {
  return new Date(timestamp).toLocaleTimeString('zh-CN', {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  })
}
