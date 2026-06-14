import { formatPercent, modeLabel } from '@/utils/liveStrategyCore'
import { formatMoney } from '@/utils/format'

export type LiveActionPhase = 'idle' | 'starting' | 'stopping'
export type LiveLaunchReadiness = {
  kind: 'busy' | 'locked' | 'blocked' | 'ready'
  title: string
  detail: string
}

type LiveControlForm = {
  strategy_id: string
  symbol: string
  timeframe: string
  initial_capital: number
  position_size: number
}

export function liveActionBusyText(actionPhase: LiveActionPhase) {
  if (actionPhase === 'starting') return '策略启动中...'
  if (actionPhase === 'stopping') return '策略停止中...'
  return '处理中...'
}

export function liveLaunchReadiness(input: {
  actionLoading: boolean
  actionPhase: LiveActionPhase
  actionBusyText: string
  formLocked: boolean
  startDisabledReason: string
  launchMode: string
  form: LiveControlForm
}): LiveLaunchReadiness {
  if (input.actionLoading) {
    return {
      kind: 'busy',
      title: input.actionPhase === 'stopping' ? '停止中' : '启动中',
      detail: input.actionBusyText,
    }
  }
  if (input.formLocked) {
    return {
      kind: 'locked',
      title: '运行中',
      detail: '当前配置已锁定；停止策略后才能切换策略文件。',
    }
  }
  if (input.startDisabledReason) {
    return { kind: 'blocked', title: '未就绪', detail: input.startDisabledReason }
  }
  return {
    kind: 'ready',
    title: '可启动',
    detail: `${modeLabel(input.launchMode)} · ${input.form.symbol} · ${input.form.timeframe} · 资金${formatMoney(input.form.initial_capital)} · 仓位${formatPercent(input.form.position_size)}`,
  }
}

export function liveRiskScopeNote() {
  return ''
}
