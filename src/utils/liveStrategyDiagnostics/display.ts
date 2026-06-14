import type { LiveDecisionDiagnostics } from '@/types'
import type { LiveStrategyDiagnosticTarget } from '@/utils/liveStrategyDiagnostics/types'

export function diagnosticScopeText(input: {
  target: LiveStrategyDiagnosticTarget
  loading: boolean
  hasCurrentResult: boolean
  emptyText: string
  loadingPrefix: string
}) {
  if (!input.target.strategy_id || !input.target.symbol) return input.emptyText
  const loadingText = input.loading && !input.hasCurrentResult ? `${input.loadingPrefix} · ` : ''
  return `${loadingText}${input.target.symbol} · ${input.target.timeframe} · ${input.target.strategy_id}`
}

export function decisionDiagnosticsMismatchText(diagnostics: LiveDecisionDiagnostics) {
  return `决策结果与当前查看层不一致：返回 ${diagnostics.symbol} · ${diagnostics.timeframe}`
}

export function shouldRefreshDecisionDiagnosticsOnCandle(autoEnabled: boolean, confirmed: boolean) {
  return autoEnabled && confirmed
}
