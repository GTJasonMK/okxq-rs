use serde_json::Value;

use crate::{app_state::AppState, error::AppResult};

use super::{
    super::{inventory, market, market_ops, tick_collector, LocalApiRequest},
    unsupported_route,
};

pub(super) async fn dispatch(
    state: &AppState,
    req: &LocalApiRequest,
    method: &str,
    path: &str,
    segment_refs: &[&str],
) -> AppResult<Value> {
    match (method, segment_refs) {
        ("GET", ["api", "market", "symbols"]) => market_ops::market_symbols(state).await,
        ("GET", ["api", "market", "inventory"]) => inventory::market_inventory(state, req).await,
        ("POST", ["api", "market", "inventory", "rebuild-cache"]) => {
            inventory::rebuild_inventory_cache(state, req).await
        }
        ("GET", ["api", "market", "inventory", "rebuild-cache", "status"]) => {
            inventory::inventory_cache_rebuild_status().await
        }
        ("GET", ["api", "market", "data-health"]) => {
            inventory::market_data_health(state, req).await
        }
        ("DELETE", ["api", "market", "inventory", "symbols", symbol]) => {
            inventory::delete_inventory_symbol(state, symbol, req).await
        }
        ("DELETE", ["api", "market", "inventory", "orphans"]) => {
            inventory::delete_orphan_inventory(state).await
        }
        ("GET", ["api", "market", "sync", "config"]) => {
            market_ops::sync_runtime_config(state).await
        }
        ("PUT", ["api", "market", "sync", "config"]) => {
            market_ops::update_sync_runtime_config(state, req).await
        }
        ("GET", ["api", "market", "sync", "records"]) => {
            inventory::market_sync_records(state, req).await
        }
        ("GET", ["api", "market", "sync", "jobs"]) => market_ops::sync_jobs(state, req).await,
        ("GET", ["api", "market", "sync", "jobs", task_id]) => {
            market_ops::sync_job_detail(state, task_id).await
        }
        ("POST", ["api", "market", "gaps", "plan"]) => {
            market_ops::market_gap_plan(state, req).await
        }
        ("POST", ["api", "market", "gaps", "repair", "jobs"]) => {
            market_ops::start_gap_repair_job(state, req).await
        }
        ("DELETE", ["api", "market", "sync", "jobs", task_id])
        | ("POST", ["api", "market", "sync", "jobs", task_id, "cancel"]) => {
            market_ops::cancel_sync_job(state, task_id).await
        }
        ("POST", ["api", "market", "sync"]) => market_ops::sync_candles_job(state, req).await,
        ("POST", ["api", "market", "sync", "jobs"]) => market_ops::start_sync_job(state, req).await,
        ("GET", ["api", "market", "data-guardian", "status"]) => {
            market_ops::guardian_status(state).await
        }
        ("GET", ["api", "market", "data-guardian", "config"]) => {
            market_ops::guardian_config(state).await
        }
        ("PUT", ["api", "market", "data-guardian", "config"]) => {
            market_ops::update_guardian_config(state, req).await
        }
        ("POST", ["api", "market", "data-guardian", "run-now"]) => {
            market_ops::run_guardian_now(state).await
        }
        ("GET", ["api", "market", "tick-collector", "status"]) => {
            tick_collector::tick_collector_status(state).await
        }
        ("POST", ["api", "market", "tick-collector", "start"]) => {
            tick_collector::start_tick_collector(state).await
        }
        ("POST", ["api", "market", "tick-collector", "stop"]) => {
            tick_collector::stop_tick_collector(state).await
        }
        ("GET", ["api", "market", "watched-symbols"]) => market_ops::watched_symbols(state).await,
        ("POST", ["api", "market", "watched-symbols"]) => {
            market_ops::add_watch_symbol(state, req).await
        }
        ("DELETE", ["api", "market", "watched-symbols", symbol]) => {
            market_ops::delete_watched_symbol(state, symbol).await
        }
        ("POST", ["api", "market", "watched-symbols", symbol, "repair"]) => {
            market_ops::repair_watched_symbol(state, symbol, req).await
        }
        ("GET", ["api", "market", "candles", inst_id]) => {
            market::market_candles(state, inst_id, req).await
        }
        ("GET", ["api", "market", "ticker", inst_id]) => {
            market::market_ticker(state, inst_id, req).await
        }
        ("GET", ["api", "market", "tickers"]) => market::market_tickers(state, req).await,
        ("GET", ["api", "market", "realtime", "status"]) => {
            market_ops::realtime_status(state).await
        }
        ("POST", ["api", "market", "realtime", "ticker", "subscribe"]) => {
            market_ops::subscribe_realtime_ticker(state, req).await
        }
        ("POST", ["api", "market", "realtime", "ticker", "unsubscribe"]) => {
            market_ops::unsubscribe_realtime_ticker(state, req).await
        }
        ("POST", ["api", "market", "realtime", "candle", "subscribe"]) => {
            market_ops::subscribe_realtime_candle(state, req).await
        }
        ("POST", ["api", "market", "realtime", "candle", "unsubscribe"]) => {
            market_ops::unsubscribe_realtime_candle(state, req).await
        }
        ("POST", ["api", "market", "realtime", "trades", "subscribe"]) => {
            market_ops::subscribe_realtime_trades(state, req).await
        }
        ("POST", ["api", "market", "realtime", "trades", "unsubscribe"]) => {
            market_ops::unsubscribe_realtime_trades(state, req).await
        }
        ("POST", ["api", "market", "realtime", "orderbook", "subscribe"]) => {
            market_ops::subscribe_realtime_orderbook(state, req).await
        }
        ("POST", ["api", "market", "realtime", "orderbook", "unsubscribe"]) => {
            market_ops::unsubscribe_realtime_orderbook(state, req).await
        }
        ("POST", ["api", "market", "realtime", "account", "subscribe"]) => {
            market_ops::subscribe_realtime_account(state, req).await
        }
        ("POST", ["api", "market", "realtime", "account", "unsubscribe"]) => {
            market_ops::unsubscribe_realtime_account(state, req).await
        }
        ("POST", ["api", "market", "realtime", "orders", "subscribe"]) => {
            market_ops::subscribe_realtime_private_orders(state, req).await
        }
        ("POST", ["api", "market", "realtime", "orders", "unsubscribe"]) => {
            market_ops::unsubscribe_realtime_private_orders(state, req).await
        }
        ("POST", ["api", "market", "realtime", "algo-orders", "subscribe"]) => {
            market_ops::subscribe_realtime_private_algo_orders(state, req).await
        }
        ("POST", ["api", "market", "realtime", "algo-orders", "unsubscribe"]) => {
            market_ops::unsubscribe_realtime_private_algo_orders(state, req).await
        }
        ("POST", ["api", "market", "realtime", "fills", "subscribe"]) => {
            market_ops::subscribe_realtime_private_fills(state, req).await
        }
        ("POST", ["api", "market", "realtime", "fills", "unsubscribe"]) => {
            market_ops::unsubscribe_realtime_private_fills(state, req).await
        }
        ("POST", ["api", "market", "realtime", "positions", "subscribe"]) => {
            market_ops::subscribe_realtime_private_positions(state, req).await
        }
        ("POST", ["api", "market", "realtime", "positions", "unsubscribe"]) => {
            market_ops::unsubscribe_realtime_private_positions(state, req).await
        }
        ("GET", ["api", "market", "trades", inst_id]) => {
            market::market_recent_trades(state, inst_id, req).await
        }
        ("GET", ["api", "market", "orderbook", inst_id]) => {
            market::market_orderbook(state, inst_id, req).await
        }
        ("GET", ["api", "market", "instruments"]) => {
            market_ops::market_instruments(state, req).await
        }
        ("POST", ["api", "market", "indicators"]) => market::market_indicators(state, req).await,
        ("POST", ["api", "market", "correlation"]) => market::market_correlation(state, req).await,
        ("GET", ["api", "market", "alerts"]) => market_ops::price_alerts(state, req).await,
        ("POST", ["api", "market", "alerts", "evaluate"]) => {
            market_ops::evaluate_price_alerts(state, req).await
        }
        ("POST", ["api", "market", "alerts"]) => market_ops::create_price_alert(state, req).await,
        ("PATCH", ["api", "market", "alerts", alert_id]) => {
            market_ops::update_price_alert(state, alert_id, req).await
        }
        ("DELETE", ["api", "market", "alerts", alert_id]) => {
            market_ops::delete_price_alert(state, alert_id).await
        }
        _ => unsupported_route(method, path),
    }
}
