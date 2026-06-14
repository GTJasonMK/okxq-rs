use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::{
    okx::OkxCandle,
    risk_controls,
    strategy_engine::{StrategyActionRecord, StrategyConfig},
    strategy_executor::{normalize_runtime_inst_id, types::RuntimeStrategyAction},
    trading_semantics::{
        exchange_order_type_requires_price_safe, live_algo_target_order_kind_from_order_type,
        normalize_runtime_order_type_text, LIVE_ALGO_TARGET_ORDER_TYPES,
        RUNTIME_ACTIONS_V1_ENGINE_CONTROLLED_FIELDS, RUNTIME_ACTIONS_V1_JSON_BOOL_FIELDS,
        RUNTIME_ACTIONS_V1_JSON_NUMBER_FIELDS, RUNTIME_ACTIONS_V1_JSON_TEXT_FIELDS,
        RUNTIME_ACTIONS_V1_REMOVED_FIELD_ALIASES,
    },
};

use super::super::close_order_side_from_text;

#[cfg(test)]
mod tests;
mod types;

pub(crate) use types::{
    StrategyCancelOrderIntent, StrategyExecutionIntent, StrategyExecutionPlan,
    StrategyIntentAction, StrategyModifyOrderIntent, StrategyOrderTargetKind,
    StrategyPlannedExitIntent, StrategyRiskOrderIntent,
};

#[derive(Clone, Copy, Debug, Default)]
struct ActionRiskSettings {
    stop_loss: Option<f64>,
    take_profit: Option<f64>,
    max_slippage: Option<f64>,
}

impl ActionRiskSettings {
    fn merge(self, other: ActionRiskSettings) -> Self {
        Self {
            stop_loss: other.stop_loss.or(self.stop_loss),
            take_profit: other.take_profit.or(self.take_profit),
            max_slippage: other.max_slippage.or(self.max_slippage),
        }
    }
}

pub(crate) fn plan_runtime_actions(
    actions: &[RuntimeStrategyAction],
    latest: &OkxCandle,
    config: &StrategyConfig,
) -> (
    Vec<StrategyExecutionIntent>,
    Vec<Value>,
    Vec<Value>,
    Vec<Value>,
) {
    let mut skipped_actions = Vec::new();
    let mut invalid_risk_by_symbol = HashMap::<String, Vec<String>>::new();
    let mut valid_actions = Vec::new();
    for action in actions {
        if let Some(reason) = disallowed_action_field_reason(action) {
            if is_risk_order_action(action) {
                invalid_risk_by_symbol
                    .entry(action_symbol_for_plan(action, config))
                    .or_default()
                    .push(reason.clone());
            }
            skipped_actions.push(skipped_action_value(action, &reason));
            continue;
        }
        valid_actions.push(action);
    }

    let open_action_symbols = valid_actions
        .iter()
        .copied()
        .filter(|action| !is_risk_order_action(action))
        .filter(|action| {
            action_kind_from_action(action) == Some(StrategyIntentAction::OpenPosition)
        })
        .map(|action| action_symbol_for_plan(action, config))
        .collect::<HashSet<_>>();
    let mut risk_by_symbol = HashMap::<String, ActionRiskSettings>::new();
    let mut attached_by_symbol = HashMap::<String, Vec<StrategyRiskOrderIntent>>::new();
    let mut risk_actions = Vec::new();
    for action in valid_actions.iter().copied() {
        if !is_risk_order_action(action) {
            continue;
        }
        let symbol = action_symbol_for_plan(action, config);
        let current = risk_by_symbol.get(&symbol).copied().unwrap_or_default();
        risk_by_symbol.insert(
            symbol.clone(),
            current.merge(risk_settings_from_action(action)),
        );
        attached_by_symbol
            .entry(symbol)
            .or_default()
            .push(risk_order_intent_from_action(action, config));
        risk_actions.push(action.to_value());
    }

    let mut intents = Vec::new();
    let mut idle_actions = Vec::new();
    for action in valid_actions.iter().copied() {
        if is_risk_order_action(action) {
            continue;
        }
        if action_kind_from_action(action) == Some(StrategyIntentAction::Hold) {
            idle_actions.push(action.to_value());
            continue;
        }
        let symbol = action_symbol_for_plan(action, config);
        if action_kind_from_action(action) == Some(StrategyIntentAction::OpenPosition) {
            if let Some(reasons) = invalid_risk_by_symbol.get(&symbol) {
                skipped_actions.push(skipped_action_value(
                    action,
                    &invalid_attached_risk_block_reason(&symbol, reasons),
                ));
                continue;
            }
        }
        let risk = default_risk_settings(config)
            .merge(risk_by_symbol.get(&symbol).copied().unwrap_or_default())
            .merge(risk_settings_from_action(action));
        let attached_risk_orders = attached_by_symbol.get(&symbol).cloned().unwrap_or_default();
        match intent_from_action(action, latest, config, risk, attached_risk_orders) {
            Ok(intent) => intents.push(intent),
            Err(reason) => skipped_actions.push(skipped_action_value(action, &reason)),
        }
    }

    for action in valid_actions.iter().copied() {
        if !is_risk_order_action(action) {
            continue;
        }
        let symbol = action_symbol_for_plan(action, config);
        if open_action_symbols.contains(&symbol) {
            continue;
        }
        let risk = default_risk_settings(config).merge(risk_settings_from_action(action));
        let attached_risk_orders = vec![risk_order_intent_from_action(action, config)];
        match intent_from_action(action, latest, config, risk, attached_risk_orders) {
            Ok(intent) => intents.push(intent),
            Err(reason) => skipped_actions.push(skipped_action_value(action, &reason)),
        }
    }

    (intents, risk_actions, skipped_actions, idle_actions)
}

