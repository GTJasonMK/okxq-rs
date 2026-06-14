use serde_json::Value;

use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
};

use super::{
    super::{trading, LocalApiRequest},
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
        ("GET", ["api", "risk", "snapshots"]) => trading::risk_snapshots(state, req).await,
        ("GET", ["api", "risk", "overview"]) => trading::risk_overview(state, req).await,
        ("GET", ["api", "risk", "metrics"]) => trading::risk_metrics(state, req).await,
        ("GET", ["api", "risk", "var"]) => trading::risk_var(state, req).await,
        ("GET", ["api", "risk", "drawdown"]) => trading::risk_drawdown(state, req).await,
        ("GET", ["api", "risk", "rolling"]) => trading::risk_rolling(state, req).await,

        ("GET", ["api", "trading", "status"]) => trading::trading_status(state, req).await,
        ("GET", ["api", "trading", "account"]) => trading::trading_account(state, req).await,
        ("GET", ["api", "trading", "positions"]) => trading::trading_positions(state, req).await,
        ("GET", ["api", "trading", "spot-holdings"]) => {
            trading::trading_spot_holdings(state, req).await
        }
        ("GET", ["api", "trading", "holdings-base"]) => {
            trading::trading_holdings_base(state, req).await
        }
        ("GET", ["api", "trading", "orders"]) => trading::trading_orders(state, req).await,
        ("GET", ["api", "trading", "orders", "history"]) => {
            trading::trading_order_history(state, req).await
        }
        ("GET", ["api", "trading", "fills"]) => trading::trading_fills(state, req).await,
        ("GET", ["api", "trading", "max-size", inst_id]) => {
            trading::trading_max_size(state, inst_id, req).await
        }
        ("GET", ["api", "trading", "contract", "positions"]) => {
            trading::trading_contract_positions(state, req).await
        }
        ("GET", ["api", "trading", "contract", "leverage", inst_id]) => {
            trading::trading_contract_leverage(state, inst_id, req).await
        }
        ("GET", ["api", "trading", "contract", "max-size", inst_id]) => {
            trading::trading_contract_max_size(state, inst_id, req).await
        }
        ("GET", ["api", "trading", "contract", "account-config"]) => {
            trading::trading_contract_account_config(state, req).await
        }
        ("GET", ["api", "trading", "fee-rates"]) => trading::trading_fee_rates(state, req).await,
        ("GET", ["api", "trading", "local-fee-rates"]) => {
            trading::local_trading_fee_rates(state, req).await
        }
        ("GET", ["api", "trading", "local-fills"]) => {
            trading::history::local_fills(state, req).await
        }
        ("GET", ["api", "trading", "cost-basis"]) => trading::history::cost_basis(state, req).await,
        ("GET", ["api", "trading", "performance"]) => {
            trading::history::trade_performance(state, req).await
        }
        ("GET", ["api", "trading", "performance", "export"]) => {
            trading::history::trade_performance_export(state, req).await
        }
        ("GET", ["api", "trading", "risk-summary"]) => trading::risk_summary(state, req).await,
        ("GET", ["api", "trading", "risk-control"]) => {
            trading::risk_control_config(state, req).await
        }
        ("PUT", ["api", "trading", "risk-control"]) => {
            trading::update_risk_control_config(state, req).await
        }
        ("GET", ["api", "trading", "order"]) => trading::trading_get_order(state, req).await,
        ("GET", ["api", "trading", "fills-history"]) => {
            trading::trading_fills_history(state, req).await
        }
        ("GET", ["api", "trading", "max-avail-size", inst_id]) => {
            trading::trading_max_avail_size(state, inst_id, req).await
        }
        ("POST", ["api", "trading", "local-fills", "sync"]) => {
            trading::history::sync_local_fills(state, req).await
        }
        ("POST", ["api", "trading", "fee-rates", "sync"]) => {
            trading::sync_trading_fee_rates(state, req).await
        }
        ("POST", ["api", "trading", "order"]) => trading::trading_place_order(state, req).await,
        ("POST", ["api", "trading", "cancel"]) => trading::trading_cancel_order(state, req).await,
        ("POST", ["api", "trading", "contract", "set-leverage"]) => {
            trading::trading_set_leverage(state, req).await
        }
        ("POST", ["api", "trading", "contract", "set-position-mode"]) => {
            trading::trading_set_position_mode(state, req).await
        }
        ("POST", ["api", "trading", ..]) | ("DELETE", ["api", "trading", ..]) => {
            Err(AppError::Validation("不支持的交易操作".to_string()))
        }
        _ => unsupported_route(method, path),
    }
}
