import type { AnyRecord } from '../types'
import { isPlainRecord } from '../readable'

export function setNestedValue(target: AnyRecord, path: string, value: unknown) {
  const keys = path.split('.').filter(Boolean)
  if (keys.length === 0) return
  let cursor = target
  for (const key of keys.slice(0, -1)) {
    if (!isPlainRecord(cursor[key])) {
      cursor[key] = {}
    }
    cursor = cursor[key] as AnyRecord
  }
  cursor[keys[keys.length - 1]] = value
}