pub(crate) fn plan_runtime_actions_for_execution(
    actions: &[RuntimeStrategyAction],
    latest: &OkxCandle,
    config: &StrategyConfig,
) -> (
    Vec<StrategyExecutionIntent>,
    Vec<Value>,
    Vec<Value>,
    Vec<Value>,
) {
    let (mut intents, mut risk_actions, skipped_actions, idle_actions) =
        plan_runtime_actions(actions, latest, config);
    if !skipped_actions.is_empty() {
        intents.clear();
        risk_actions.clear();
    }
    (intents, risk_actions, skipped_actions, idle_actions)
}

fn invalid_attached_risk_block_reason(symbol: &str, reasons: &[String]) -> String {
    let first_reason = reasons
        .first()
        .map(String::as_str)
        .unwrap_or("保护单动作未通过合约校验");
    if reasons.len() > 1 {
        format!(
            "开仓动作 {symbol} 的 {} 个保护单动作未通过合约校验，已拒绝裸开仓: {first_reason}",
            reasons.len()
        )
    } else {
        format!("开仓动作 {symbol} 的保护单动作未通过合约校验，已拒绝裸开仓: {first_reason}")
    }
}

fn intent_from_action(
    action: &RuntimeStrategyAction,
    _latest: &OkxCandle,
    config: &StrategyConfig,
    risk: ActionRiskSettings,
    attached_risk_orders: Vec<StrategyRiskOrderIntent>,
) -> Result<StrategyExecutionIntent, String> {
    if let Some(reason) = disallowed_action_field_reason(action) {
        return Err(reason);
    }
    let action_kind =
        action_kind_from_action(action).ok_or_else(|| unsupported_action_reason(action))?;
    let side = executable_order_side(action, action_kind).ok_or_else(|| {
        format!(
            "动作 {} 缺少可执行 side: {}",
            action.action.trim(),
            action.side.trim()
        )
    })?;
    if action.timestamp <= 0 {
        return Err(format!("动作 {} 缺少有效 timestamp", action.action.trim()));
    }
    let timestamp = action.timestamp;
    if !matches!(
        action_kind,
        StrategyIntentAction::CancelOrder | StrategyIntentAction::ModifyOrder
    ) && (has_invalid_explicit_price(action) || has_invalid_reference_price(action))
    {
        return Err(format!(
            "动作 {} 的显式 price/reference_price 无法解析为有限数字",
            action.action.trim()
        ));
    }
    let order_type = normalized_execution_order_type(action);
    if !matches!(
        action_kind,
        StrategyIntentAction::CancelOrder | StrategyIntentAction::ModifyOrder
    ) && action.price.is_none()
        && exchange_order_type_requires_price_safe(&order_type)
    {
        return Err(format!(
            "动作 {} 的 order_type={} 需要显式 price",
            action.action.trim(),
            order_type
        ));
    }
    let price = execution_reference_price(action, &order_type);
    if action_requires_reference_price(action, action_kind, &order_type)
        && (!price.is_finite() || price <= 0.0)
    {
        return Err(format!(
            "动作 {} 缺少有效 price/reference_price",
            action.action.trim()
        ));
    }
    let action_name = action_kind.as_str();
    let cancel_order = if action_kind == StrategyIntentAction::CancelOrder {
        Some(cancel_order_from_action(action)?)
    } else {
        None
    };
    let modify_order = if action_kind == StrategyIntentAction::ModifyOrder {
        Some(modify_order_from_action(action)?)
    } else {
        None
    };
    Ok(StrategyExecutionIntent {
        action: action_kind,
        action_record: StrategyActionRecord {
            action: action_name.to_string(),
            side,
            price: if price.is_finite() && price > 0.0 {
                price
            } else {
                0.0
            },
            reason: if action.reason.trim().is_empty() {
                action.action.clone()
            } else {
                action.reason.clone()
            },
            strength: action.strength,
            timestamp,
            position_size: action.position_size,
        },
        symbol: action_symbol_for_plan(action, config),
        inst_type: action_inst_type_for_plan(action, config),
        timeframe: action
            .raw
            .get("timeframe")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&config.timeframe)
            .to_string(),
        order_type,
        order_side: close_order_side_from_action(action),
        exchange_size: if matches!(
            action_kind,
            StrategyIntentAction::OpenPosition
                | StrategyIntentAction::ClosePosition
                | StrategyIntentAction::PlaceRiskOrder
        ) {
            explicit_exchange_size_from_action(action)
        } else {
            None
        },
        planned_exit: if action_kind == StrategyIntentAction::OpenPosition {
            planned_exit_from_action(action)
        } else {
            None
        },
        cancel_order,
        modify_order,
        stop_loss: risk.stop_loss,
        take_profit: risk.take_profit,
        max_slippage: risk.max_slippage,
        attached_risk_orders: if matches!(
            action_kind,
            StrategyIntentAction::OpenPosition | StrategyIntentAction::PlaceRiskOrder
        ) {
            attached_risk_orders
        } else {
            Vec::new()
        },
    })
}

