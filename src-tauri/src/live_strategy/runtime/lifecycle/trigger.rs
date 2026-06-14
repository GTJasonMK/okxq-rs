use std::time::Duration;

use tokio::{sync::broadcast, time::Instant};

use crate::realtime::RealtimeCandleEvent;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::live_strategy::runtime::lifecycle) enum LiveLoopTrigger {
    ConfirmedCandle(RealtimeCandleEvent),
    RestWatchdog,
}

pub(in crate::live_strategy::runtime::lifecycle) async fn wait_for_strategy_candle_or_watchdog(
    events: &mut broadcast::Receiver<RealtimeCandleEvent>,
    subscriptions: &[(String, String)],
    watchdog: Duration,
) -> LiveLoopTrigger {
    let deadline = Instant::now() + watchdog;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return LiveLoopTrigger::RestWatchdog;
        }
        match tokio::time::timeout(remaining, events.recv()).await {
            Ok(Ok(event)) if event_matches_subscriptions(&event, subscriptions) => {
                return LiveLoopTrigger::ConfirmedCandle(event);
            }
            Ok(Ok(_)) => continue,
            Ok(Err(broadcast::error::RecvError::Lagged(_))) => {
                return LiveLoopTrigger::RestWatchdog;
            }
            Ok(Err(broadcast::error::RecvError::Closed)) | Err(_) => {
                return LiveLoopTrigger::RestWatchdog;
            }
        }
    }
}

pub(in crate::live_strategy::runtime::lifecycle) fn event_matches_subscriptions(
    event: &RealtimeCandleEvent,
    subscriptions: &[(String, String)],
) -> bool {
    event.timestamp > 0
        && subscriptions.iter().any(|(symbol, timeframe)| {
            event.inst_id.eq_ignore_ascii_case(symbol)
                && event.timeframe.eq_ignore_ascii_case(timeframe)
        })
}
