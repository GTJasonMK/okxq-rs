import { serializeError } from './describeError'
import { debugEnabled, logger } from './writer'

export function installGlobalLogging(): void {
  logger.info('frontend logger initialized', {
    scope: 'startup',
    mode: import.meta.env.MODE,
    debugEnabled,
  })

  window.addEventListener('error', (event) => {
    logger.error('unhandled window error', {
      scope: 'runtime',
      message: event.message,
      filename: event.filename,
      lineno: event.lineno,
      colno: event.colno,
      error: serializeError(event.error),
    })
  })

  window.addEventListener('unhandledrejection', (event) => {
    logger.error('unhandled promise rejection', {
      scope: 'runtime',
      reason: serializeError(event.reason),
    })
  })
}
