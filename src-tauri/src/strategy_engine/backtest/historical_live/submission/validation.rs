use crate::{
    strategy_execution_contract::StrategyExecutionIntent,
    trading_semantics::{
        normalized_exchange_order_type, resolve_attached_protective_trigger_price,
        validate_attached_protective_order_scope,
        validate_exchange_order_price_shape as validate_trade_order_price_shape,
        validate_explicit_exchange_size as validate_trade_size,
        validate_price_matches_tick_size as validate_trade_price,
    },
};

use super::super::{
    state::SimOrderQuantity,
    values::{position_side_from_action_side, risk_kind},
    HistoricalLiveBacktest,
};

impl HistoricalLiveBacktest {
    pub(super) fn validate_order_shape(
        &self,
        intent: &StrategyExecutionIntent,
        quantity: SimOrderQuantity,
        price: f64,
        reduce_only: bool,
    ) -> Result<(), String> {
        let normalized = normalized_exchange_order_type(&intent.order_type)
            .map_err(|error| error.to_string())?;
        let rules = self
            .settings
            .instrument_rules_for(&intent.symbol)
            .map_err(|error| error.to_string())?;
        let trade_rules = rules.to_trade_rules();
        validate_trade_order_price_shape(
            &trade_rules,
            normalized,
            &intent.action_record.side,
            price,
            |order_type| format!("OKX {order_type} 订单需要有效价格"),
            "已拒绝回测下单以避免静默改价",
        )
        .map_err(|error| error.to_string())?;
        if trade_rules.lot_sz.is_some() {
            validate_trade_size(
                &trade_rules,
                quantity.exchange,
                "模拟订单数量",
                "已拒绝回测下单以避免静默改量",
            )
            .map_err(|error| error.to_string())?;
        }
        if let Some(min_notional) = self.settings.min_notional {
            let notional = quantity.base * price;
            if !reduce_only && notional + f64::EPSILON < min_notional {
                return Err(format!(
                    "模拟订单金额 {:.8} 小于 min notional {:.8}",
                    notional, min_notional
                ));
            }
        }
        if !reduce_only && !intent.attached_risk_orders.is_empty() {
            self.validate_attached_risk_orders(intent, price)?;
        }
        Ok(())
    }

    fn validate_attached_risk_orders(
        &self,
        intent: &StrategyExecutionIntent,
        entry_price: f64,
    ) -> Result<(), String> {
        let rules = self
            .settings
            .instrument_rules_for(&intent.symbol)
            .map_err(|error| error.to_string())?;
        let pos_side = position_side_from_action_side(&intent.action_record.side);
        for risk in &intent.attached_risk_orders {
            validate_attached_protective_order_scope(
                &risk.symbol,
                &risk.side,
                &intent.symbol,
                &intent.action_record.side,
            )
            .map_err(|error| error.to_string())?;
            let kind = risk_kind(risk)?;
            let trigger_price = resolve_attached_protective_trigger_price(
                kind,
                &pos_side,
                entry_price,
                risk.trigger_price,
                risk.stop_loss,
                risk.take_profit,
                &risk.reason,
            )
            .map_err(|error| error.to_string())?;
            validate_trade_price(
                &rules.to_trade_rules(),
                trigger_price,
                "保护单触发价",
                "已拒绝裸开仓以避免保护单静默改价",
            )
            .map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}
