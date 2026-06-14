use crate::{
    error::{AppError, AppResult},
    okx::OkxCandle,
    risk_controls,
    strategy_execution_contract::StrategyExecutionIntent,
    trading_semantics::{
        apply_instrument_size_rules as apply_trade_size_rules,
        base_quantity_from_exchange_quantity as trade_base_quantity_from_exchange_quantity,
        close_quantity_limit_decision,
        exchange_quantity_from_base_quantity as trade_exchange_quantity_from_base_quantity,
        exchange_quantity_value as trade_exchange_quantity_value, is_contract_inst_type,
        parse_positive_decimal_text, resolve_exchange_quantity_from_base_order_quantity,
        resolve_explicit_exchange_quantity as trade_resolve_explicit_exchange_quantity,
        validate_explicit_exchange_size as validate_trade_size, validate_modify_order_price_field,
        validate_modify_order_size_field, CloseQuantityLimitDecision, ModifyOrderFieldSemantics,
        ALGO_MODIFY_ORDER_FIELDS, EXCHANGE_MODIFY_ORDER_FIELDS,
    },
};

use super::{
    market_data::HistoricalMarketData,
    settings::SimInstrumentRules,
    state::{OrderAction, SimOrder, SimOrderQuantity, SimPosition},
    values::{norm_pos_side, norm_symbol},
    HistoricalLiveBacktest,
};