fn skipped_action_value(action: &RuntimeStrategyAction, reason: &str) -> Value {
    let mut value = action.to_value();
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "_execution_skip_reason".to_string(),
            Value::String(reason.to_string()),
        );
    }
    value
}

fn normalized_execution_order_type(action: &RuntimeStrategyAction) -> String {
    normalize_runtime_order_type_text(&action.order_type)
}

fn execution_reference_price(action: &RuntimeStrategyAction, order_type: &str) -> f64 {
    if exchange_order_type_requires_price_safe(order_type) {
        return action.price.unwrap_or(f64::NAN);
    }
    action.price.or(action.reference_price).unwrap_or(f64::NAN)
}

fn action_requires_reference_price(
    action: &RuntimeStrategyAction,
    action_kind: StrategyIntentAction,
    order_type: &str,
) -> bool {
    match action_kind {
        StrategyIntentAction::OpenPosition => true,
        StrategyIntentAction::ClosePosition | StrategyIntentAction::PlaceRiskOrder => {
            exchange_order_type_requires_price_safe(order_type) || action.position_size.is_some()
        }
        StrategyIntentAction::CancelOrder
        | StrategyIntentAction::ModifyOrder
        | StrategyIntentAction::Hold => false,
    }
}

fn unsupported_action_reason(action: &RuntimeStrategyAction) -> String {
    let action_name = action.action.trim();
    if action_name.is_empty() {
        return "动作缺少 action 字段，无法映射到实时交易意图".to_string();
    }
    format!(
        "实时策略执行层暂不支持动作 {action_name}，请输出 open_position/close_position/cancel_order/modify_order/hold/place_risk_order"
    )
}

