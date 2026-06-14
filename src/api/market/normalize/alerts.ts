import type { PriceAlert } from '@/types/market'
import {
  isRecord,
  stringValue,
} from '../../normalize'
import {
  inferInstTypeFromId,
  normalizeBaseSymbol,
  normalizeInstId,
  normalizeInstType,
  normalizePriceAlert,
} from '../../marketNormalize'

export function normalizePriceAlerts(raw: unknown): PriceAlert[] {
  return alertArrayValue(raw).map(normalizePriceAlert)
}

export function normalizePriceAlertPayload(data: Record<string, unknown>): Record<string, unknown> {
  const item = isRecord(data) ? data : {}
  const payload: Record<string, unknown> = {}
  const rawInstId = item.inst_id
  const rawInstType = item.inst_type

  if (typeof rawInstId === 'string' && rawInstId.trim()) {
    const instType = normalizeInstType(rawInstType, inferInstTypeFromId(rawInstId))
    payload.inst_id = normalizeInstId(rawInstId, instType)
    payload.inst_type = instType
  } else if (typeof rawInstType === 'string') {
    payload.inst_type = normalizeInstType(rawInstType)
  }

  const symbol = stringValue(item.symbol)
  if (symbol) payload.symbol = normalizeBaseSymbol(symbol)

  const alertType = normalizeAlertTypeForPayload(item.alert_type)
  if (alertType) payload.alert_type = alertType

  const direction = normalizeDirectionForPayload(item.direction)
  if (direction) payload.direction = direction

  const targetPrice = optionalPayloadNumber(item.target_price)
  if (targetPrice !== undefined) payload.target_price = targetPrice

  const changePercent = optionalPayloadNumber(item.change_percent)
  if (changePercent !== undefined) payload.change_percent = changePercent

  if (typeof item.note === 'string') payload.note = item.note.trim()
  if (typeof item.enabled === 'boolean') payload.enabled = item.enabled
  if (item.trigger_once !== undefined) {
    if (typeof item.trigger_once === 'boolean') payload.trigger_once = item.trigger_once
  }
  if (item.cooldown_seconds !== undefined) {
    if (typeof item.cooldown_seconds === 'number' && Number.isFinite(item.cooldown_seconds)) {
      payload.cooldown_seconds = Math.max(0, Math.round(item.cooldown_seconds))
    }
  }
  if (item.created_at !== undefined) {
    if (typeof item.created_at === 'string') payload.created_at = item.created_at
  }
  if (item.updated_at !== undefined) {
    if (typeof item.updated_at === 'string') payload.updated_at = item.updated_at
  }

  return payload
}

function normalizeAlertTypeForPayload(value: unknown): PriceAlert['alert_type'] | '' {
  const normalized = stringValue(value).trim().toLowerCase()
  if (normalized === 'price' || normalized === 'change') return normalized
  return ''
}

function normalizeDirectionForPayload(value: unknown): PriceAlert['direction'] | '' {
  const normalized = stringValue(value).trim().toLowerCase()
  if (normalized === 'above' || normalized === 'below') return normalized
  return ''
}

function optionalPayloadNumber(value: unknown): number | null | undefined {
  if (value === undefined) return undefined
  if (value === null) return null
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined
}

function alertArrayValue(value: unknown): unknown[] {
  if (Array.isArray(value)) return value
  if (isRecord(value) && value.success === true && Array.isArray(value.data)) return value.data
  return []
}
