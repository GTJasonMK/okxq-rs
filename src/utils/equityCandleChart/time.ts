import type { Time } from 'lightweight-charts'

export function chartTimeSecond(time: Time | undefined): number {
  if (typeof time === 'number' && Number.isFinite(time) && time > 0) return Math.floor(time)
  if (time && typeof time === 'object' && 'year' in time && 'month' in time && 'day' in time) {
    return Math.floor(Date.UTC(time.year, time.month - 1, time.day) / 1000)
  }
  return 0
}
