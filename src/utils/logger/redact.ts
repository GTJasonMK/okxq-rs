export function redactSecrets(value: unknown, seen = new WeakSet<object>()): unknown {
  if (Array.isArray(value)) {
    return value.map(item => redactSecrets(item, seen))
  }
  if (!value || typeof value !== 'object') {
    return value
  }
  if (seen.has(value)) {
    return '[Circular]'
  }
  seen.add(value)
  const output: Record<string, unknown> = {}
  for (const [key, item] of Object.entries(value as Record<string, unknown>)) {
    if (isSecretKey(key)) {
      output[key] = maskValue(item)
    } else {
      output[key] = redactSecrets(item, seen)
    }
  }
  return output
}

function isSecretKey(key: string): boolean {
  return /api[_-]?key|secret|passphrase|password|token|authorization/i.test(key)
}

function maskValue(value: unknown): string {
  if (typeof value !== 'string' || value.length === 0) {
    return ''
  }
  if (value.length <= 8) {
    return '*'.repeat(value.length)
  }
  return `${value.slice(0, 4)}${'*'.repeat(value.length - 8)}${value.slice(-4)}`
}
