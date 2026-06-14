import type { AnyRecord, ReadableParamRow } from './types'
import {
  enumValueLabels,
  keyTokenLabels,
  paramLabels,
} from './labels'
import {
  formatBars,
  formatLeverage,
  formatRate,
  formatRatio,
  formatSignedPlainNumber,
} from './format'

export function readableParamRows(value: AnyRecord): ReadableParamRow[] {
  return flattenReadableRows(value)
}

function flattenReadableRows(value: unknown, parentKey = '', depth = 0): ReadableParamRow[] {
  if (!isPlainRecord(value)) return []
  const entries = Object.entries(value)
  const rows: ReadableParamRow[] = []
  for (const [key, item] of entries) {
    const fullKey = parentKey ? `${parentKey}.${key}` : key
    if (isPlainRecord(item)) {
      rows.push({
        key: fullKey,
        label: readableLabel(fullKey),
        value: '',
        depth,
        group: true,
        multiline: false,
      })
      rows.push(...flattenReadableRows(item, fullKey, depth + 1))
      continue
    }
    const formatted = formatParamValue(fullKey, item)
    rows.push({
      key: fullKey,
      label: readableLabel(fullKey),
      value: formatted.value,
      depth,
      group: false,
      multiline: formatted.multiline,
    })
  }
  return rows
}

export function isPlainRecord(value: unknown): value is AnyRecord {
  return Boolean(value && typeof value === 'object' && !Array.isArray(value))
}

export function readableLabel(path: string) {
  const exact = paramLabels[path]
  if (exact) return exact
  const key = path.split('.').pop() ?? path
  return paramLabels[key] ?? humanizeKey(key)
}

function humanizeKey(key: string) {
  const words = key
    .split(/[_\s.-]+/)
    .filter(Boolean)
    .map(word => keyTokenLabels[word] ?? upperKnownToken(word))
  return words.length > 0 ? words.join('') : key
}

function upperKnownToken(word: string) {
  const upper = word.toUpperCase()
  if (upper === 'ML' || upper === 'RSI' || upper === 'ATR' || upper === 'EMA' || upper === 'SMA') {
    return upper
  }
  return word
}

function formatParamValue(key: string, value: unknown): { value: string; multiline: boolean } {
  if (Array.isArray(value) || isPlainRecord(value)) {
    return { value: JSON.stringify(value, null, 2), multiline: true }
  }
  if (typeof value === 'boolean') {
    return { value: value ? '是' : '否', multiline: false }
  }
  if (typeof value === 'string') {
    return { value: enumValueLabels[value] ?? value, multiline: false }
  }
  if (typeof value === 'number' && Number.isFinite(value)) {
    if (isRateField(key)) return { value: formatRate(value), multiline: false }
    if (key.endsWith('leverage') || key.endsWith('.leverage')) {
      return { value: formatLeverage(value), multiline: false }
    }
    if (key.endsWith('delay_bars') || key.endsWith('max_hold_bars')) {
      return { value: formatBars(value), multiline: false }
    }
    if (key.endsWith('position_size')) {
      return { value: formatRatio(value), multiline: false }
    }
    if (key.endsWith('total_funding')) {
      return { value: formatSignedPlainNumber(value), multiline: false }
    }
    return { value: value.toLocaleString('en-US', { maximumFractionDigits: 8 }), multiline: false }
  }
  if (value === null || value === undefined) {
    return { value: '--', multiline: false }
  }
  return { value: String(value), multiline: false }
}

function isRateField(key: string) {
  return [
    'rate',
    'ratio',
    'pct',
    'commission_rate',
    'slippage_rate',
    'funding_rate_8h',
    'maintenance_margin_rate',
    'stop_loss',
    'take_profit',
  ].some(token => key.endsWith(token) || key.includes(`${token}.`))
}
