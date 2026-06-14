import type {
  AnyRecord,
  ParamDraftKind,
  ParamDraftRow,
} from '../types'
import {
  isPlainRecord,
  readableLabel,
} from '../readable'

export function draftRowsFromParams(value: AnyRecord): ParamDraftRow[] {
  return flattenDraftRows(value)
}

function flattenDraftRows(value: unknown, parentKey = '', depth = 0): ParamDraftRow[] {
  if (!isPlainRecord(value)) return []
  const rows: ParamDraftRow[] = []
  for (const [key, item] of Object.entries(value)) {
    const fullKey = parentKey ? `${parentKey}.${key}` : key
    if (isPlainRecord(item)) {
      rows.push({
        key: fullKey,
        label: readableLabel(fullKey),
        value: '',
        input: '',
        depth,
        group: true,
        multiline: false,
        kind: 'json',
        error: '',
      })
      rows.push(...flattenDraftRows(item, fullKey, depth + 1))
      continue
    }
    rows.push(draftRowFromValue({
      key: fullKey,
      label: readableLabel(fullKey),
      value: item,
      depth,
    }))
  }
  return rows
}

export function draftRowFromValue(
  row: { key: string; label: string; value: unknown; depth: number },
  kindOverride?: ParamDraftKind,
): ParamDraftRow {
  const kind = kindOverride ?? draftKind(row.value)
  const multiline = kind === 'json' && (Array.isArray(row.value) || isPlainRecord(row.value))
  return {
    key: row.key,
    label: row.label,
    value: '',
    input: draftInput(row.value, kind),
    depth: row.depth,
    group: false,
    multiline,
    kind,
    error: '',
  }
}

function draftKind(value: unknown): ParamDraftKind {
  if (typeof value === 'number') return 'number'
  if (typeof value === 'boolean') return 'boolean'
  if (typeof value === 'string') return 'string'
  return 'json'
}

function draftInput(value: unknown, kind: ParamDraftKind) {
  if (value === undefined || value === null) return ''
  if (kind === 'boolean') return value === true ? 'true' : value === false ? 'false' : ''
  if (kind === 'json') return JSON.stringify(value, null, 2)
  return String(value)
}
