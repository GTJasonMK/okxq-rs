import { apiGet, apiPost, apiPostWithParams, apiPut } from './client'
import { arrayRecords } from './normalize'
import type { InstType, Timeframe } from '@/types'
import {
  normalizeDataset,
  normalizeFactorValue,
  normalizeTrainingRun,
  normalizeTrendConfig,
  normalizeTrendConfigPayload,
  normalizeTrendFactors,
} from './research/normalize'

export type ResearchScopeOptions = {
  inst_type?: InstType | 'SPOT' | 'SWAP'
  timeframe?: Timeframe | string
}

export function fetchCollectionSessions() {
  return apiGet<unknown>('/api/research-platform/sessions').then(arrayRecords)
}

export function fetchDatasets() {
  return apiGet<unknown>('/api/research-platform/datasets')
    .then(data => arrayRecords(data).map(normalizeDataset))
}

export function fetchTrainingRuns() {
  return apiGet<unknown>('/api/research-platform/training-runs')
    .then(data => arrayRecords(data).map(normalizeTrainingRun))
}

export function computeFactors(instId: string, barCount?: number, opts?: ResearchScopeOptions) {
  return apiPostWithParams<unknown>(
    '/api/research/factors/compute',
    {
      inst_id: instId,
      inst_type: opts?.inst_type,
      timeframe: opts?.timeframe,
    },
    { bar_count: barCount ?? 120 },
  )
    .then(data => arrayRecords(data).map(normalizeFactorValue))
}

export function buildDataset(instId: string, barCount?: number, opts?: ResearchScopeOptions) {
  return apiPostWithParams<unknown>(
    '/api/research/dataset/build',
    {
      inst_id: instId,
      inst_type: opts?.inst_type,
      timeframe: opts?.timeframe,
    },
    { bar_count: barCount ?? 3600 },
  )
    .then(normalizeDataset)
}

export function trainModel(datasetId: string) {
  return apiPost<unknown>('/api/research/model/train', { dataset_id: datasetId })
    .then(normalizeTrainingRun)
}

export function fetchTrendFactors(instId: string, limit?: number, opts?: ResearchScopeOptions) {
  const params: Record<string, string | number | boolean> = { limit: limit ?? 20 }
  if (opts?.inst_type) params.inst_type = opts.inst_type
  if (opts?.timeframe) params.timeframe = opts.timeframe
  return apiGet<unknown>(`/api/trend-research/factors/${encodeURIComponent(instId)}`, params)
    .then(normalizeTrendFactors)
}

export function fetchTrendConfig() {
  return apiGet<unknown>('/api/trend-research/config').then(normalizeTrendConfig)
}

export function updateTrendConfig(data: Record<string, unknown>) {
  return apiPut('/api/trend-research/config', normalizeTrendConfigPayload(data))
}
