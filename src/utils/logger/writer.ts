import { redactSecrets } from './redact'
import type { LogContext, LogLevel } from './types'

const isDev = import.meta.env.DEV
export const debugEnabled = isDev || import.meta.env.VITE_OKXQ_DEBUG === 'true'

function write(level: LogLevel, message: string, context?: LogContext): void {
  if (!debugEnabled && level !== 'error') {
    return
  }
  const payload = context ? sanitizeContext(context) : undefined
  const prefix = context?.scope ? `[okxq:${context.scope}]` : '[okxq]'
  const line = `${prefix} ${message}`

  if (level === 'error') {
    console.error(line, payload ?? '')
  } else if (level === 'warn') {
    console.warn(line, payload ?? '')
  } else if (level === 'debug') {
    console.debug(line, payload ?? '')
  } else {
    console.info(line, payload ?? '')
  }
}

export const logger = {
  debug(message: string, context?: LogContext) {
    write('debug', message, context)
  },
  info(message: string, context?: LogContext) {
    write('info', message, context)
  },
  warn(message: string, context?: LogContext) {
    write('warn', message, context)
  },
  error(message: string, context?: LogContext) {
    write('error', message, context)
  },
}

function sanitizeContext(context: LogContext): LogContext {
  return redactSecrets(context) as LogContext
}
