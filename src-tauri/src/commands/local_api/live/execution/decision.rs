use serde_json::{json, Value};

use crate::{
    live_strategy::{
        decision::{plan_runtime_actions_for_execution, StrategyExecutionIntent},
        LiveStrategyConfig, LiveStrategyStatus,
    },
    okx::OkxCandle,
    risk_controls,
    strategy_engine::StrategyConfig,
    strategy_executor::types::RuntimeStrategyAction,
};

use super::gates::{execution_gate, execution_mode_gate, execution_params_gate, runtime_gate};

pub(in crate::commands::local_api::live) enum ExchangeRiskEvidence {
    State,
    Error(String),
}

pub(in crate::commands::local_api::live) fn attach_execution_decision(
    diagnostics: &mut Value,
    actions: &[RuntimeStrategyAction],
    config: &LiveStrategyConfig,
    runtime_status: &LiveStrategyStatus,
    latest_candle: &OkxCandle,
    exchange_risk_evidence: Option<&ExchangeRiskEvidence>,
) {
    let decision = build_execution_decision_with_exchange_risk_evidence(
        actions,
        config,
        runtime_status,
        latest_candle,
        exchange_risk_evidence,
    );
    if let Some(obj) = diagnostics.as_object_mut() {
        obj.insert("execution_decision".to_string(), decision);
    }
}

pub(in crate::commands::local_api::live) fn execution_decision_requires_exchange_risk_state(
    config: &LiveStrategyConfig,
) -> bool {
    risk_controls::max_daily_loss_ratio(&config.params).is_some()
        || risk_controls::max_same_direction_exposure_pct(&config.params).is_some()
}

#[cfg(test)]
pub(in crate::commands::local_api::live::execution) fn build_execution_decision(
    actions: &[RuntimeStrategyAction],
    config: &LiveStrategyConfig,
    runtime_status: &LiveStrategyStatus,
    latest_candle: &OkxCandle,
) -> Value {
    build_execution_decision_with_exchange_risk_evidence(
        actions,
        config,
        runtime_status,
        latest_candle,
        None,
    )
}

pub(in crate::commands::local_api::live::execution) fn build_execution_decision_with_exchange_risk_evidence(
    actions: &[RuntimeStrategyAction],
    config: &LiveStrategyConfig,
    runtime_status: &LiveStrategyStatus,
    latest_candle: &OkxCandle,
    exchange_risk_evidence: Option<&ExchangeRiskEvidence>,
) -> Value {
    let action_values = actions
        .iter()
        .map(RuntimeStrategyAction::to_value)
        .collect::<Vec<_>>();
    let planner_config = strategy_config_from_live(config);
    let (intents, risk_actions, skipped_actions, idle_actions) =
        plan_runtime_actions_for_execution(actions, latest_candle, &planner_config);
    let action_summary = action_summary(actions);
    let intent = decision_intent(&action_summary);
    let runtime_active =
        runtime_status.status == "running" && !runtime_status.run_id.trim().is_empty();
    let target_matches_runtime = runtime_active
        && runtime_status.strategy_id == config.strategy_id
        && runtime_status.symbol.eq_ignore_ascii_case(&config.symbol)
        && runtime_status
            .timeframe
            .eq_ignore_ascii_case(&config.timeframe)
        && runtime_status
            .inst_type
            .eq_ignore_ascii_case(&config.inst_type)
        && runtime_status.mode.eq_ignore_ascii_case(&config.mode);
    let execution_mode = config.runtime_execution_mode();
    let risk_reason = action_risk_reason(config, &action_summary, exchange_risk_evidence);
    let mut gates = vec![
        action_gate(&intent, &action_values, &action_summary, &skipped_actions),
        runtime_gate(runtime_active, target_matches_runtime, runtime_status),
        execution_mode_gate(
            runtime_active,
            target_matches_runtime,
            execution_mode,
            &intent,
        ),
        execution_params_gate(config, &intents),
    ];
    if !risk_reason.is_empty() {
        gates.push(execution_gate(
            "risk",
            "交易所风控状态",
            "block",
            false,
            true,
            &risk_reason,
        ));
    }

    let blocking_keys = gates
        .iter()
        .filter(|gate| {
            gate.get("blocking")
                .and_then(Value::as_bool)
                .expect("execution gate blocking should be boolean")
        })
        .filter_map(|gate| gate.get("key").and_then(Value::as_str).map(str::to_string))
        .collect::<Vec<_>>();
    let verdict = action_execution_verdict(
        &intent,
        runtime_active,
        target_matches_runtime,
        &blocking_keys,
    );
    let executable_intent_count = executable_intent_count(&intents);

    json!({
        "verdict": verdict,
        "summary": action_execution_summary(&intent, &verdict, &blocking_keys, &action_summary),
        "executable_intent_count": executable_intent_count,
        "risk_action_count": risk_actions.len(),
        "skipped_action_count": skipped_actions.len(),
        "idle_action_count": idle_actions.len(),
        "skipped_actions": skipped_actions,
        "gates": gates,
    })
}

fn action_summary(actions: &[RuntimeStrategyAction]) -> Value {
    let mut open_position = 0_u64;
    let mut close_position = 0_u64;
    let mut place_risk_order = 0_u64;
    let mut cancel_order = 0_u64;
    let mut modify_order = 0_u64;
    let mut hold = 0_u64;
    for action in actions {
        match action.action.trim().to_ascii_lowercase().as_str() {
            "open_position" => open_position += 1,
            "close_position" => close_position += 1,
            "place_risk_order" => place_risk_order += 1,
            "cancel_order" => cancel_order += 1,
            "modify_order" => modify_order += 1,
            "hold" => hold += 1,
            _ => unreachable!("runtime parser rejects unsupported actions"),
        }
    }
    json!({
        "open_position": open_position,
        "close_position": close_position,
        "place_risk_order": place_risk_order,
        "cancel_order": cancel_order,
        "modify_order": modify_order,
        "hold": hold,
        "total": actions.len(),
    })
}

