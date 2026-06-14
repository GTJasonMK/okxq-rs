use crate::strategy_execution_contract::{StrategyPlannedExitIntent, StrategyRiskOrderIntent};

use crate::trading_semantics::remaining_quantity;
pub(super) use crate::trading_semantics::ProtectiveOrderKind as RiskKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OrderAction {
    Open,
    Close,
    Risk,
}

#[derive(Clone, Debug)]
pub(super) struct SimOrder {
    pub(super) order_id: String,
    pub(super) client_order_id: String,
    pub(super) symbol: String,
    pub(super) inst_type: String,
    pub(super) timeframe: String,
    pub(super) action: OrderAction,
    pub(super) order_type: String,
    pub(super) side: String,
    pub(super) pos_side: String,
    pub(super) leverage: f64,
    pub(super) exchange_quantity: f64,
    pub(super) exchange_filled: f64,
    pub(super) quantity: f64,
    pub(super) filled: f64,
    pub(super) fill_summary: SimOrderFillSummary,
    pub(super) price: Option<f64>,
    pub(super) reference_price: f64,
    pub(super) reference_price_source: String,
    pub(super) reference_price_missing: bool,
    pub(super) trigger_price: Option<f64>,
    pub(super) risk_kind: Option<RiskKind>,
    pub(super) status: String,
    pub(super) reason: String,
    pub(super) submitted_ts: i64,
    pub(super) last_processed_ts: i64,
    pub(super) action_ts: i64,
    pub(super) reduce_only: bool,
    pub(super) entry_order_id: Option<String>,
    pub(super) attached_risk_identity: Option<String>,
    pub(super) attached_risk_orders: Vec<StrategyRiskOrderIntent>,
    pub(super) planned_exit: Option<StrategyPlannedExitIntent>,
    pub(super) error_message: String,
}

#[derive(Clone, Debug, Default)]
pub(super) struct SimOrderFillSummary {
    pub(super) count: u64,
    pub(super) exchange_quantity: f64,
    pub(super) notional: f64,
    pub(super) price_quantity: f64,
    pub(super) total_fee: f64,
    pub(super) first_ts: Option<i64>,
    pub(super) last_ts: Option<i64>,
}

impl SimOrderFillSummary {
    pub(super) fn record(
        &mut self,
        timestamp: i64,
        price: f64,
        exchange_quantity: f64,
        notional: f64,
        fee: f64,
    ) {
        self.count += 1;
        self.exchange_quantity += exchange_quantity;
        self.notional += notional;
        self.price_quantity += price * exchange_quantity;
        self.total_fee += fee;
        self.first_ts = Some(
            self.first_ts
                .map_or(timestamp, |value| value.min(timestamp)),
        );
        self.last_ts = Some(self.last_ts.map_or(timestamp, |value| value.max(timestamp)));
    }

    pub(super) fn avg_fill_price(&self) -> Option<f64> {
        (self.exchange_quantity > 0.0 && self.price_quantity > 0.0)
            .then_some(self.price_quantity / self.exchange_quantity)
    }

    pub(super) fn fill_notional(&self) -> Option<f64> {
        (self.count > 0).then_some(self.notional)
    }

    pub(super) fn total_fee(&self) -> Option<f64> {
        (self.count > 0).then_some(self.total_fee)
    }
}

impl SimOrder {
    pub(super) fn remaining(&self) -> f64 {
        remaining_quantity(self.quantity, self.filled)
    }

    pub(super) fn exchange_remaining(&self) -> f64 {
        remaining_quantity(self.exchange_quantity, self.exchange_filled)
    }

    pub(super) fn is_open(&self) -> bool {
        matches!(self.status.as_str(), "open" | "partially_filled")
    }
}

#[derive(Clone, Debug)]
pub(super) struct SimPosition {
    pub(super) symbol: String,
    pub(super) inst_type: String,
    pub(super) timeframe: String,
    pub(super) side: String,
    pub(super) exchange_quantity: f64,
    pub(super) quantity: f64,
    pub(super) entry_price: f64,
    pub(super) leverage: f64,
    pub(super) realized_pnl: f64,
    pub(super) opened_ts: i64,
    pub(super) last_funding_ts: i64,
    pub(super) accumulated_funding: f64,
    pub(super) entry_order_id: String,
}

impl SimPosition {
    pub(super) fn side_dir(&self) -> i32 {
        match self.side.as_str() {
            "short" => -1,
            "long" => 1,
            _ => 0,
        }
    }

    pub(super) fn unrealized_pnl(&self, mark_price: f64) -> f64 {
        (mark_price - self.entry_price) * self.quantity * self.side_dir() as f64
    }
}

#[derive(Clone, Debug)]
pub(super) struct PlannedExit {
    pub(super) symbol: String,
    pub(super) inst_type: String,
    pub(super) timeframe: String,
    pub(super) side: String,
    pub(super) exchange_quantity: f64,
    pub(super) quantity: f64,
    pub(super) due_ts: i64,
    pub(super) entry_order_id: Option<String>,
    pub(super) reason: String,
    pub(super) contract: String,
    pub(super) submitted: bool,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SimOrderQuantity {
    pub(super) exchange: f64,
    pub(super) base: f64,
}