fn has_invalid_explicit_price(action: &RuntimeStrategyAction) -> bool {
    if action
        .raw
        .get("_invalid_price")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return true;
    }
    action
        .raw
        .get("price")
        .is_some_and(|value| !value.is_null() && finite_f64_value(value).is_none())
}

fn has_invalid_reference_price(action: &RuntimeStrategyAction) -> bool {
    if action
        .raw
        .get("_invalid_reference_price")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return true;
    }
    action
        .raw
        .get("reference_price")
        .is_some_and(|value| !value.is_null() && finite_f64_value(value).is_none())
}

fn finite_f64_value(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_i64().map(|item| item as f64))
        .or_else(|| value.as_u64().map(|item| item as f64))
        .filter(|item| item.is_finite())
}

fn positive_i64_value(value: &Value) -> Option<i64> {
    match value {
        Value::Number(item) => item
            .as_i64()
            .or_else(|| item.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| item.as_f64().map(|value| value.round() as i64)),
        _ => None,
    }
    .filter(|value| *value > 0)
}

fn planned_exit_from_action(action: &RuntimeStrategyAction) -> Option<StrategyPlannedExitIntent> {
    let timestamp = action
        .raw
        .get("planned_exit_time")
        .and_then(positive_i64_value)?;
    let reason = action
        .raw
        .get("planned_exit_reason")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("planned_exit")
        .to_string();
    let contract = action
        .raw
        .get("planned_exit_contract")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("planned_exit_time_v1")
        .to_string();
    Some(StrategyPlannedExitIntent {
        timestamp,
        reason,
        contract,
    })
}

fn cancel_order_from_action(
    action: &RuntimeStrategyAction,
) -> Result<StrategyCancelOrderIntent, String> {
    let (order_id, client_order_id) = order_identity_from_action(action, "cancel_order")?;
    Ok(StrategyCancelOrderIntent {
        order_id,
        client_order_id,
        scope_explicit: action_scope_explicit(action),
        target_kind: order_target_kind_from_action(action)?,
    })
}

fn modify_order_from_action(
    action: &RuntimeStrategyAction,
) -> Result<StrategyModifyOrderIntent, String> {
    let (order_id, client_order_id) = order_identity_from_action(action, "modify_order")?;
    let new_size = action.raw.get("new_size").and_then(text_value);
    let new_price = action.raw.get("new_price").and_then(text_value);
    if new_size.is_none() && new_price.is_none() {
        return Err("modify_order 动作缺少显式 new_size 或 new_price".to_string());
    }
    let cancel_on_fail = action
        .raw
        .get("cancel_on_fail")
        .and_then(bool_value)
        .unwrap_or(false);
    let request_id = action
        .raw
        .get("request_id")
        .and_then(text_value)
        .unwrap_or_default();
    Ok(StrategyModifyOrderIntent {
        order_id,
        client_order_id,
        new_size,
        new_price,
        cancel_on_fail,
        request_id,
        scope_explicit: action_scope_explicit(action),
        target_kind: order_target_kind_from_action(action)?,
        target_order_type: action
            .raw
            .get("target_order_type")
            .and_then(text_value)
            .map(|value| normalize_runtime_order_type_text(&value)),
    })
}

