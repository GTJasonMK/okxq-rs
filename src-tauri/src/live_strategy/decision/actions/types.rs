use crate::strategy_execution_contract::StrategyExecutionConfig;
pub(crate) use crate::strategy_execution_contract::{
    StrategyCancelOrderIntent, StrategyExecutionIntent, StrategyExecutionPlan,
    StrategyIntentAction, StrategyModifyOrderIntent, StrategyOrderTargetKind,
    StrategyPlannedExitIntent, StrategyRiskOrderIntent,
};

use super::super::super::types::LiveStrategyConfig;

impl StrategyExecutionIntent {
    pub(crate) fn execution_config(&self, base: &LiveStrategyConfig) -> LiveStrategyConfig {
        let mut config = base.clone();
        let execution_config = self.apply_config_overrides(StrategyExecutionConfig {
            symbol: config.symbol.clone(),
            inst_type: config.inst_type.clone(),
            timeframe: config.timeframe.clone(),
            stop_loss: config.stop_loss,
            take_profit: config.take_profit,
            params: config.params.clone(),
        });
        config.symbol = execution_config.symbol;
        config.inst_type = execution_config.inst_type;
        config.timeframe = execution_config.timeframe;
        config.stop_loss = execution_config.stop_loss;
        config.take_profit = execution_config.take_profit;
        config.params = execution_config.params;
        config
    }
}