fn summary_count(summary: &Value, key: &str) -> u64 {
    summary
        .get(key)
        .and_then(Value::as_u64)
        .expect("execution action summary count should be u64")
}

fn decision_intent(summary: &Value) -> String {
    let executable = summary_count(summary, "open_position")
        + summary_count(summary, "close_position")
        + summary_count(summary, "place_risk_order")
        + summary_count(summary, "cancel_order")
        + summary_count(summary, "modify_order");
    if executable == 0 {
        return "hold".to_string();
    }
    let mut kinds = Vec::new();
    for key in [
        "open_position",
        "close_position",
        "place_risk_order",
        "cancel_order",
        "modify_order",
    ] {
        if summary_count(summary, key) > 0 {
            kinds.push(key);
        }
    }
    if kinds.len() == 1 {
        kinds[0].to_string()
    } else {
        "mixed".to_string()
    }
}

fn action_gate(
    intent: &str,
    actions: &[Value],
    summary: &Value,
    skipped_actions: &[Value],
) -> Value {
    if actions.is_empty() || intent == "hold" {
        return json!({
            "key": "actions",
            "label": "策略动作",
            "status": "wait",
            "passed": false,
            "blocking": false,
            "detail": "策略当前未返回可执行动作",
        });
    }
    if !skipped_actions.is_empty() {
        return json!({
            "key": "actions",
            "label": "策略动作",
            "status": "block",
            "passed": false,
            "blocking": true,
            "detail": skipped_action_summary(skipped_actions),
        });
    }
    json!({
        "key": "actions",
        "label": "策略动作",
        "status": "pass",
        "passed": true,
        "blocking": false,
        "detail": action_summary_text(summary),
    })
}

fn skipped_action_summary(skipped_actions: &[Value]) -> String {
    let Some(first) = skipped_actions.first() else {
        return "动作合约未通过".to_string();
    };
    let reason = first
        .get("_execution_skip_reason")
        .and_then(Value::as_str)
        .or_else(|| first.get("reason").and_then(Value::as_str))
        .unwrap_or("动作合约未通过");
    if skipped_actions.len() == 1 {
        format!("动作合约未通过：{reason}")
    } else {
        format!(
            "动作合约未通过：{reason}；另有 {} 个动作被跳过",
            skipped_actions.len().saturating_sub(1)
        )
    }
}

fn action_summary_text(summary: &Value) -> String {
    let labels = [
        ("open_position", "开仓"),
        ("close_position", "平仓"),
        ("place_risk_order", "保护单"),
        ("cancel_order", "撤单"),
        ("modify_order", "改单"),
        ("hold", "等待"),
    ];
    let parts = labels
        .iter()
        .filter_map(|(key, label)| {
            let count = summary_count(summary, key);
            (count > 0).then(|| format!("{label} {count}"))
        })
        .collect::<Vec<_>>();
    if parts.is_empty() {
        "策略当前未返回可执行动作".to_string()
    } else {
        format!("策略返回动作：{}", parts.join("，"))
    }
}

fn action_execution_verdict(
    intent: &str,
    runtime_active: bool,
    target_matches_runtime: bool,
    blocking_keys: &[String],
) -> String {
    if !blocking_keys.is_empty() {
        return "blocked".to_string();
    }
    if intent == "hold" {
        return "hold".to_string();
    }
    if !runtime_active {
        return "preview".to_string();
    }
    if !target_matches_runtime {
        return "mismatch".to_string();
    }
    "ready".to_string()
}

fn action_execution_summary(
    intent: &str,
    verdict: &str,
    blocking_keys: &[String],
    summary: &Value,
) -> String {
    match verdict {
        "ready" => format!(
            "{}，运行时会按 actions 合约提交到 OKX。",
            action_summary_text(summary)
        ),
        "preview" => format!(
            "策略未运行；当前仅按 actions 合约预览。{}",
            action_summary_text(summary)
        ),
        "mismatch" => "诊断目标与当前运行策略不一致，不会由当前运行时下单。".to_string(),
        "hold" if intent == "hold" => "策略当前未返回可执行动作。".to_string(),
        "blocked" => format!("执行链路被阻断：{}。", blocking_keys.join(" / ")),
        _ => "当前无法判断执行结果。".to_string(),
    }
}

fn action_risk_reason(
    config: &LiveStrategyConfig,
    summary: &Value,
    exchange_risk_evidence: Option<&ExchangeRiskEvidence>,
) -> String {
    if summary_count(summary, "open_position") == 0 {
        return String::new();
    }
    if !execution_decision_requires_exchange_risk_state(config) {
        return String::new();
    }
    match exchange_risk_evidence {
        Some(ExchangeRiskEvidence::State) => String::new(),
        Some(ExchangeRiskEvidence::Error(error)) => error.clone(),
        None => "缺少 OKX 账户/持仓状态，无法评估交易所风控".to_string(),
    }
}

fn executable_intent_count(intents: &[StrategyExecutionIntent]) -> usize {
    intents.len()
}

fn strategy_config_from_live(config: &LiveStrategyConfig) -> StrategyConfig {
    StrategyConfig {
        strategy_id: config.strategy_id.clone(),
        strategy_name: config.strategy_name.clone(),
        symbol: config.symbol.clone(),
        inst_type: config.inst_type.clone(),
        timeframe: config.timeframe.clone(),
        initial_capital: config.initial_capital,
        position_size: config.position_size,
        stop_loss: config.stop_loss,
        take_profit: config.take_profit,
        params: config.params.clone(),
    }
}