fn order_target_kind_from_action(
    action: &RuntimeStrategyAction,
) -> Result<StrategyOrderTargetKind, String> {
    let Some(value) = action.raw.get("target_order_kind").and_then(text_value) else {
        return Ok(StrategyOrderTargetKind::Any);
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "any" => Ok(StrategyOrderTargetKind::Any),
        "exchange" => Ok(StrategyOrderTargetKind::Exchange),
        "algo" => Ok(StrategyOrderTargetKind::Algo),
        _ => Err(format!(
            "动作 {} 的 target_order_kind 只能是 any、exchange 或 algo",
            action.action.trim()
        )),
    }
}

fn explicit_exchange_size_from_action(action: &RuntimeStrategyAction) -> Option<String> {
    action
        .exchange_size
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn order_identity_from_action(
    action: &RuntimeStrategyAction,
    action_name: &str,
) -> Result<(String, String), String> {
    let order_id = action
        .raw
        .get("order_id")
        .and_then(text_value)
        .unwrap_or_default();
    let client_order_id = action
        .raw
        .get("client_order_id")
        .and_then(text_value)
        .unwrap_or_default();
    if order_id.is_empty() && client_order_id.is_empty() {
        return Err(format!(
            "{action_name} 动作缺少 order_id 或 client_order_id"
        ));
    }
    Ok((order_id, client_order_id))
}

fn removed_action_alias_reason(action: &RuntimeStrategyAction) -> Option<String> {
    let object = action.raw.as_object()?;
    RUNTIME_ACTIONS_V1_REMOVED_FIELD_ALIASES
        .iter()
        .find(|(alias, _)| object.contains_key(*alias))
        .map(|(alias, canonical)| {
            format!(
                "动作 {} 使用已删除字段别名 {alias}，请使用 {canonical}",
                action.action.trim()
            )
        })
}

fn engine_controlled_action_field_reason(action: &RuntimeStrategyAction) -> Option<String> {
    let object = action.raw.as_object()?;
    RUNTIME_ACTIONS_V1_ENGINE_CONTROLLED_FIELDS
        .iter()
        .find(|(field, _)| object.contains_key(*field))
        .map(|(field, reason)| {
            format!(
                "动作 {} 使用交易引擎控制字段 {field}: {reason}",
                action.action.trim()
            )
        })
}

fn disallowed_action_field_reason(action: &RuntimeStrategyAction) -> Option<String> {
    removed_action_alias_reason(action)
        .or_else(|| engine_controlled_action_field_reason(action))
        .or_else(|| invalid_json_number_action_field_reason(action))
        .or_else(|| invalid_json_bool_action_field_reason(action))
        .or_else(|| invalid_json_text_action_field_reason(action))
        .or_else(|| invalid_target_order_type_reason(action))
}

fn text_value(value: &Value) -> Option<String> {
    match value {
        Value::String(item) => {
            let item = item.trim();
            (!item.is_empty()).then(|| item.to_string())
        }
        _ => None,
    }
}

fn bool_value(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(item) => Some(*item),
        _ => None,
    }
}

fn invalid_json_number_action_field_reason(action: &RuntimeStrategyAction) -> Option<String> {
    let object = action.raw.as_object()?;
    RUNTIME_ACTIONS_V1_JSON_NUMBER_FIELDS
        .iter()
        .find(|field| {
            object
                .get(**field)
                .is_some_and(|value| !value.is_null() && finite_f64_value(value).is_none())
        })
        .map(|field| {
            format!(
                "动作 {} 的 {field} 必须是 JSON number，不能使用字符串数字",
                action.action.trim()
            )
        })
}

fn invalid_json_bool_action_field_reason(action: &RuntimeStrategyAction) -> Option<String> {
    let object = action.raw.as_object()?;
    RUNTIME_ACTIONS_V1_JSON_BOOL_FIELDS
        .iter()
        .find(|field| {
            object
                .get(**field)
                .is_some_and(|value| !value.is_null() && !value.is_boolean())
        })
        .map(|field| {
            format!(
                "动作 {} 的 {field} 必须是 JSON boolean",
                action.action.trim()
            )
        })
}

