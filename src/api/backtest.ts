import { apiGet, apiPost, apiDelete } from './client'
import {
  arrayRecords,
  recordFrom,
} from './normalize'
import {
  normalizeProgress,
  normalizeResult,
  normalizeStrategy,
} from './backtest/normalize'
import type {
  BacktestMonteCarloOptions,
  BacktestMonteCarloResponse,
} from '@/types/backtest'

export function fetchStrategies() {
  return apiGet<unknown>('/api/backtest/strategies').then(data => arrayRecords(data).map(normalizeStrategy))
}

export function runBacktest(strategyId: string, data: Record<string, unknown>) {
  return apiPost<unknown>(`/api/backtest/run/${strategyId}`, data).then(data => normalizeResult(recordFrom(data)))
}

export function fetchBacktestHistory() {
  return apiGet<unknown>('/api/backtest/history').then(data => arrayRecords(data).map(normalizeResult))
}

export function fetchBacktestDetail(resultId: string) {
  return apiGet<unknown>(`/api/backtest/history/${resultId}`).then(data => normalizeResult(recordFrom(data)))
}

export function deleteBacktestResult(resultId: string) {
  return apiDelete(`/api/backtest/history/${resultId}`)
}

export function fetchBacktestProgress(runId: string) {
  return apiGet<unknown>(`/api/backtest/progress/${runId}`, undefined, { dedupe: false })
    .then(data => normalizeProgress(recordFrom(data)))
}

export function runBacktestMonteCarloAnalysis(
  resultId: string,
  options: BacktestMonteCarloOptions = {},
) {
  return apiPost<BacktestMonteCarloResponse>(`/api/backtest/monte-carlo/${resultId}`, options)
}
