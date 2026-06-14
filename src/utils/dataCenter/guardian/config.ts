import {
  DEFAULT_UNIFIED_SYNC_DAYS,
} from '@/utils/syncPlans'
import type {
  GuardianConfig,
  GuardianPlan,
  GuardianSettings,
} from '@/types/dataCenter'
import {
  arrayRecords,
  booleanValue,
  numberValue,
  recordFrom,
  stringValue,
} from '../normalize'
import { normalizeGuardianStatus } from './status'

export function normalizeGuardianConfig(value: unknown): GuardianConfig {
  const raw = recordFrom(value)
  const settings = normalizeGuardianSettings(raw.settings)
  return {
    settings,
    defaults: normalizeGuardianSettings(raw.defaults, settings),
    ...(raw.status !== undefined ? { status: normalizeGuardianStatus(raw.status) } : {}),
  }
}

function normalizeGuardianSettings(value: unknown, baseSettings?: GuardianSettings): GuardianSettings {
  const raw = recordFrom(value)
  const base = baseSettings ?? {
    enabled: true,
    scan_interval_seconds: 300,
    max_full_backfill_jobs_per_cycle: 1,
    plans: [],
  }
  const plans = arrayRecords(raw.plans).map(normalizeGuardianPlan).filter((plan): plan is GuardianPlan => !!plan)
  return {
    enabled: booleanValue(raw.enabled, base.enabled),
    scan_interval_seconds: numberValue(raw.scan_interval_seconds, base.scan_interval_seconds),
    max_full_backfill_jobs_per_cycle: numberValue(
      raw.max_full_backfill_jobs_per_cycle,
      base.max_full_backfill_jobs_per_cycle,
    ),
    plans: plans.length > 0 ? plans : base.plans,
  }
}

function normalizeGuardianPlan(value: unknown): GuardianPlan | null {
  const raw = recordFrom(value)
  const timeframe = stringValue(raw.timeframe).trim()
  if (!timeframe) return null
  return {
    timeframe,
    enabled: booleanValue(raw.enabled, true),
    bootstrap_days: numberValue(raw.bootstrap_days, DEFAULT_UNIFIED_SYNC_DAYS),
    archive_mode: stringValue(raw.archive_mode).trim().toLowerCase() === 'full' ? 'full' : 'rolling',
  }
}
