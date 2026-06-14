import { describeError } from '@/utils/logger'

export type SettledErrorSource = {
  label: string
  result: PromiseSettledResult<unknown>
}

export function settledErrorMessages(
  sources: SettledErrorSource[],
  describe: (reason: unknown) => string = describeError,
) {
  return sources
    .map(source => source.result.status === 'rejected'
      ? `${source.label}: ${describe(source.result.reason)}`
      : ''
    )
    .filter(Boolean)
}

export function settledErrorMessage(
  results: readonly PromiseSettledResult<unknown>[],
  labels: readonly string[],
) {
  const errors = settledErrorMessages(results.map((result, index) => ({
    label: labels[index] ?? `任务 ${index + 1}`,
    result,
  })))
  return errors.length > 0 ? errors.join('；') : null
}
