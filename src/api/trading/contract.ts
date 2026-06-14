import type * as T from '@/types/trading'
import {
  apiGet,
  apiPost,
} from '../client'
import { arrayRecords } from '../normalize'
import {
  normalizeContractAccountConfig,
  normalizeContractLeverage,
} from './normalize'
import { modeParams } from './shared'

export function fetchContractAccountConfig(mode?: string) {
  return apiGet<unknown>('/api/trading/contract/account-config', modeParams(mode))
    .then(normalizeContractAccountConfig)
}

export function fetchContractLeverage(instId: string, opts?: {
  mode?: string
  mgn_mode?: string
}) {
  return apiGet<unknown>(
    `/api/trading/contract/leverage/${encodeURIComponent(instId)}`,
    {
      ...modeParams(opts?.mode),
      ...(opts?.mgn_mode ? { mgn_mode: opts.mgn_mode } : {}),
    },
  ).then(data => arrayRecords(data).map(normalizeContractLeverage))
}

export function setLeverage(instId: string, lever: number, opts?: {
  mode?: string
  mgn_mode?: string
  pos_side?: string
}) {
  const normalizedInstId = String(instId ?? '').trim()
  const normalizedLever = Number.isFinite(lever) ? String(lever) : ''
  return apiPost('/api/trading/contract/set-leverage', {
    inst_id: normalizedInstId,
    lever: normalizedLever,
    ...modeParams(opts?.mode),
    ...(opts?.mgn_mode ? { mgn_mode: opts.mgn_mode } : {}),
    ...(opts?.pos_side ? { pos_side: opts.pos_side } : {}),
  })
}

export function setPositionMode(posMode: T.PositionMode, mode?: string) {
  return apiPost('/api/trading/contract/set-position-mode', {
    pos_mode: posMode,
    ...modeParams(mode),
  })
}
