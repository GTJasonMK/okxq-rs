use serde_json::json;

use crate::{
    risk_controls,
    strategy_execution_contract::{
        StrategyExecutionConfig, StrategyExecutionIntent, StrategyIntentAction,
    },
    strategy_execution_semantics::{
        action_dedupe_identity as shared_action_dedupe_identity,
        action_submission_key as shared_action_submission_key, ActionDedupeIdentityInput,
        ActionSubmissionKeyInput, OrderManagementCancelIdentity, OrderManagementModifyIdentity,
        RiskOrderIdentity,
    },
    trading_semantics::{
        action_position_side, check_max_adverse_slippage, is_contract_inst_type,
        max_slippage_from_params, resolve_standalone_protective_trigger_price,
        select_single_standalone_risk_order,
        validate_price_matches_tick_size as validate_trade_price,
        validate_standalone_risk_order_symbol, StandaloneRiskOrderSelectionError,
    },
};

use super::{
    market_data::HistoricalMarketData,
    state::OrderAction,
    values::{
        entry_order_side, position_side_from_action_side, risk_kind, risk_order_type, NewOrder,
    },
    HistoricalLiveBacktest,
};

mod mutation;
mod orders;
mod scopes;
mod validation;

impl HistoricalLiveBacktest {
    pub fn submit_intents(
        &mut self,
        intents: &[StrategyExecutionIntent],
        timestamp: i64,
        market: &HistoricalMarketData,
    ) {
        for (action_index, intent) in intents.iter().enumerate() {
            if self.should_submit_intent(intent, action_index) {
                self.submit_intent(intent, timestamp, market);
            }
        }
    }

    fn should_submit_intent(
        &mut self,
        intent: &StrategyExecutionIntent,
        action_index: usize,
    ) -> bool {
        if matches!(intent.action, StrategyIntentAction::Hold) {
            return true;
        }
        let action_identity = intent_action_dedupe_identity(intent);
        let key = intent_action_submission_key(intent, action_identity.as_deref(), action_index);
        if self.submitted_action_keys.insert(key) {
            return true;
        }
        self.skipped_actions.push(duplicate_intent_skip_value(
            intent,
            action_index,
            action_identity,
        ));
        false
    }

    fn submit_intent(
        &mut self,
        intent: &StrategyExecutionIntent,
        timestamp: i64,
        market: &HistoricalMarketData,
    ) {
        if intent_requires_matching_candle_series(intent)
            && !market.has_candle_series(&intent.symbol, &intent.timeframe)
        {
            self.reject_intent(intent, timestamp, &missing_candle_series_reason(intent));
            return;
        }
        match intent.action {
            StrategyIntentAction::Hold => {}
            StrategyIntentAction::CancelOrder => self.cancel_order(intent, timestamp),
            StrategyIntentAction::ModifyOrder => self.modify_order(intent, timestamp),
            StrategyIntentAction::OpenPosition => self.submit_open(intent, timestamp, market),
            StrategyIntentAction::ClosePosition => self.submit_close(intent, timestamp),
            StrategyIntentAction::PlaceRiskOrder => self.submit_risk(intent, timestamp),
        }
    }

    fn submit_open(
        &mut self,
        intent: &StrategyExecutionIntent,
        timestamp: i64,
        market: &HistoricalMarketData,
    ) {
        if action_position_side(&intent.action_record.side).is_none() {
            self.reject_intent(
                intent,
                timestamp,
                &format!("无法从策略 side={} 推导开仓方向", intent.action_record.side),
            );
            return;
        }
        let side = entry_order_side(&intent.action_record.side);
        let pos_side = position_side_from_action_side(&intent.action_record.side);
        if pos_side == "short" && !is_contract_inst_type(&intent.inst_type) {
            self.reject_intent(intent, timestamp, "现货历史撮合暂不支持做空");
            return;
        }
        if pos_side == "short" && !self.allow_short_for_inst_type(&intent.inst_type) {
            self.reject_intent(intent, timestamp, "策略配置不允许做空");
            return;
        }
        let leverage = match self.configured_leverage_for_inst_type(&intent.inst_type) {
            Ok(value) => value,
            Err(error) => {
                self.reject_intent(intent, timestamp, &error.to_string());
                return;
            }
        };
        let quantity = match self.open_quantity(intent, timestamp, market) {
            Ok(value) => value,
            Err(error) => {
                self.reject_intent(intent, timestamp, &error.to_string());
                return;
            }
        };
        if let Err(reason) =
            self.validate_order_shape(intent, quantity, intent.action_record.price, false)
        {
            self.reject_intent(intent, timestamp, &reason);
            return;
        }
        let execution_config = self.execution_config_for_intent(intent);
        let risk_state = self.runtime_risk_state(timestamp, market);
        let (risk_passed, risk_reason) = risk_controls::check_runtime_risk_controls(
            runtime_risk_check_config(&execution_config, self.settings.initial_capital),
            &intent.action_record.side,
            intent.action_record.price,
            quantity.base,
            &risk_state,
        );
        if !risk_passed {
            self.reject_intent(intent, timestamp, &risk_reason);
            return;
        }
        let (arrival_bid, arrival_ask) = self.historical_arrival_bid_ask(intent, timestamp, market);
        let (slippage_passed, slippage_reason) = check_max_adverse_slippage(
            max_slippage_from_params(&execution_config.params),
            &side,
            intent.action_record.price,
            arrival_bid,
            arrival_ask,
        );
        if !slippage_passed {
            self.reject_intent(intent, timestamp, &slippage_reason);
            return;
        }
        let margin_available = match self.margin_available(
            &intent.inst_type,
            quantity.base * intent.action_record.price,
            timestamp,
            market,
        ) {
            Ok(value) => value,
            Err(error) => {
                self.reject_intent(intent, timestamp, &error.to_string());
                return;
            }
        };
        if !margin_available {
            self.reject_intent(intent, timestamp, "模拟账户可用保证金不足");
            return;
        }
        self.create_order(NewOrder {
            intent,
            timestamp,
            action: OrderAction::Open,
            order_type: None,
            side,
            pos_side,
            leverage,
            quantity,
            reference_price: None,
            reference_price_source: None,
            reduce_only: false,
            trigger_price: None,
            risk_kind: None,
            attached_risk_orders: intent.attached_risk_orders.clone(),
            planned_exit: intent.planned_exit.clone(),
            reason: intent.action_record.reason.clone(),
        });
    }

