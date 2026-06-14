use serde_json::Value;

use crate::{
    strategy_engine::StrategyActionRecord, strategy_executor::types::RuntimeStrategyExecutionLog,
};

#[derive(Clone, Debug)]
pub(crate) struct StrategyExecutionIntent {
    pub(crate) action: StrategyIntentAction,
    pub(crate) action_record: StrategyActionRecord,
    pub(crate) symbol: String,
    pub(crate) inst_type: String,
    pub(crate) timeframe: String,
    pub(crate) order_type: String,
    pub(crate) order_side: Option<String>,
    pub(crate) exchange_size: Option<String>,
    pub(crate) planned_exit: Option<StrategyPlannedExitIntent>,
    pub(crate) cancel_order: Option<StrategyCancelOrderIntent>,
    pub(crate) modify_order: Option<StrategyModifyOrderIntent>,
    pub(crate) stop_loss: Option<f64>,
    pub(crate) take_profit: Option<f64>,
    pub(crate) max_slippage: Option<f64>,
    pub(crate) attached_risk_orders: Vec<StrategyRiskOrderIntent>,
}

#[derive(Clone, Debug)]
pub(crate) struct StrategyExecutionConfig {
    pub(crate) symbol: String,
    pub(crate) inst_type: String,
    pub(crate) timeframe: String,
    pub(crate) stop_loss: f64,
    pub(crate) take_profit: f64,
    pub(crate) params: Value,
}

impl StrategyExecutionIntent {
    pub(crate) fn apply_config_overrides(
        &self,
        mut config: StrategyExecutionConfig,
    ) -> StrategyExecutionConfig {
        if !self.symbol.trim().is_empty() {
            config.symbol = self.symbol.clone();
        }
        if !self.inst_type.trim().is_empty() {
            config.inst_type = self.inst_type.clone();
        }
        if !self.timeframe.trim().is_empty() {
            config.timeframe = self.timeframe.clone();
        }
        if let Some(stop_loss) = self.stop_loss {
            config.stop_loss = stop_loss.clamp(0.0, 1.0);
        }
        if let Some(take_profit) = self.take_profit {
            config.take_profit = take_profit.clamp(0.0, 5.0);
        }
        if let Some(max_slippage) = self.max_slippage {
            let value = serde_json::json!(max_slippage.clamp(0.0, 1.0));
            if let Some(params) = config.params.as_object_mut() {
                params.insert("_runtime_max_slippage".to_string(), value);
            } else {
                config.params = serde_json::json!({"_runtime_max_slippage": value});
            }
        }
        config
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StrategyPlannedExitIntent {
    pub(crate) timestamp: i64,
    pub(crate) reason: String,
    pub(crate) contract: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StrategyCancelOrderIntent {
    pub(crate) order_id: String,
    pub(crate) client_order_id: String,
    pub(crate) scope_explicit: bool,
    pub(crate) target_kind: StrategyOrderTargetKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StrategyModifyOrderIntent {
    pub(crate) order_id: String,
    pub(crate) client_order_id: String,
    pub(crate) new_size: Option<String>,
    pub(crate) new_price: Option<String>,
    pub(crate) cancel_on_fail: bool,
    pub(crate) request_id: String,
    pub(crate) scope_explicit: bool,
    pub(crate) target_kind: StrategyOrderTargetKind,
    pub(crate) target_order_type: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum StrategyOrderTargetKind {
    #[default]
    Any,
    Exchange,
    Algo,
}

impl StrategyOrderTargetKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::Exchange => "exchange",
            Self::Algo => "algo",
        }
    }

    pub(crate) fn allows_exchange(self) -> bool {
        matches!(self, Self::Any | Self::Exchange)
    }

    pub(crate) fn allows_algo(self) -> bool {
        matches!(self, Self::Any | Self::Algo)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct StrategyRiskOrderIntent {
    pub(crate) symbol: String,
    pub(crate) side: String,
    pub(crate) order_type: String,
    pub(crate) trigger_price: Option<f64>,
    pub(crate) stop_loss: Option<f64>,
    pub(crate) take_profit: Option<f64>,
    pub(crate) reason: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StrategyIntentAction {
    OpenPosition,
    ClosePosition,
    PlaceRiskOrder,
    CancelOrder,
    ModifyOrder,
    Hold,
}

impl StrategyIntentAction {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::OpenPosition => "open_position",
            Self::ClosePosition => "close_position",
            Self::PlaceRiskOrder => "place_risk_order",
            Self::CancelOrder => "cancel_order",
            Self::ModifyOrder => "modify_order",
            Self::Hold => "hold",
        }
    }

    pub(crate) fn closes_position(self) -> bool {
        matches!(self, Self::ClosePosition)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct StrategyExecutionPlan {
    pub(crate) intents: Vec<StrategyExecutionIntent>,
    pub(crate) risk_actions: Vec<Value>,
    pub(crate) skipped_actions: Vec<Value>,
    pub(crate) idle_actions: Vec<Value>,
    pub(crate) execution_logs: Vec<RuntimeStrategyExecutionLog>,
    pub(crate) diagnostics: Value,
}