fn invalid_json_text_action_field_reason(action: &RuntimeStrategyAction) -> Option<String> {
    let object = action.raw.as_object()?;
    RUNTIME_ACTIONS_V1_JSON_TEXT_FIELDS
        .iter()
        .find(|field| {
            object
                .get(**field)
                .is_some_and(|value| !value.is_null() && !value.is_string())
        })
        .map(|field| {
            format!(
                "动作 {} 的 {field} 必须是 JSON string",
                action.action.trim()
            )
        })
}

fn invalid_target_order_type_reason(action: &RuntimeStrategyAction) -> Option<String> {
    let value = action.raw.get("target_order_type").and_then(text_value)?;
    if live_algo_target_order_kind_from_order_type(&value).is_some() {
        return None;
    }
    Some(format!(
        "动作 {} 的 target_order_type={} 不受支持，必须使用保护单类型: {}",
        action.action.trim(),
        value,
        LIVE_ALGO_TARGET_ORDER_TYPES.join(", ")
    ))
}

fn is_risk_order_action(action: &RuntimeStrategyAction) -> bool {
    action
        .action
        .trim()
        .eq_ignore_ascii_case("place_risk_order")
}

fn action_symbol_for_plan(action: &RuntimeStrategyAction, config: &StrategyConfig) -> String {
    let raw_symbol = if action.symbol.trim().is_empty() {
        config.symbol.as_str()
    } else {
        action.symbol.as_str()
    };
    normalize_runtime_inst_id(raw_symbol, &action_inst_type_for_plan(action, config))
        .expect("runtime action symbol should be normalizable after config validation")
}

fn action_inst_type_for_plan(action: &RuntimeStrategyAction, config: &StrategyConfig) -> String {
    if let Some(inst_type) = action
        .raw
        .get("inst_type")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return inst_type.to_ascii_uppercase();
    }
    if action.symbol.trim().to_ascii_uppercase().ends_with("-SWAP") {
        return "SWAP".to_string();
    }
    config.inst_type.to_ascii_uppercase()
}

fn action_scope_explicit(action: &RuntimeStrategyAction) -> bool {
    if let Some(value) = action.raw.get("_symbol_explicit").and_then(Value::as_bool) {
        return value;
    }
    action.raw.get("symbol").and_then(text_value).is_some()
}

fn default_risk_settings(config: &StrategyConfig) -> ActionRiskSettings {
    ActionRiskSettings {
        stop_loss: configured_risk_decimal(config, config.stop_loss, "stop_loss"),
        take_profit: configured_risk_decimal(config, config.take_profit, "take_profit"),
        max_slippage: risk_controls::risk_decimal(
            &config.params,
            "max_slippage_bps",
            "max_slippage_pct",
            "max_slippage",
        )
        .map(|value| value.clamp(0.0, 1.0)),
    }
}

fn risk_settings_from_action(action: &RuntimeStrategyAction) -> ActionRiskSettings {
    ActionRiskSettings {
        stop_loss: risk_decimal(action, "stop_loss_bps", "stop_loss_pct", "stop_loss"),
        take_profit: risk_decimal(action, "take_profit_bps", "take_profit_pct", "take_profit"),
        max_slippage: risk_decimal(
            action,
            "max_slippage_bps",
            "max_slippage_pct",
            "max_slippage",
        ),
    }
}

fn risk_order_intent_from_action(
    action: &RuntimeStrategyAction,
    config: &StrategyConfig,
) -> StrategyRiskOrderIntent {
    let risk = risk_settings_from_action(action);
    StrategyRiskOrderIntent {
        symbol: action_symbol_for_plan(action, config),
        side: close_order_side_from_action(action).unwrap_or_default(),
        order_type: risk_order_type_from_action(action),
        trigger_price: explicit_risk_trigger_price(action),
        stop_loss: risk.stop_loss,
        take_profit: risk.take_profit,
        reason: if action.reason.trim().is_empty() {
            action.action.clone()
        } else {
            action.reason.clone()
        },
    }
}