    fn historical_arrival_bid_ask(
        &self,
        intent: &StrategyExecutionIntent,
        timestamp: i64,
        market: &HistoricalMarketData,
    ) -> (Option<f64>, Option<f64>) {
        let Some(mid_price) = market.last_close(&intent.symbol, &intent.timeframe, timestamp)
        else {
            return (None, None);
        };
        let adverse_rate =
            (self.settings.slippage_rate + self.settings.spread_rate / 2.0).clamp(0.0, 1.0);
        (
            positive_quote_price(mid_price * (1.0 - adverse_rate)),
            positive_quote_price(mid_price * (1.0 + adverse_rate)),
        )
    }

    fn submit_close(&mut self, intent: &StrategyExecutionIntent, timestamp: i64) {
        let (side, pos_side, quantity) = match self.close_scope(intent) {
            Ok(scope) => scope,
            Err(error) => {
                self.reject_intent(intent, timestamp, &error);
                return;
            }
        };
        if let Err(reason) =
            self.validate_order_shape(intent, quantity, intent.action_record.price, true)
        {
            self.reject_intent(intent, timestamp, &reason);
            return;
        }
        self.create_order(NewOrder {
            intent,
            timestamp,
            action: OrderAction::Close,
            order_type: None,
            side,
            pos_side,
            leverage: 1.0,
            quantity,
            reference_price: None,
            reference_price_source: None,
            reduce_only: true,
            trigger_price: None,
            risk_kind: None,
            attached_risk_orders: Vec::new(),
            planned_exit: None,
            reason: intent.action_record.reason.clone(),
        });
    }

    fn submit_risk(&mut self, intent: &StrategyExecutionIntent, timestamp: i64) {
        let risk = match select_single_standalone_risk_order(&intent.attached_risk_orders) {
            Ok(risk) => risk,
            Err(StandaloneRiskOrderSelectionError::Missing) => {
                self.reject_intent(intent, timestamp, "place_risk_order 缺少保护单内容");
                return;
            }
            Err(StandaloneRiskOrderSelectionError::Multiple { .. }) => {
                self.reject_intent(
                    intent,
                    timestamp,
                    "place_risk_order 独立动作一次只支持一个保护单",
                );
                return;
            }
        };
        if validate_standalone_risk_order_symbol(&risk.symbol, &intent.symbol).is_err() {
            self.reject_intent(intent, timestamp, "保护单 symbol 与执行 symbol 不一致");
            return;
        }
        let (side, pos_side, quantity, reference_price) = match self.risk_close_scope(intent, risk)
        {
            Ok(scope) => scope,
            Err(error) => {
                self.reject_intent(intent, timestamp, &error);
                return;
            }
        };
        let risk_kind = match risk_kind(risk) {
            Ok(value) => value,
            Err(error) => {
                self.reject_intent(intent, timestamp, &error);
                return;
            }
        };
        let rules = match self.settings.instrument_rules_for(&intent.symbol) {
            Ok(value) => value,
            Err(error) => {
                self.reject_intent(intent, timestamp, &error.to_string());
                return;
            }
        };
        let trigger_price = match resolve_standalone_protective_trigger_price(
            risk_kind,
            &side,
            reference_price,
            risk.trigger_price,
            risk.stop_loss,
            risk.take_profit,
            &risk.reason,
        ) {
            Ok(value) => value,
            Err(error) => {
                self.reject_intent(intent, timestamp, &error.to_string());
                return;
            }
        };
        if let Err(error) = validate_trade_price(
            &rules.to_trade_rules(),
            trigger_price,
            "独立保护单触发价",
            "已拒绝提交独立保护单以避免静默改价",
        ) {
            self.reject_intent(intent, timestamp, &error.to_string());
            return;
        }
        self.create_order(NewOrder {
            intent,
            timestamp,
            action: OrderAction::Risk,
            order_type: Some(risk_order_type(risk)),
            side,
            pos_side,
            leverage: 1.0,
            quantity,
            reference_price,
            reference_price_source: reference_price.map(|_| "position_entry_price"),
            reduce_only: true,
            trigger_price: Some(trigger_price),
            risk_kind: Some(risk_kind),
            attached_risk_orders: Vec::new(),
            planned_exit: None,
            reason: risk.reason.clone(),
        });
    }

