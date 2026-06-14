const ACTION_LABELS: Record<string, string> = {
  open_position: '开仓',
  close_position: '平仓',
  place_risk_order: '保护单',
  cancel_order: '撤单',
  modify_order: '改单',
  hold: '观望',
  risk_blocked: '风控拦截',
  blocked: '监控记录',
  skipped_action: '动作合约错误',
}

const EXIT_ACTIONS = new Set(['close_position'])
const BLOCKED_ACTIONS = new Set(['risk_blocked', 'blocked', 'skipped_action'])

export function liveActionName(action: string): string {
  return String(action || '').trim().toLowerCase()
}

export function isLiveExitAction(action: string): boolean {
  return EXIT_ACTIONS.has(liveActionName(action))
}

export function isLiveBlockedAction(action: string): boolean {
  return BLOCKED_ACTIONS.has(liveActionName(action))
}

export function liveExitReasonLabel(action: string): string {
  return ACTION_LABELS[liveActionName(action)] || '平仓'
}

export function liveActionLabel(action: string): string {
  const normalized = liveActionName(action)
  return ACTION_LABELS[normalized] || normalized || '--'
}
