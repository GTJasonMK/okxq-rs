export function finiteNumber(value: unknown) {
  return typeof value === 'number' && Number.isFinite(value) ? value : null
}

export function positiveNumber(value: unknown) {
  const number = finiteNumber(value)
  return number !== null && number > 0 ? number : null
}

export function firstPositiveNumber(...values: unknown[]) {
  for (const value of values) {
    const number = positiveNumber(value)
    if (number !== null) return number
  }
  return null
}
