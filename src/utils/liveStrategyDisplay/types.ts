import type {
	  LiveOrder,
	  LiveEquityHistory,
	  LiveStrategyStatus,
	  LiveDecisionDiagnostics,
	} from '@/types'

export type LiveStrategyKpiKind = 'neutral' | 'ready' | 'blocked' | 'positive' | 'negative' | 'warning'

export type LiveStrategyKpi = {
  label: string
  value: string
  detail: string
  kind: LiveStrategyKpiKind
}

export type BuildLiveStrategyKpisInput = {
	  status: LiveStrategyStatus | null
	  orders: LiveOrder[]
	  equityHistory: LiveEquityHistory | null
	  diagnostics: LiveDecisionDiagnostics | null
	  decisionDiagnosticsLoading: boolean
	  decisionDiagnosticsScopeText: string
	  autoDecisionDiagnosticsEnabled: boolean
	}

export type DecisionKpiInput = Pick<
	  BuildLiveStrategyKpisInput,
	  'diagnostics' | 'decisionDiagnosticsLoading' | 'decisionDiagnosticsScopeText' | 'autoDecisionDiagnosticsEnabled'
	>
