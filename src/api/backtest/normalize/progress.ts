import type * as T from '@/types/backtest'
import {
  isRecord,
  stringValue,
} from '../../normalize'
import type { AnyRecord } from './types'
import { numberValue } from './numbers'

export function normalizeProgress(raw: AnyRecord): T.BacktestProgress {
  return {
    run_id: stringValue(raw.run_id),
    strategy_id: stringValue(raw.strategy_id),
    status: progressStatus(raw.status),
    stage: stringValue(raw.stage),
    message: stringValue(raw.message),
    progress: clampPercent(numberValue(raw.progress)),
    processed_candles: numberValue(raw.processed_candles),
    total_candles: numberValue(raw.total_candles),
    strategy_progress: isRecord(raw.strategy_progress) ? raw.strategy_progress : undefined,
    started_at: stringValue(raw.started_at),
    updated_at: stringValue(raw.updated_at),
  }
}

function progressStatus(value: unknown): T.BacktestProgress['status'] {
  if (value === 'running' || value === 'completed' || value === 'failed' || value === 'idle') {
    return value
  }
  return 'idle'
}

function clampPercent(value: number): number {
  return Math.max(0, Math.min(100, Math.round(value)))
}
