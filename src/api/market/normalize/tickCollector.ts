import type {
  TickCollectorActionResult,
} from '@/types/dataCenter'
import {
  isRecord,
  stringValue,
} from '../../normalize'
import {
  normalizeTickCollectorStatus,
} from '@/utils/dataCenter'

export function normalizeTickCollectorActionResult(raw: unknown): TickCollectorActionResult {
  const item = isRecord(raw) ? raw : {}
  return {
    message: stringValue(item.message),
    status: normalizeTickCollectorStatus(item.status),
    realtime: item.realtime,
  }
}
