use super::super::super::timeframe_to_ms;
use super::slices::build_window_fetch_slices;

#[test]
fn window_slices_do_not_round_end_into_future() {
    let start_ts = 1_779_264_260_000;
    let end_ts = 1_779_523_460_000;

    let slices = build_window_fetch_slices(start_ts, end_ts, "1m");

    assert!(!slices.is_empty());
    assert!(slices.iter().all(|slice| slice.end_ts <= end_ts));
    assert_eq!(
        slices.last().map(|slice| slice.end_ts),
        Some(1_779_523_440_000)
    );
}

#[test]
fn window_slices_keep_a_minimum_single_candle_span() {
    let end_ts = 1_779_523_420_000;
    let start_ts = end_ts - 1_000;

    let slices = build_window_fetch_slices(start_ts, end_ts, "1m");

    assert_eq!(slices.len(), 1);
    assert!(slices[0].end_ts <= end_ts);
    assert_eq!(
        slices[0].end_ts.saturating_sub(slices[0].start_ts),
        timeframe_to_ms("1m")
    );
}
