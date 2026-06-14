import type { LiveDecisionDiagnostics } from '@/types'
import type { LiveStrategyDiagnosticTarget } from '@/utils/liveStrategyDiagnostics/types'

export function decisionDiagnosticsMatchesTarget(
  diagnostics: LiveDecisionDiagnostics | null,
  target: LiveStrategyDiagnosticTarget,
) {
  return Boolean(
    diagnostics &&
    target.strategy_id &&
    target.symbol &&
    diagnostics.strategy_id === target.strategy_id &&
    diagnostics.symbol === target.symbol &&
    diagnostics.timeframe === target.timeframe
  )
}

export function currentDecisionDiagnosticsForTarget(
  diagnostics: LiveDecisionDiagnostics | null,
  storedTargetKey: string,
  currentTargetKey: string,
  target: LiveStrategyDiagnosticTarget,
) {
  return storedTargetKey === currentTargetKey && decisionDiagnosticsMatchesTarget(diagnostics, target)
    ? diagnostics
    : null
}

export function decisionDiagnosticsIsStale(
  diagnostics: LiveDecisionDiagnostics | null,
  storedTargetKey: string,
  currentTargetKey: string,
  target: LiveStrategyDiagnosticTarget,
) {
  return storedTargetKey !== currentTargetKey || !decisionDiagnosticsMatchesTarget(diagnostics, target)
}
