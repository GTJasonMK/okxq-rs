import type {
  SeriesMarker,
  SeriesMarkerBarPosition,
  SeriesMarkerShape,
  Time,
  UTCTimestamp,
} from 'lightweight-charts'
import type { Timeframe } from '@/types'
import type { StrategyTriggerMarker, StrategyTriggerMarkerMode } from '@/types/strategy-visualization'
import { candleBucketStart } from '@/utils/marketView'
import {
  isValidTriggerMarker,
  sortMarkers,
} from '@/utils/strategyTriggers/shared'

export function toChartSeriesMarkers(
  markers: StrategyTriggerMarker[],
  timeframe: Timeframe,
  mode: StrategyTriggerMarkerMode = 'auto',
): SeriesMarker<Time>[] {
  if (mode === 'hidden') return []
  const sortedMarkers = sortMarkers(markers)
  const compact = mode === 'icon' || (mode === 'auto' && sortedMarkers.length > 80)
  const dense = compact || (mode === 'auto' && sortedMarkers.length > 40)
  return sortedMarkers
    .filter(isValidTriggerMarker)
    .map(marker => ({
      id: marker.id,
      time: Math.floor(candleBucketStart(marker.timestamp, timeframe) / 1000) as UTCTimestamp,
      position: markerPosition(marker),
      shape: markerShape(marker),
      color: markerColor(marker),
      text: markerText(marker, mode, dense, compact),
      size: markerSize(marker, compact),
    }))
}

function markerPosition(marker: StrategyTriggerMarker): SeriesMarkerBarPosition {
  if (marker.kind === 'risk' || marker.kind === 'blocked') return 'inBar'
  if (marker.kind === 'exit') return marker.side === 'buy' ? 'belowBar' : 'aboveBar'
  return marker.side === 'sell' ? 'aboveBar' : 'belowBar'
}

function markerShape(marker: StrategyTriggerMarker): SeriesMarkerShape {
  if (marker.kind === 'risk' || marker.kind === 'blocked') return 'square'
  if (marker.kind === 'pending') return 'circle'
  if (marker.kind === 'exit') return 'circle'
  return marker.side === 'sell' ? 'arrowDown' : 'arrowUp'
}

function markerColor(marker: StrategyTriggerMarker) {
  if (marker.kind === 'risk') return '#ef5350'
  if (marker.kind === 'blocked') return '#f6c85d'
  if (marker.kind === 'exit') {
    if (marker.pnl !== undefined && Number.isFinite(marker.pnl)) {
      return marker.pnl >= 0 ? '#26a69a' : '#ef5350'
    }
    return '#94a3b8'
  }
  if (marker.kind === 'pending') return '#2962ff'
  return marker.side === 'sell' ? '#ff9800' : '#26a69a'
}

function markerText(
  marker: StrategyTriggerMarker,
  mode: StrategyTriggerMarkerMode,
  dense: boolean,
  compact: boolean,
) {
  if (mode === 'text') return marker.label
  if (compact) return ''
  if (!dense) return marker.label
  if (marker.kind === 'risk') return '风控'
  if (marker.kind === 'blocked') return '拦截'
  if (marker.kind === 'exit') return '平'
  if (marker.kind === 'entry') return marker.side === 'sell' ? '空' : '多'
  return ''
}

function markerSize(marker: StrategyTriggerMarker, compact: boolean) {
  if (marker.kind === 'risk' || marker.kind === 'blocked') return compact ? 0.9 : 1.15
  return compact ? 0.9 : 1.25
}
