import type {
  GuardianStatus,
} from '@/types/dataCenter'
import {
  recordFrom,
} from '../normalize'

export function guardianErrorMessages(status: GuardianStatus | null) {
  return (status?.last_errors ?? [])
    .map(guardianErrorMessage)
    .filter(Boolean)
}

export function guardianCurrentTargetText(status: GuardianStatus | null) {
  if (!status?.current_inst_id && !status?.current_timeframe) return '--'
  return [
    status.current_inst_id || '--',
    status.current_timeframe || '--',
    status.current_mode || status.current_phase || '',
  ].filter(Boolean).join(' · ')
}

function guardianErrorMessage(value: unknown) {
  if (typeof value === 'string') return value.trim()
  const message = recordFrom(value).message
  return typeof message === 'string' ? message.trim() : ''
}
