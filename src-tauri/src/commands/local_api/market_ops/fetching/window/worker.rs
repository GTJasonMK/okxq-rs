use std::{collections::BTreeMap, sync::Arc};

use crate::okx::{OkxCandle, OkxPublicClient};

use super::super::super::{check_sync_cancel, AppResult, SyncCancelGuard, OKX_CANDLE_BATCH_LIMIT};
use super::super::paging::pause_okx_page;
use super::{progress::WindowFetchProgress, slices::WindowFetchSlice};

#[derive(Debug)]
pub(super) struct WindowFetchSliceResult {
    pub(super) index: i64,
    pub(super) candles: Vec<OkxCandle>,
    pub(super) batches: i64,
    pub(super) api_calls: i64,
}

pub(super) async fn fetch_window_slice(
    client: OkxPublicClient,
    inst_id: String,
    timeframe: String,
    slice: WindowFetchSlice,
    progress: Arc<WindowFetchProgress>,
    cancel_guard: Option<SyncCancelGuard>,
) -> AppResult<WindowFetchSliceResult> {
    let mut candles = BTreeMap::<i64, OkxCandle>::new();
    let mut cursor_after = Some(slice.end_ts.to_string());
    let mut previous_oldest = None;
    let mut batches = 0i64;
    let mut api_calls = 0i64;

    loop {
        check_sync_cancel(cancel_guard.as_ref()).await?;
        let batch = client
            .get_candles(
                &inst_id,
                &timeframe,
                OKX_CANDLE_BATCH_LIMIT,
                None,
                cursor_after.clone(),
                true,
            )
            .await?;
        api_calls += 1;
        if batch.is_empty() {
            break;
        }

        let oldest = batch.first().map(|item| item.timestamp).unwrap_or(0);
        let mut added = 0i64;
        for candle in batch {
            if candle.timestamp >= slice.start_ts
                && candle.timestamp < slice.end_ts
                && candles.insert(candle.timestamp, candle).is_none()
            {
                added += 1;
            }
        }
        batches += 1;
        let (global_fetched, global_batches, global_api_calls) = progress.record_batch(added);
        tracing::debug!(
            inst_id = %inst_id,
            timeframe = %timeframe,
            slice_index = slice.index,
            slice_start_ts = slice.start_ts,
            slice_end_ts = slice.end_ts,
            slice_batches = batches,
            slice_fetched = candles.len(),
            global_fetched,
            target_fetch_count = progress.target_fetch_count,
            global_batches,
            target_batches = progress.target_batches,
            global_api_calls,
            "window candle slice fetched"
        );

        if oldest <= slice.start_ts || previous_oldest == Some(oldest) {
            break;
        }
        previous_oldest = Some(oldest);
        cursor_after = Some(oldest.to_string());
        pause_okx_page().await;
    }

    Ok(WindowFetchSliceResult {
        index: slice.index,
        candles: candles.into_values().collect(),
        batches,
        api_calls,
    })
}