impl HistoricalLiveBacktest {
    pub(super) fn open_quantity(
        &self,
        intent: &StrategyExecutionIntent,
        timestamp: i64,
        market: &HistoricalMarketData,
    ) -> AppResult<SimOrderQuantity> {
        let price = intent.action_record.price;
        if !price.is_finite() || price <= 0.0 {
            return Err(AppError::Validation("开仓价格无效".to_string()));
        }
        if let Some(exchange_size) = intent
            .exchange_size
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return self.explicit_exchange_quantity(
                &intent.inst_type,
                &intent.symbol,
                exchange_size,
                price,
                "策略显式 exchange_size",
                "已拒绝回测下单以避免静默改量",
            );
        }
        let base_quantity = self.requested_base_quantity(intent, timestamp, market)?;
        let rules = self.settings.instrument_rules_for(&intent.symbol)?;
        let quantity = resolve_exchange_quantity_from_base_order_quantity(
            &rules.to_trade_rules(),
            &intent.inst_type,
            &intent.symbol,
            base_quantity,
            price,
            "模拟基础币数量必须是有效正数",
            "模拟价格无效，无法把基础币数量换算为 OKX 合约张数",
            "模拟 OKX 下单数量必须是有效正数",
        )?;
        Ok(SimOrderQuantity {
            exchange: quantity.exchange_quantity,
            base: quantity.base_quantity,
        })
    }

    fn requested_base_quantity(
        &self,
        intent: &StrategyExecutionIntent,
        _timestamp: i64,
        _market: &HistoricalMarketData,
    ) -> AppResult<f64> {
        let price = intent.action_record.price;
        if !price.is_finite() || price <= 0.0 {
            return Err(AppError::Validation("开仓价格无效".to_string()));
        }
        let quantity = risk_controls::runtime_order_base_quantity(
            &self.config.params,
            &intent.inst_type,
            self.settings.initial_capital,
            self.config.position_size,
            intent.action_record.position_size,
            price,
        )
        .ok_or_else(|| AppError::Validation("模拟下单数量必须是有效正数".to_string()))?;
        Ok(quantity)
    }

    pub(super) fn explicit_exchange_quantity(
        &self,
        inst_type: &str,
        symbol: &str,
        raw_exchange_size: &str,
        price: f64,
        label: &str,
        consequence: &str,
    ) -> AppResult<SimOrderQuantity> {
        let rules = self.settings.instrument_rules_for(symbol)?;
        let quantity = trade_resolve_explicit_exchange_quantity(
            &rules.to_trade_rules(),
            inst_type,
            symbol,
            raw_exchange_size,
            price,
            label,
            consequence,
            "模拟 OKX 下单数量必须是有效正数",
            "模拟价格无效，无法估算 OKX 合约张数的基础币敞口",
        )?;
        Ok(SimOrderQuantity {
            exchange: quantity.exchange_quantity,
            base: quantity.base_quantity,
        })
    }

    pub(super) fn modify_order_quantity(
        &self,
        order: &SimOrder,
        raw_new_size: &str,
        price_override: Option<f64>,
    ) -> AppResult<SimOrderQuantity> {
        let rules = self.settings.instrument_rules_for(&order.symbol)?;
        let exchange_quantity = validate_modify_order_size_field(
            &rules.to_trade_rules(),
            raw_new_size,
            modify_order_field_semantics(order),
        )?;
        let base_quantity = if matches!(order.action, OrderAction::Close | OrderAction::Risk)
            || order.reduce_only
        {
            self.reduce_only_modify_base_quantity(order, exchange_quantity)?
        } else {
            let price = price_override
                .or(order.price)
                .or(order.trigger_price)
                .unwrap_or(order.reference_price);
            self.base_quantity_from_exchange_quantity(
                &order.inst_type,
                &order.symbol,
                exchange_quantity,
                price,
            )?
        };
        Ok(SimOrderQuantity {
            exchange: exchange_quantity,
            base: base_quantity,
        })
    }

    pub(super) fn modify_order_price(
        &self,
        order: &SimOrder,
        raw_new_price: &str,
    ) -> AppResult<f64> {
        let rules = self.settings.instrument_rules_for(&order.symbol)?;
        validate_modify_order_price_field(
            &rules.to_trade_rules(),
            raw_new_price,
            modify_order_field_semantics(order),
        )
    }

    pub(super) fn apply_exchange_size_rules(
        &self,
        symbol: &str,
        exchange_quantity: f64,
    ) -> AppResult<f64> {
        let rules = self.settings.instrument_rules_for(symbol)?;
        self.apply_exchange_size_rules_with_rules(&rules, exchange_quantity)
    }

    pub(super) fn apply_exchange_size_rules_with_rules(
        &self,
        rules: &SimInstrumentRules,
        exchange_quantity: f64,
    ) -> AppResult<f64> {
        apply_trade_size_rules(&rules.to_trade_rules(), exchange_quantity)
    }

    fn validate_explicit_exchange_size(
        &self,
        symbol: &str,
        exchange_quantity: f64,
        label: &str,
        consequence: &str,
    ) -> AppResult<()> {
        let rules = self.settings.instrument_rules_for(symbol)?;
        validate_trade_size(
            &rules.to_trade_rules(),
            exchange_quantity,
            label,
            consequence,
        )
    }

    pub(super) fn exchange_quantity_from_base_quantity(
        &self,
        inst_type: &str,
        symbol: &str,
        base_quantity: f64,
        price: f64,
    ) -> AppResult<f64> {
        let rules = self.settings.instrument_rules_for(symbol)?;
        trade_exchange_quantity_from_base_quantity(
            inst_type,
            symbol,
            &rules.to_trade_rules(),
            base_quantity,
            price,
            "模拟基础币数量必须是有效正数",
            "模拟价格无效，无法把基础币数量换算为 OKX 合约张数",
        )
    }

    pub(super) fn base_quantity_from_exchange_quantity(
        &self,
        inst_type: &str,
        symbol: &str,
        exchange_quantity: f64,
        price: f64,
    ) -> AppResult<f64> {
        let rules = self.settings.instrument_rules_for(symbol)?;
        trade_base_quantity_from_exchange_quantity(
            inst_type,
            symbol,
            &rules.to_trade_rules(),
            exchange_quantity,
            price,
            "模拟 OKX 下单数量必须是有效正数",
            "模拟价格无效，无法估算 OKX 合约张数的基础币敞口",
        )
    }

    pub(super) fn exchange_quantity_value(
        &self,
        inst_type: &str,
        symbol: &str,
        exchange_quantity: f64,
        price: f64,
    ) -> AppResult<f64> {
        let rules = self.settings.instrument_rules_for(symbol)?;
        trade_exchange_quantity_value(
            inst_type,
            symbol,
            &rules.to_trade_rules(),
            exchange_quantity,
            price,
        )
    }

    pub(super) fn candle_base_capacity(&self, inst_type: &str, candle: &OkxCandle) -> f64 {
        if is_contract_inst_type(inst_type) {
            candle.volume_ccy
        } else {
            candle.volume
        }
    }

    pub(super) fn close_quantity_from_position(
        &self,
        intent: &StrategyExecutionIntent,
        position: &SimPosition,
    ) -> Result<SimOrderQuantity, String> {
        if let Some(exchange_size) = intent
            .exchange_size
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let exchange_quantity = parse_positive_decimal_text(exchange_size, "exchange_size")
                .map_err(|error| error.to_string())?;
            self.validate_explicit_exchange_size(
                &position.symbol,
                exchange_quantity,
                "策略显式平仓 exchange_size",
                "已拒绝回测平仓以避免静默改量",
            )
            .map_err(|error| error.to_string())?;
            match close_quantity_limit_decision(position.exchange_quantity, exchange_quantity, true)
                .map_err(|error| error.to_string())?
            {
                CloseQuantityLimitDecision::RejectAboveAvailable => {
                    return Err(format!(
                        "策略显式平仓 exchange_size={:.8} 大于模拟可平数量 {:.8}，已拒绝平仓以避免静默改量",
                        exchange_quantity, position.exchange_quantity
                    ));
                }
                CloseQuantityLimitDecision::CapTo(_) | CloseQuantityLimitDecision::UseAvailable => {
                }
            }
            let base = self
                .close_base_quantity_from_position_exchange(position, exchange_quantity)
                .map_err(|error| error.to_string())?;
            return Ok(SimOrderQuantity {
                exchange: exchange_quantity,
                base,
            });
        }
        if intent.action_record.position_size.is_some() {
            let price = intent.action_record.price;
            if !price.is_finite() || price <= 0.0 {
                return Err("平仓价格无效，无法按 position_size 换算 OKX 下单数量".to_string());
            }
            let requested_base = risk_controls::runtime_order_base_quantity(
                &self.config.params,
                &intent.inst_type,
                self.settings.initial_capital,
                self.config.position_size,
                intent.action_record.position_size,
                price,
            )
            .ok_or_else(|| "平仓价格无效，无法按 position_size 换算 OKX 下单数量".to_string())?;
            let requested_exchange = self
                .exchange_quantity_from_base_quantity(
                    &position.inst_type,
                    &position.symbol,
                    requested_base,
                    price,
                )
                .and_then(|value| self.apply_exchange_size_rules(&position.symbol, value))
                .map_err(|error| error.to_string())?;
            if let CloseQuantityLimitDecision::CapTo(requested_exchange) =
                close_quantity_limit_decision(position.exchange_quantity, requested_exchange, false)
                    .map_err(|error| error.to_string())?
            {
                let base = self
                    .close_base_quantity_from_position_exchange(position, requested_exchange)
                    .map_err(|error| error.to_string())?;
                return Ok(SimOrderQuantity {
                    exchange: requested_exchange,
                    base,
                });
            }
        }
        Ok(SimOrderQuantity {
            exchange: position.exchange_quantity,
            base: position.quantity,
        })
    }

    pub(super) fn close_base_quantity_from_position_exchange(
        &self,
        position: &SimPosition,
        exchange_quantity: f64,
    ) -> AppResult<f64> {
        if !exchange_quantity.is_finite() || exchange_quantity <= 0.0 {
            return Err(AppError::Validation(
                "模拟 OKX 平仓数量必须是有效正数".to_string(),
            ));
        }
        if !position.exchange_quantity.is_finite()
            || position.exchange_quantity <= 0.0
            || !position.quantity.is_finite()
            || position.quantity <= 0.0
        {
            return Err(AppError::Validation(
                "模拟持仓数量无效，无法计算平仓成交数量".to_string(),
            ));
        }
        let close_exchange = exchange_quantity.min(position.exchange_quantity);
        Ok(
            (position.quantity * close_exchange / position.exchange_quantity)
                .min(position.quantity),
        )
    }

    fn reduce_only_modify_base_quantity(
        &self,
        order: &SimOrder,
        exchange_quantity: f64,
    ) -> AppResult<f64> {
        let target_exchange = exchange_quantity.max(order.exchange_filled);
        let remaining_exchange = (target_exchange - order.exchange_filled).max(0.0);
        if remaining_exchange <= 1e-12 {
            return Ok(order.filled.max(0.0));
        }
        let key = (norm_symbol(&order.symbol), norm_pos_side(&order.pos_side));
        let Some(position) = self.positions.get(&key) else {
            return Err(AppError::Validation(
                "没有可平持仓，无法计算 reduce-only 改单后的基础币数量".to_string(),
            ));
        };
        let remaining_base =
            self.close_base_quantity_from_position_exchange(position, remaining_exchange)?;
        Ok(order.filled + remaining_base)
    }
}

fn modify_order_field_semantics(order: &SimOrder) -> ModifyOrderFieldSemantics {
    if matches!(order.action, OrderAction::Risk) {
        ALGO_MODIFY_ORDER_FIELDS
    } else {
        EXCHANGE_MODIFY_ORDER_FIELDS
    }
}
