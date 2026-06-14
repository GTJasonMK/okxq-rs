import type {
  AnyRecord,
  ParamDraftRow,
  ParsedDraftValue,
} from '../types'
import { setNestedValue } from './path'

export function buildParamsFromDraftRows(
  rows: ParamDraftRow[],
  emptyMode: 'strict' | 'skip-empty',
): AnyRecord | null {
  const output: AnyRecord = {}
  let valid = true
  for (const row of rows) {
    row.error = ''
    if (row.group) continue
    const parsed = parseDraftValue(row, emptyMode)
    if (!parsed.ok) {
      row.error = parsed.error
      valid = false
      continue
    }
    if (parsed.skip) continue
    setNestedValue(output, row.key, parsed.value)
  }
  return valid ? output : null
}

function parseDraftValue(row: ParamDraftRow, emptyMode: 'strict' | 'skip-empty'): ParsedDraftValue {
  const raw = String(row.input ?? '').trim()
  if (raw.length === 0 && emptyMode === 'skip-empty') {
    return { ok: true, skip: true }
  }
  if (row.kind === 'number') {
    if (raw.length === 0) return { ok: false, error: '请输入有效数字' }
    const value = Number(raw)
    if (!Number.isFinite(value)) return { ok: false, error: '请输入有效数字' }
    return { ok: true, skip: false, value }
  }
  if (row.kind === 'boolean') {
    if (raw === 'true') return { ok: true, skip: false, value: true }
    if (raw === 'false') return { ok: true, skip: false, value: false }
    return { ok: false, error: '请选择是或否' }
  }
  if (row.kind === 'select') {
    if (!raw) return { ok: false, error: '请选择有效选项' }
    if (row.options?.length && !row.options.some(option => option.value === raw)) {
      return { ok: false, error: '请选择有效选项' }
    }
    return { ok: true, skip: false, value: raw }
  }
  if (row.kind === 'json') {
    try {
      return { ok: true, skip: false, value: raw.length > 0 ? JSON.parse(raw) : null }
    } catch {
      return { ok: false, error: 'JSON 格式无效' }
    }
  }
  return { ok: true, skip: false, value: row.input }
}
