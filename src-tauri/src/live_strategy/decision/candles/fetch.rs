use std::collections::BTreeMap;

use sqlx::SqlitePool;

use crate::{
    error::AppResult,
    okx::{OkxCandle, OkxPublicClient},
};

use super::{
    derivation::{aggregate_candles_from_source, source_candles_required_for_target},
    freshness::{latest_local_candle_is_fresh, latest_map_candle_is_fresh, tail_candles},
    persistence::{load_local_strategy_candles, persist_and_merge_candles, save_strategy_candles},
    storage_kind::{resolve_timeframe_storage_kind_from_db, TimeframeStorageKind},
    MAX_DERIVED_SOURCE_CANDLES, OKX_CANDLE_BATCH_LIMIT,
};
use crate::live_strategy::{
    runtime_helpers::{canonical_timeframe, timeframe_millis},
    types::LiveStrategyConfig,
};

struct NewerGapFillRequest<'a> {
    db: &'a SqlitePool,
    client: &'a OkxPublicClient,
    symbol: &'a str,
    inst_type: &'a str,
    timeframe: &'a str,
    local_newest: Option<i64>,
    recent_oldest: Option<i64>,
}

pub(in crate::live_strategy) async fn fetch_strategy_candles(
    db: &SqlitePool,
    client: &OkxPublicClient,
    config: &LiveStrategyConfig,
    required_candles: usize,
) -> AppResult<Vec<OkxCandle>> {
    fetch_candles_by_timeframe(
        db,
        client,
        &config.symbol,
        &config.inst_type,
        &config.timeframe,
        required_candles,
    )
    .await
}

pub(in crate::live_strategy::decision) async fn fetch_candles_by_timeframe(
    db: &SqlitePool,
    client: &OkxPublicClient,
    symbol: &str,
    inst_type: &str,
    timeframe: &str,
    required_candles: usize,
) -> AppResult<Vec<OkxCandle>> {
    let timeframe = canonical_timeframe(timeframe)
        .map(|value| value.to_string())
        .unwrap_or_else(|| timeframe.trim().to_string());
    match resolve_timeframe_storage_kind_from_db(db, symbol, inst_type, &timeframe).await? {
        TimeframeStorageKind::Direct => {
            fetch_direct_candles_by_timeframe(
                db,
                client,
                symbol,
                inst_type,
                &timeframe,
                required_candles,
            )
            .await
        }
        TimeframeStorageKind::Derived { source_timeframe } => {
            fetch_derived_candles_by_timeframe(
                db,
                client,
                symbol,
                inst_type,
                &source_timeframe,
                &timeframe,
                required_candles,
            )
            .await
        }
    }
}

async fn fetch_direct_candles_by_timeframe(
    db: &SqlitePool,
    client: &OkxPublicClient,
    symbol: &str,
    inst_type: &str,
    timeframe: &str,
    required_candles: usize,
) -> AppResult<Vec<OkxCandle>> {
    fetch_direct_candles_by_timeframe_with_cap(
        db,
        client,
        symbol,
        inst_type,
        timeframe,
        required_candles,
        20_000,
    )
    .await
}

async fn fetch_derived_candles_by_timeframe(
    db: &SqlitePool,
    client: &OkxPublicClient,
    symbol: &str,
    inst_type: &str,
    source_timeframe: &str,
    timeframe: &str,
    required_candles: usize,
) -> AppResult<Vec<OkxCandle>> {
    let target = required_candles.clamp(3, 20_000);
    let target_local =
        load_local_strategy_candles(db, symbol, inst_type, timeframe, target).await?;
    if latest_local_candle_is_fresh(&target_local, timeframe) && target_local.len() >= target {
        return Ok(target_local);
    }

    let source_required = source_candles_required_for_target(target, source_timeframe, timeframe)?;
    fetch_direct_source_candles_by_timeframe(
        db,
        client,
        symbol,
        inst_type,
        source_timeframe,
        source_required,
    )
    .await?;
    let source_candles =
        load_local_strategy_candles(db, symbol, inst_type, source_timeframe, source_required)
            .await?;
    let derived = aggregate_candles_from_source(&source_candles, timeframe);
    save_strategy_candles(db, symbol, inst_type, timeframe, &derived).await?;
    load_local_strategy_candles(db, symbol, inst_type, timeframe, target).await
}

async fn fetch_direct_source_candles_by_timeframe(
    db: &SqlitePool,
    client: &OkxPublicClient,
    symbol: &str,
    inst_type: &str,
    timeframe: &str,
    required_candles: usize,
) -> AppResult<Vec<OkxCandle>> {
    fetch_direct_candles_by_timeframe_with_cap(
        db,
        client,
        symbol,
        inst_type,
        timeframe,
        required_candles,
        MAX_DERIVED_SOURCE_CANDLES,
    )
    .await
}

