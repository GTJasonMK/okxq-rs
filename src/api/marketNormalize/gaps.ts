import type {
  MarketGapPlan,
  MarketGapRangePlan,
} from '@/types/market'
import {
  booleanValue,
  nullableTimestampNumber as optionalTimestampValue,
  numberValue,
  recordFrom,
  stringValue as textValue,
  timestampNumber as timestampValue,
} from '../normalize'
import {
  inferInstTypeFromId,
  normalizeInstId,
  normalizeInstType,
  normalizeTimeframe,
  normalizeTimeframeList,
} from './core'

export function normalizeMarketGapPlan(raw: unknown): MarketGapPlan {
  const item = recordFrom(raw)
  const rawInstId = textValue(item.inst_id)
  const instType = normalizeInstType(item.inst_type, inferInstTypeFromId(rawInstId))
  const timeframe = normalizeTimeframe(textValue(item.timeframe, '1m')) || '1m'
  const sourceTimeframe = normalizeTimeframe(textValue(item.source_timeframe, '1m')) || '1m'
  const methods = recordFrom(item.methods)
  return {
    inst_id: normalizeInstId(rawInstId, instType),
    inst_type: instType,
    timeframe: timeframe as MarketGapPlan['timeframe'],
    source_timeframe: sourceTimeframe as MarketGapPlan['source_timeframe'],
    target_timeframes: normalizeTimeframeList(item.target_timeframes) as MarketGapPlan['target_timeframes'],
    range: normalizeGapPlanRange(item.range),
    local_range: normalizeGapPlanLocalRange(item.local_range),
    expected_candles: numberValue(item.expected_candles),
    available_candles: numberValue(item.available_candles),
    missing_candles: numberValue(item.missing_candles),
    coverage_ratio: numberValue(item.coverage_ratio),
    gap_event_count: numberValue(item.gap_event_count),
    returned_gap_count: numberValue(item.returned_gap_count),
    returned_missing_candles: numberValue(item.returned_missing_candles),
    truncated: booleanValue(item.truncated),
    max_internal_gap_ms: numberValue(item.max_internal_gap_ms),
    methods: {
      paginated_ranges: numberValue(methods.paginated_ranges),
      historical_zip_ranges: numberValue(methods.historical_zip_ranges),
    },
    gaps: Array.isArray(item.gaps)
      ? item.gaps.map(normalizeMarketGapRangePlan)
      : [],
  }
}

function normalizeGapPlanRange(raw: unknown): MarketGapPlan['range'] {
  const item = recordFrom(raw)
  return {
    start_ts: timestampValue(item.start_ts),
    end_ts: timestampValue(item.end_ts),
    start_time: textValue(item.start_time) || null,
    end_time: textValue(item.end_time) || null,
  }
}

function normalizeGapPlanLocalRange(raw: unknown): MarketGapPlan['local_range'] {
  const item = recordFrom(raw)
  return {
    oldest_timestamp: optionalTimestampValue(item.oldest_timestamp),
    newest_timestamp: optionalTimestampValue(item.newest_timestamp),
    oldest_time: textValue(item.oldest_time) || null,
    newest_time: textValue(item.newest_time) || null,
  }
}

function normalizeMarketGapRangePlan(raw: unknown): MarketGapRangePlan {
  const item = recordFrom(raw)
  const method = textValue(item.method).trim() === 'historical_zip'
    ? 'historical_zip'
    : 'paginated'
  const fetchTimeframe = normalizeTimeframe(textValue(item.fetch_timeframe, '1m')) || '1m'
  return {
    start_ts: timestampValue(item.start_ts),
    end_ts: timestampValue(item.end_ts),
    start_time: textValue(item.start_time) || null,
    end_time: textValue(item.end_time) || null,
    span_ms: numberValue(item.span_ms),
    missing_candles: numberValue(item.missing_candles),
    method,
    reason: textValue(item.reason),
    fetch_timeframe: fetchTimeframe as MarketGapRangePlan['fetch_timeframe'],
    target_timeframes: normalizeTimeframeList(item.target_timeframes) as MarketGapRangePlan['target_timeframes'],
    requires_derivation: booleanValue(item.requires_derivation),
    zip: normalizeGapPlanZipSource(item.zip),
  }
}

function normalizeGapPlanZipSource(raw: unknown): MarketGapRangePlan['zip'] {
  const item = recordFrom(raw)
  if (Object.keys(item).length === 0) return null
  const sourceTimeframe = normalizeTimeframe(textValue(item.source_timeframe, '1m')) || '1m'
  const dateAggrType = textValue(item.date_aggr_type).trim() === 'monthly'
    ? 'monthly'
    : 'daily'
  return {
    provider: textValue(item.provider),
    module: textValue(item.module),
    date_aggr_type: dateAggrType,
    source_timeframe: sourceTimeframe as MarketGapPlan['source_timeframe'],
  }
}
