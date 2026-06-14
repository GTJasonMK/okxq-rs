export type LogLevel = 'debug' | 'info' | 'warn' | 'error'

export interface LogContext {
  scope?: string
  [key: string]: unknown
}