async fn fetch_direct_candles_by_timeframe_with_cap(
    db: &SqlitePool,
    client: &OkxPublicClient,
    symbol: &str,
    inst_type: &str,
    timeframe: &str,
    required_candles: usize,
    max_candles: usize,
) -> AppResult<Vec<OkxCandle>> {
    let target = required_candles.clamp(3, max_candles.max(3));
    let mut by_ts = BTreeMap::<i64, OkxCandle>::new();
    let local = load_local_strategy_candles(db, symbol, inst_type, timeframe, target).await?;
    let local_newest = local.last().map(|candle| candle.timestamp);
    let local_is_fresh = latest_local_candle_is_fresh(&local, timeframe);
    if local_is_fresh && local.len() >= target {
        return Ok(local);
    }
    for candle in local {
        by_ts.insert(candle.timestamp, candle);
    }

    let first_limit = target.min(OKX_CANDLE_BATCH_LIMIT as usize) as u32;
    let recent = client
        .get_candles(symbol, timeframe, first_limit, None, None, false)
        .await?;
    let recent_oldest = recent.first().map(|candle| candle.timestamp);
    persist_and_merge_candles(db, symbol, inst_type, timeframe, recent, &mut by_ts).await?;

    if newer_gap_exceeds_target_window(local_newest, recent_oldest, target, timeframe) {
        if let Some(recent_oldest) = recent_oldest {
            by_ts.retain(|timestamp, _| *timestamp >= recent_oldest);
        }
    } else if should_fill_newer_gap(local_newest, recent_oldest, timeframe) {
        fill_newer_gap_from_okx(
            NewerGapFillRequest {
                db,
                client,
                symbol,
                inst_type,
                timeframe,
                local_newest,
                recent_oldest,
            },
            &mut by_ts,
        )
        .await?;
    }

    if by_ts.len() >= target && latest_map_candle_is_fresh(&by_ts, timeframe) {
        return Ok(tail_candles(by_ts, target));
    }

    while by_ts.len() < target {
        let Some(oldest) = by_ts.keys().next().copied() else {
            break;
        };
        let remaining = target
            .saturating_sub(by_ts.len())
            .min(OKX_CANDLE_BATCH_LIMIT as usize) as u32;
        let batch = client
            .get_candles(
                symbol,
                timeframe,
                remaining,
                None,
                Some(oldest.to_string()),
                true,
            )
            .await?;
        if batch.is_empty() {
            break;
        }
        let previous_len = by_ts.len();
        persist_and_merge_candles(db, symbol, inst_type, timeframe, batch, &mut by_ts).await?;
        if by_ts.len() == previous_len {
            break;
        }
    }

    Ok(tail_candles(by_ts, target))
}

async fn fill_newer_gap_from_okx(
    request: NewerGapFillRequest<'_>,
    by_ts: &mut BTreeMap<i64, OkxCandle>,
) -> AppResult<()> {
    let Some(mut cursor_before) = request.local_newest else {
        return Ok(());
    };
    let stop_at = request.recent_oldest.unwrap_or(i64::MAX);
    let mut previous_newest = None;
    loop {
        let batch = request
            .client
            .get_candles(
                request.symbol,
                request.timeframe,
                OKX_CANDLE_BATCH_LIMIT,
                Some(cursor_before.to_string()),
                None,
                true,
            )
            .await?;
        let batch = batch
            .into_iter()
            .filter(|candle| candle.timestamp > cursor_before)
            .collect::<Vec<_>>();
        if batch.is_empty() {
            break;
        }
        let newest_in_batch = batch
            .iter()
            .map(|candle| candle.timestamp)
            .max()
            .unwrap_or(cursor_before);
        persist_and_merge_candles(
            request.db,
            request.symbol,
            request.inst_type,
            request.timeframe,
            batch,
            by_ts,
        )
        .await?;
        if newest_in_batch >= stop_at
            || super::freshness::latest_timestamp_is_fresh(newest_in_batch, request.timeframe)
        {
            break;
        }
        if previous_newest == Some(newest_in_batch) || newest_in_batch <= cursor_before {
            break;
        }
        previous_newest = Some(newest_in_batch);
        cursor_before = newest_in_batch;
    }
    Ok(())
}

fn should_fill_newer_gap(
    local_newest: Option<i64>,
    recent_oldest: Option<i64>,
    timeframe: &str,
) -> bool {
    let Some(local_newest) = local_newest else {
        return false;
    };
    let Some(recent_oldest) = recent_oldest else {
        return false;
    };
    let timeframe_ms = timeframe_millis(timeframe);
    timeframe_ms > 0 && recent_oldest > local_newest.saturating_add(timeframe_ms)
}

pub(super) fn newer_gap_exceeds_target_window(
    local_newest: Option<i64>,
    recent_oldest: Option<i64>,
    target: usize,
    timeframe: &str,
) -> bool {
    let Some(local_newest) = local_newest else {
        return false;
    };
    let Some(recent_oldest) = recent_oldest else {
        return false;
    };
    let timeframe_ms = timeframe_millis(timeframe);
    if timeframe_ms <= 0 || recent_oldest <= local_newest {
        return false;
    }
    let gap_bars = (recent_oldest - local_newest) / timeframe_ms;
    usize::try_from(gap_bars).is_ok_and(|gap| gap >= target)
}
