import type { SyncTaskSubmissionResult } from '@/utils/dataCenter/watchlist/types'

export function watchRuleSavedAction(existed: boolean, wasInventoryOnly: boolean) {
  if (existed) return '已更新采集规则'
  if (wasInventoryOnly) return '已接管库内标的规则'
  return '已新增数据标的规则'
}

export function syncTaskSubmissionSummary(result: SyncTaskSubmissionResult) {
  const started = result.started_count ?? result.sync_jobs?.length ?? 0
  const reused = result.reused_count ?? 0
  const exactGapJobs = result.exact_gap_jobs ?? 0
  const ruleJobs = result.rule_jobs ?? 0
  const parts = [`新增 ${started} 个任务`]
  if (reused > 0) parts.push(`复用 ${reused} 个任务`)
  if (exactGapJobs > 0) parts.push(`精确缺口 ${exactGapJobs} 个`)
  if (ruleJobs > 0) parts.push(`规则同步 ${ruleJobs} 个`)
  return parts.join('，')
}

export function repairWatchedSymbolMessage(symbol: string, result: SyncTaskSubmissionResult) {
  return `${symbol} 已按关注规则提交补齐，${syncTaskSubmissionSummary(result)}`
}

export function sameSyncRuntimeSettings<T extends object>(left: T, right: T) {
  return (Object.keys(left) as Array<keyof T>).every(key => left[key] === right[key])
}
