use serde_json::Value;

use crate::{
    live_strategy::{
        decision::{StrategyExecutionIntent, StrategyIntentAction},
        LiveStrategyConfig,
    },
    trading_semantics::{
        live_configured_leverage_from_params, live_contract_mode_enabled, live_td_mode_from_params,
    },
};

use super::builder::execution_gate;

pub(in crate::commands::local_api::live::execution) fn execution_params_gate(
    config: &LiveStrategyConfig,
    intents: &[StrategyExecutionIntent],
) -> Value {
    let trade_intents = intents
        .iter()
        .filter(|intent| {
            matches!(
                intent.action,
                StrategyIntentAction::OpenPosition
                    | StrategyIntentAction::ClosePosition
                    | StrategyIntentAction::PlaceRiskOrder
            )
        })
        .collect::<Vec<_>>();
    if trade_intents.is_empty() {
        return execution_gate(
            "execution_params",
            "执行参数",
            "skip",
            true,
            false,
            "当前动作不需要校验下单交易参数",
        );
    }

    for intent in trade_intents {
        let execution_config = intent.execution_config(config);
        if let Err(error) =
            live_contract_mode_enabled(&execution_config.params, &execution_config.inst_type)
        {
            return execution_gate(
                "execution_params",
                "执行参数",
                "block",
                false,
                true,
                &format!(
                    "{} {}: {}",
                    execution_config.symbol, execution_config.inst_type, error
                ),
            );
        }
        if let Err(error) =
            live_td_mode_from_params(&execution_config.params, &execution_config.inst_type)
        {
            return execution_gate(
                "execution_params",
                "执行参数",
                "block",
                false,
                true,
                &format!(
                    "{} {}: {}",
                    execution_config.symbol, execution_config.inst_type, error
                ),
            );
        }
        if intent.action == StrategyIntentAction::OpenPosition {
            if let Err(error) = live_configured_leverage_from_params(
                &execution_config.params,
                &execution_config.inst_type,
            ) {
                return execution_gate(
                    "execution_params",
                    "执行参数",
                    "block",
                    false,
                    true,
                    &format!(
                        "{} {}: {}",
                        execution_config.symbol, execution_config.inst_type, error
                    ),
                );
            }
        }
    }

    execution_gate(
        "execution_params",
        "执行参数",
        "pass",
        true,
        false,
        "有效执行动作的交易参数与交易品类一致",
    )
}