fn risk_order_type_from_action(action: &RuntimeStrategyAction) -> String {
    action
        .raw
        .get("target_order_type")
        .and_then(text_value)
        .or_else(|| {
            (!action.order_type.trim().is_empty()).then(|| action.order_type.trim().to_string())
        })
        .map(|value| normalize_runtime_order_type_text(&value))
        .unwrap_or_default()
}

fn explicit_risk_trigger_price(action: &RuntimeStrategyAction) -> Option<f64> {
    [
        "trigger_price",
        "stop_price",
        "sl_trigger_px",
        "slTriggerPx",
        "take_profit_price",
        "tp_trigger_px",
        "tpTriggerPx",
    ]
    .iter()
    .find_map(|key| action.raw.get(*key).and_then(finite_f64_value))
}

fn configured_risk_decimal(
    config: &StrategyConfig,
    configured_value: f64,
    decimal_key: &str,
) -> Option<f64> {
    if configured_value.is_finite() && configured_value > 0.0 {
        return Some(configured_value.clamp(0.0, 5.0));
    }
    let bps_key = format!("{decimal_key}_bps");
    let pct_key = format!("{decimal_key}_pct");
    risk_controls::risk_decimal(&config.params, &bps_key, &pct_key, decimal_key)
        .map(|value| value.clamp(0.0, 5.0))
}

fn risk_decimal(
    action: &RuntimeStrategyAction,
    bps_key: &str,
    pct_key: &str,
    decimal_key: &str,
) -> Option<f64> {
    risk_controls::risk_decimal(&action.raw, bps_key, pct_key, decimal_key)
        .map(|value| value.clamp(0.0, 5.0))
}

fn action_kind_from_action(action: &RuntimeStrategyAction) -> Option<StrategyIntentAction> {
    match action.action.trim().to_ascii_lowercase().as_str() {
        "open_position" => Some(StrategyIntentAction::OpenPosition),
        "close_position" => Some(StrategyIntentAction::ClosePosition),
        "place_risk_order" => Some(StrategyIntentAction::PlaceRiskOrder),
        "cancel_order" => Some(StrategyIntentAction::CancelOrder),
        "modify_order" => Some(StrategyIntentAction::ModifyOrder),
        "hold" => Some(StrategyIntentAction::Hold),
        _ => None,
    }
}

fn executable_order_side(
    action: &RuntimeStrategyAction,
    action_kind: StrategyIntentAction,
) -> Option<String> {
    match action_kind {
        StrategyIntentAction::OpenPosition => {
            match action.side.trim().to_ascii_lowercase().as_str() {
                "long" | "buy" => Some("buy".to_string()),
                "short" | "sell" => Some("sell".to_string()),
                _ => None,
            }
        }
        StrategyIntentAction::ClosePosition => Some("flat".to_string()),
        StrategyIntentAction::PlaceRiskOrder => {
            Some(close_order_side_from_action(action).unwrap_or_else(|| "flat".to_string()))
        }
        StrategyIntentAction::CancelOrder => Some("hold".to_string()),
        StrategyIntentAction::ModifyOrder => Some("hold".to_string()),
        StrategyIntentAction::Hold => Some("hold".to_string()),
    }
}

fn close_order_side_from_action(action: &RuntimeStrategyAction) -> Option<String> {
    action
        .raw
        .get("order_side")
        .and_then(Value::as_str)
        .or_else(|| action.raw.get("close_side").and_then(Value::as_str))
        .or_else(|| (!action.side.trim().is_empty()).then_some(action.side.as_str()))
        .and_then(close_order_side_from_text)
        .map(ToOwned::to_owned)
}
