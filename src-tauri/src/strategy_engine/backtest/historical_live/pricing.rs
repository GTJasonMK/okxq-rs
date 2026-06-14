use crate::okx::OkxCandle;

use super::{
    state::{OrderAction, RiskKind, SimOrder},
    values::normalized_position_side,
    HistoricalLiveBacktest,
};

impl HistoricalLiveBacktest {
    pub(super) fn execution_price(&self, order: &SimOrder, candle: &OkxCandle) -> Option<f64> {
        match order.action {
            OrderAction::Risk => self.risk_execution_price(order, candle),
            _ => match order.order_type.as_str() {
                "market" | "optimal_limit_ioc" => self.market_fill_price(&order.side, candle.open),
                "ioc" | "fok" => self.immediate_limit_execution_price(order, candle),
                "limit" | "mmp" => self.limit_execution_price(order, candle),
                "post_only" | "mmp_and_post_only" => {
                    resting_limit_fill_price(&order.side, order.price?, candle)
                }
                _ => None,
            },
        }
    }

    fn risk_execution_price(&self, order: &SimOrder, candle: &OkxCandle) -> Option<f64> {
        let trigger = order.trigger_price?;
        let kind = order.risk_kind?;
        if matches!(kind, RiskKind::TakeProfit)
            && self.triggered_sibling_stop_loss_exists(order, candle)
        {
            return None;
        }
        let long_position = match normalized_position_side(&order.pos_side)? {
            "long" => true,
            "short" => false,
            _ => return None,
        };
        let triggered = match (kind, long_position) {
            (RiskKind::StopLoss, true) => candle.low <= trigger,
            (RiskKind::StopLoss, false) => candle.high >= trigger,
            (RiskKind::TakeProfit, true) => candle.high >= trigger,
            (RiskKind::TakeProfit, false) => candle.low <= trigger,
        };
        if !triggered {
            return None;
        }
        let base = match (kind, long_position) {
            (RiskKind::StopLoss, true) if candle.open < trigger => candle.open,
            (RiskKind::StopLoss, false) if candle.open > trigger => candle.open,
            _ => trigger,
        };
        self.market_fill_price(&order.side, base)
    }

    fn triggered_sibling_stop_loss_exists(&self, order: &SimOrder, candle: &OkxCandle) -> bool {
        self.orders.iter().any(|candidate| {
            candidate.order_id != order.order_id
                && candidate.is_open()
                && matches!(candidate.action, OrderAction::Risk)
                && candidate.reduce_only
                && candidate.symbol.eq_ignore_ascii_case(&order.symbol)
                && candidate.pos_side.eq_ignore_ascii_case(&order.pos_side)
                && candidate.risk_kind == Some(RiskKind::StopLoss)
                && candidate
                    .trigger_price
                    .is_some_and(|trigger| stop_loss_triggered(candidate, candle, trigger))
        })
    }

    fn market_fill_price(&self, side: &str, reference: f64) -> Option<f64> {
        if !reference.is_finite() || reference <= 0.0 {
            return None;
        }
        let direction = match side {
            "buy" => 1.0,
            "sell" => -1.0,
            _ => return None,
        };
        let price = reference
            * (1.0 + direction * (self.settings.slippage_rate + self.settings.spread_rate / 2.0));
        (price.is_finite() && price > 0.0).then_some(price)
    }

    fn immediate_limit_execution_price(&self, order: &SimOrder, candle: &OkxCandle) -> Option<f64> {
        marketable_limit_open_price(order, candle.open)
            .and_then(|limit| self.limit_capped_market_fill_price(&order.side, candle.open, limit))
    }

    fn limit_execution_price(&self, order: &SimOrder, candle: &OkxCandle) -> Option<f64> {
        if order.last_processed_ts == order.submitted_ts {
            if let Some(limit) = marketable_limit_open_price(order, candle.open) {
                return self.limit_capped_market_fill_price(&order.side, candle.open, limit);
            }
        }
        resting_limit_fill_price(&order.side, order.price?, candle)
    }

    fn limit_capped_market_fill_price(
        &self,
        side: &str,
        reference: f64,
        limit: f64,
    ) -> Option<f64> {
        let market_price = self.market_fill_price(side, reference)?;
        let price = match side {
            "buy" => market_price.min(limit),
            "sell" => market_price.max(limit),
            _ => return None,
        };
        (price.is_finite() && price > 0.0).then_some(price)
    }
}

fn stop_loss_triggered(order: &SimOrder, candle: &OkxCandle, trigger: f64) -> bool {
    match normalized_position_side(&order.pos_side) {
        Some("long") => candle.low <= trigger,
        Some("short") => candle.high >= trigger,
        _ => false,
    }
}

fn marketable_limit_open_price(order: &SimOrder, open: f64) -> Option<f64> {
    let limit = order.price?;
    match order.side.as_str() {
        "buy" if limit >= open => Some(limit),
        "sell" if limit <= open => Some(limit),
        _ => None,
    }
}

fn resting_limit_fill_price(side: &str, limit_price: f64, candle: &OkxCandle) -> Option<f64> {
    match side {
        "buy" if candle.low <= limit_price => Some(limit_price),
        "sell" if candle.high >= limit_price => Some(limit_price),
        _ => None,
    }
}
