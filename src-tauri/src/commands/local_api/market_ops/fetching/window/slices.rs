use super::super::super::{
    estimated_batches_for_count, timeframe_to_ms, window_fetch_batches_per_slice,
    OKX_CANDLE_BATCH_LIMIT,
};

#[derive(Clone, Debug)]
pub(super) struct WindowFetchSlice {
    pub(super) index: i64,
    pub(super) start_ts: i64,
    pub(super) end_ts: i64,
}

pub(super) fn build_window_fetch_slices(
    start_ts: i64,
    end_ts: i64,
    timeframe: &str,
) -> Vec<WindowFetchSlice> {
    let timeframe_ms = timeframe_to_ms(timeframe).max(1);
    let aligned_end = end_ts.saturating_sub(end_ts.rem_euclid(timeframe_ms));
    let aligned_start = start_ts.saturating_sub(start_ts.rem_euclid(timeframe_ms));
    let aligned_start = if aligned_end <= aligned_start {
        aligned_end.saturating_sub(timeframe_ms)
    } else {
        aligned_start
    };
    let slice_span = timeframe_ms
        .saturating_mul(i64::from(OKX_CANDLE_BATCH_LIMIT))
        .saturating_mul(window_fetch_batches_per_slice())
        .max(timeframe_ms);
    let mut slices = Vec::new();
    let mut current_start = aligned_start;
    let mut index = 0i64;
    while current_start < aligned_end {
        let current_end = current_start.saturating_add(slice_span).min(aligned_end);
        slices.push(WindowFetchSlice {
            index,
            start_ts: current_start,
            end_ts: current_end,
        });
        index += 1;
        if current_end <= current_start {
            break;
        }
        current_start = current_end;
    }
    slices
}

pub(super) fn estimated_window_slice_batches(slices: &[WindowFetchSlice], timeframe: &str) -> i64 {
    let timeframe_ms = timeframe_to_ms(timeframe).max(1);
    slices
        .iter()
        .map(|slice| {
            let candle_count = slice
                .end_ts
                .saturating_sub(slice.start_ts)
                .saturating_div(timeframe_ms)
                .max(1);
            estimated_batches_for_count(candle_count)
        })
        .sum::<i64>()
        .max(1)
}
