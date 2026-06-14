import { redactSecrets } from './redact'

export function describeError(error: unknown): string {
  const message = errorToMessage(error, new WeakSet<object>()) ?? '未知错误'
  return normalizeKnownRuntimeError(message)
}

export function serializeError(error: unknown): unknown {
  if (error instanceof Error) {
    return {
      name: error.name,
      message: error.message,
      stack: error.stack,
    }
  }
  return redactSecrets(error)
}

function errorToMessage(value: unknown, seen: WeakSet<object>): string | null {
  if (value === null || value === undefined) return null

  if (typeof value === 'string') return normalizeMessage(value)
  if (typeof value === 'number' || typeof value === 'boolean' || typeof value === 'bigint') {
    return String(value)
  }

  if (Array.isArray(value)) {
    const messages = value
      .map(item => errorToMessage(item, seen))
      .filter((item): item is string => Boolean(item))
    return uniqueMessages(messages).join('；') || null
  }

  if (value instanceof Error) {
    const record = value as Error & Record<string, unknown>
    const code = primitiveText(record.code ?? record.status ?? record.statusCode)
    const raw = errorToMessage(record.raw, seen)
    const cause = errorToMessage(record.cause, seen)
    const direct = normalizeMessage(value.message)
    if (direct && !isObjectString(direct)) return prependCode(direct, code)
    if (raw) return raw
    if (cause) return cause
    const name = normalizeMessage(value.name)
    return name && name !== 'Error' ? prependCode(name, code) : code
  }

  if (typeof value === 'object') {
    if (seen.has(value)) return null
    seen.add(value)

    const record = value as Record<string, unknown>
    const code = primitiveText(record.code ?? record.status ?? record.statusCode)
    for (const key of ['message', 'msg', 'error', 'detail', 'reason', 'description']) {
      const message = errorToMessage(record[key], seen)
      if (message) return prependCode(message, code)
    }

    const nestedData = errorToMessage(record.data, seen)
    if (nestedData) return prependCode(nestedData, code)

    const json = normalizeMessage(safeJson(redactSecrets(value)))
    if (json && json !== '{}' && json !== '[]') return prependCode(json, code)
    return code
  }

  try {
    return normalizeMessage(String(value))
  } catch {
    return null
  }
}

function normalizeMessage(value: unknown): string | null {
  if (typeof value !== 'string') return null
  const text = value.trim()
  if (!text || isObjectString(text)) return null
  return text
}

function normalizeKnownRuntimeError(message: string): string {
  if (/Command\s+local_api_request\s+not\s+found/i.test(message)) {
    return [
      'Tauri 后端命令 local_api_request 未加载到当前窗口',
      '这通常是只启动了前端 dev server，或当前桌面窗口仍在运行旧后端',
      '请关闭现有 OKXQ 窗口，执行 npm run dev:cleanup 后再用 npm run dev 启动完整桌面端',
    ].join('；')
  }
  return message
}

function isObjectString(value: string): boolean {
  return value === '[object Object]'
}

function primitiveText(value: unknown): string | null {
  if (typeof value === 'string') return normalizeMessage(value)
  if (typeof value === 'number' || typeof value === 'boolean' || typeof value === 'bigint') {
    return String(value)
  }
  return null
}

function prependCode(message: string, code: string | null): string {
  if (!code || message.startsWith(`${code}:`) || message.startsWith(`${code}：`)) return message
  return `${code}: ${message}`
}

function uniqueMessages(messages: string[]): string[] {
  return Array.from(new Set(messages))
}

function safeJson(value: unknown): string | null {
  try {
    return JSON.stringify(value, (_key, item) => typeof item === 'bigint' ? String(item) : item)
  } catch {
    return null
  }
}