    fn allow_short_for_inst_type(&self, inst_type: &str) -> bool {
        risk_controls::allow_short(&self.config.params, inst_type)
    }
}

fn intent_requires_matching_candle_series(intent: &StrategyExecutionIntent) -> bool {
    matches!(
        intent.action,
        StrategyIntentAction::OpenPosition
            | StrategyIntentAction::ClosePosition
            | StrategyIntentAction::PlaceRiskOrder
    )
}

fn missing_candle_series_reason(intent: &StrategyExecutionIntent) -> String {
    format!(
        "回测缺少订单撮合K线序列 {} {}，请在策略 DATA_REQUIREMENTS 中声明该周期或改用主周期",
        intent.symbol, intent.timeframe
    )
}

fn duplicate_intent_skip_value(
    intent: &StrategyExecutionIntent,
    action_index: usize,
    action_identity: Option<String>,
) -> serde_json::Value {
    json!({
        "source": "historical_live_backtest",
        "mode": "historical_sim",
        "action": intent.action.as_str(),
        "symbol": intent.symbol,
        "inst_id": intent.symbol,
        "inst_type": intent.inst_type,
        "timeframe": intent.timeframe,
        "side": intent.action_record.side,
        "price": intent.action_record.price,
        "timestamp": intent.action_record.timestamp,
        "order_type": intent.order_type,
        "order_side": intent.order_side,
        "exchange_size": intent.exchange_size,
        "action_index": action_index,
        "action_identity": action_identity,
        "reason": intent.action_record.reason,
        "_execution_skip_reason": "重复策略动作已按 live action 去重规则跳过",
    })
}

fn positive_quote_price(value: f64) -> Option<f64> {
    (value.is_finite() && value > 0.0).then_some(value)
}

fn runtime_risk_check_config(
    config: &StrategyExecutionConfig,
    initial_capital: f64,
) -> risk_controls::RuntimeRiskCheckConfig<'_> {
    risk_controls::RuntimeRiskCheckConfig::from_strategy_params(
        &config.params,
        &config.symbol,
        &config.inst_type,
        initial_capital,
        config.stop_loss,
    )
}

fn intent_action_submission_key(
    intent: &StrategyExecutionIntent,
    action_identity: Option<&str>,
    action_index: usize,
) -> String {
    shared_action_submission_key(ActionSubmissionKeyInput {
        symbol: &intent.symbol,
        action: intent.action.as_str(),
        order_type: &intent.order_type,
        action_side: &intent.action_record.side,
        action_price: intent.action_record.price,
        action_position_size: intent.action_record.position_size,
        action_timestamp: intent.action_record.timestamp,
        order_side: intent.order_side.as_deref(),
        exchange_size: intent.exchange_size.as_deref(),
        planned_exit_timestamp: intent.planned_exit.as_ref().map(|item| item.timestamp),
        action_identity,
        action_index,
    })
}

fn intent_action_dedupe_identity(intent: &StrategyExecutionIntent) -> Option<String> {
    let cancel_order = intent
        .cancel_order
        .as_ref()
        .map(|cancel| OrderManagementCancelIdentity {
            target_kind: cancel.target_kind.as_str(),
            order_id: &cancel.order_id,
            client_order_id: &cancel.client_order_id,
        });
    let modify_order = intent
        .modify_order
        .as_ref()
        .map(|modify| OrderManagementModifyIdentity {
            target_kind: modify.target_kind.as_str(),
            target_order_type: modify.target_order_type.as_deref(),
            order_id: &modify.order_id,
            client_order_id: &modify.client_order_id,
            new_size: modify.new_size.as_deref(),
            new_price: modify.new_price.as_deref(),
            cancel_on_fail: modify.cancel_on_fail,
        });
    let attached_risk_orders = intent
        .attached_risk_orders
        .iter()
        .map(|risk| RiskOrderIdentity {
            symbol: &risk.symbol,
            side: &risk.side,
            order_type: &risk.order_type,
            trigger_price: risk.trigger_price,
            stop_loss: risk.stop_loss,
            take_profit: risk.take_profit,
        })
        .collect::<Vec<_>>();
    shared_action_dedupe_identity(ActionDedupeIdentityInput {
        action: intent.action.as_str(),
        cancel_order,
        modify_order,
        stop_loss: intent.stop_loss,
        take_profit: intent.take_profit,
        max_slippage: intent.max_slippage,
        attached_risk_orders: &attached_risk_orders,
    })
}
