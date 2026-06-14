use crate::{
    error::{AppError, AppResult},
    okx::{normalized_okx_order_type, okx_order_type_requires_price},
};

use serde_json::Value;

#[derive(Clone, Debug)]
pub(crate) struct InstrumentTradeRules {
    pub(crate) inst_id: String,
    pub(crate) min_sz: Option<f64>,
    pub(crate) lot_sz: Option<f64>,
    pub(crate) tick_sz: Option<f64>,
    pub(crate) ct_val: Option<f64>,
    pub(crate) ct_val_ccy: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ResolvedExchangeQuantity {
    pub(crate) exchange_quantity: f64,
    pub(crate) base_quantity: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ModifyOrderFieldSemantics {
    pub(crate) missing_message: &'static str,
    pub(crate) size_field: &'static str,
    pub(crate) size_label: &'static str,
    pub(crate) size_consequence: &'static str,
    pub(crate) price_field: &'static str,
    pub(crate) price_label: &'static str,
    pub(crate) price_consequence: &'static str,
}

pub(crate) const EXCHANGE_MODIFY_ORDER_FIELDS: ModifyOrderFieldSemantics =
    ModifyOrderFieldSemantics {
        missing_message: "OKX 改单参数 newSz/newPx 至少传一个",
        size_field: "newSz",
        size_label: "改单 newSz",
        size_consequence: "已拒绝改单以避免静默改量",
        price_field: "newPx",
        price_label: "改单 newPx",
        price_consequence: "已拒绝改单以避免静默改价",
    };

pub(crate) const ALGO_MODIFY_ORDER_FIELDS: ModifyOrderFieldSemantics = ModifyOrderFieldSemantics {
    missing_message: "OKX 保护单改单参数 newSz/newTriggerPx 至少传一个",
    size_field: "newSz",
    size_label: "保护单改单 newSz",
    size_consequence: "已拒绝保护单改单以避免静默改量",
    price_field: "newTriggerPx",
    price_label: "保护单改单触发价",
    price_consequence: "已拒绝保护单改单以避免静默改价",
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProtectiveOrderKind {
    StopLoss,
    TakeProfit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StandaloneRiskOrderSelectionError {
    Missing,
    Multiple { count: usize },
}

impl ProtectiveOrderKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::StopLoss => "止损",
            Self::TakeProfit => "止盈",
        }
    }
}

pub(crate) fn is_contract_inst_type(inst_type: &str) -> bool {
    matches!(
        inst_type.trim().to_ascii_uppercase().as_str(),
        "SWAP" | "FUTURES"
    )
}

pub(crate) fn has_periodic_funding_rate(inst_type: &str) -> bool {
    inst_type.trim().eq_ignore_ascii_case("SWAP")
}

pub(crate) fn contract_mode_param_or_default(
    params: &Value,
    inst_type: &str,
    context_label: &str,
) -> AppResult<bool> {
    let is_contract = is_contract_inst_type(inst_type);
    let Some(raw) = params.get("contract_mode") else {
        return Ok(is_contract);
    };
    if raw.is_null() {
        return Ok(is_contract);
    }
    let enabled = raw
        .as_bool()
        .or_else(|| {
            let text = raw.as_str()?.trim().to_ascii_lowercase();
            match text.as_str() {
                "true" | "1" | "yes" | "on" => Some(true),
                "false" | "0" | "no" | "off" => Some(false),
                _ => None,
            }
        })
        .ok_or_else(|| {
            AppError::Validation(format!(
                "{context_label} contract_mode 必须是布尔值或 true/false 字符串"
            ))
        })?;
    Ok(enabled)
}

pub(crate) fn execution_contract_mode_enabled(
    params: &Value,
    inst_type: &str,
    context_label: &str,
) -> AppResult<bool> {
    let is_contract = is_contract_inst_type(inst_type);
    let enabled = contract_mode_param_or_default(params, inst_type, context_label)?;
    if is_contract && !enabled {
        return Err(AppError::Validation(format!(
            "{context_label} inst_type={} 是合约交易，但 contract_mode=false；已拒绝下单以避免跳过合约杠杆和保证金语义",
            inst_type
        )));
    }
    if !is_contract && enabled {
        return Err(AppError::Validation(format!(
            "{context_label} inst_type={} 不是合约交易，但 contract_mode=true；已拒绝下单以避免误用合约杠杆参数",
            inst_type
        )));
    }
    Ok(enabled)
}

pub(crate) fn live_contract_mode_enabled(params: &Value, inst_type: &str) -> AppResult<bool> {
    execution_contract_mode_enabled(params, inst_type, "实时策略")
}

pub(crate) fn live_td_mode_from_params(params: &Value, inst_type: &str) -> AppResult<String> {
    let contract_mode = live_contract_mode_enabled(params, inst_type)?;
    for key in ["td_mode", "mgn_mode", "margin_mode"] {
        let Some(raw) = params.get(key) else {
            continue;
        };
        if raw.is_null() {
            continue;
        }
        let value = raw.as_str().map(str::trim).ok_or_else(|| {
            AppError::Validation(format!("实时策略 {key} 必须是字符串: cash/cross/isolated"))
        })?;
        if value.is_empty() {
            continue;
        }
        return validate_live_td_mode(inst_type, key, value);
    }
    if contract_mode {
        Ok("cross".to_string())
    } else {
        Ok("cash".to_string())
    }
}

pub(crate) fn configured_leverage_from_params(
    params: &Value,
    inst_type: &str,
    context_label: &str,
) -> AppResult<Option<f64>> {
    if !execution_contract_mode_enabled(params, inst_type, context_label)? {
        return Ok(None);
    }
    explicit_configured_leverage_value(params, context_label)
}

pub(crate) fn explicit_configured_leverage_value(
    params: &Value,
    context_label: &str,
) -> AppResult<Option<f64>> {
    let Some(value) = params.get("leverage") else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let leverage = match value {
        Value::Number(item) => item.as_f64().ok_or_else(|| {
            AppError::Validation(format!("{context_label} leverage 必须是有效正数"))
        })?,
        Value::String(item) => item.trim().parse::<f64>().map_err(|_| {
            AppError::Validation(format!("{context_label} leverage 必须是有效正数"))
        })?,
        _ => {
            return Err(AppError::Validation(format!(
                "{context_label} leverage 必须是 JSON number 或数字字符串"
            )))
        }
    };
    if !leverage.is_finite() || leverage <= 0.0 {
        return Err(AppError::Validation(format!(
            "{context_label} leverage 必须是有效正数"
        )));
    }
    if leverage > 125.0 {
        return Err(AppError::Validation(format!(
            "{context_label} leverage={} 超过 OKX 通用最大杠杆 125x，已拒绝下单以避免静默改杠杆",
            leverage
        )));
    }
    Ok(Some(leverage))
}

pub(crate) fn live_configured_leverage_from_params(
    params: &Value,
    inst_type: &str,
) -> AppResult<Option<f64>> {
    configured_leverage_from_params(params, inst_type, "实时策略")
}

fn validate_live_td_mode(inst_type: &str, key: &str, value: &str) -> AppResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "cash" if is_contract_inst_type(inst_type) => Err(AppError::Validation(format!(
            "实时策略 {key}=cash 不能用于 {} 合约交易，合约只支持 cross/isolated",
            inst_type
        ))),
        "cash" => Ok(normalized),
        "cross" | "isolated" if is_contract_inst_type(inst_type) => Ok(normalized),
        "cross" | "isolated" => Err(AppError::Validation(format!(
            "实时策略 {key}={} 需要现货杠杆/保证金语义，但当前实盘执行引擎仅支持现货 cash；已拒绝以避免误用保证金交易",
            normalized
        ))),
        _ => Err(AppError::Validation(format!(
            "实时策略 {key}={} 不受支持，必须是 cash/cross/isolated",
            value.trim()
        ))),
    }
}

pub(crate) fn normalize_runtime_order_type_text(order_type: &str) -> String {
    let normalized = order_type.trim().to_ascii_lowercase().replace('-', "_");
    if normalized.is_empty() {
        "market".to_string()
    } else {
        normalized
    }
}

pub(crate) fn runtime_order_status_can_rest(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "submitting"
            | "submit_unknown"
            | "submitted"
            | "pending"
            | "open"
            | "live"
            | "partially_filled"
            | "partial-filled"
            | "partially-filled"
            | "modify_requested"
    )
}

pub(crate) fn runtime_order_type_can_rest(order_type: &str) -> bool {
    let requested_order_type = normalize_runtime_order_type_text(order_type);
    let normalized = normalized_okx_order_type(&requested_order_type)
        .map(str::to_string)
        .unwrap_or(requested_order_type);
    matches!(
        normalized.as_str(),
        "limit" | "post_only" | "mmp" | "mmp_and_post_only"
    )
}

pub(crate) fn order_type_omits_price(order_type: &str) -> bool {
    matches!(
        normalize_runtime_order_type_text(order_type).as_str(),
        "market" | "optimal_limit_ioc"
    )
}

pub(crate) fn immediate_remainder_order_type(order_type: &str) -> bool {
    matches!(
        normalize_runtime_order_type_text(order_type).as_str(),
        "market" | "optimal_limit_ioc" | "ioc" | "fok"
    )
}

pub(crate) fn fill_or_kill_order_type(order_type: &str) -> bool {
    normalize_runtime_order_type_text(order_type) == "fok"
}

pub(crate) fn maker_only_order_type(order_type: &str) -> bool {
    matches!(
        normalize_runtime_order_type_text(order_type).as_str(),
        "post_only" | "mmp_and_post_only"
    )
}

pub(crate) fn normalized_exchange_order_type(order_type: &str) -> AppResult<&'static str> {
    let requested_order_type = normalize_runtime_order_type_text(order_type);
    normalized_okx_order_type(&requested_order_type)
}

pub(crate) fn exchange_order_type_requires_price(order_type: &str) -> AppResult<bool> {
    normalized_exchange_order_type(order_type).map(okx_order_type_requires_price)
}

pub(crate) fn exchange_order_type_requires_price_safe(order_type: &str) -> bool {
    exchange_order_type_requires_price(order_type).unwrap_or(false)
}

pub(crate) fn required_exchange_order_price_type(
    order_type: &str,
    price: f64,
    invalid_price_message: impl FnOnce(&str) -> String,
) -> AppResult<Option<&'static str>> {
    let normalized = normalized_exchange_order_type(order_type)?;
    if !okx_order_type_requires_price(normalized) {
        return Ok(None);
    }
    if !price.is_finite() || price <= 0.0 {
        return Err(AppError::Validation(invalid_price_message(normalized)));
    }
    Ok(Some(normalized))
}

pub(crate) fn validate_exchange_order_price_shape(
    rules: &InstrumentTradeRules,
    order_type: &str,
    order_side: &str,
    price: f64,
    invalid_price_message: impl FnOnce(&str) -> String,
    consequence: &str,
) -> AppResult<()> {
    let Some(normalized) =
        required_exchange_order_price_type(order_type, price, invalid_price_message)?
    else {
        return Ok(());
    };
    validate_price_matches_tick_size(
        rules,
        price,
        &format!("{order_side} {normalized} 价格"),
        consequence,
    )
}

pub(crate) fn max_slippage_from_params(params: &Value) -> Option<f64> {
    numeric_param(params, "_runtime_max_slippage")
        .or_else(|| numeric_param(params, "max_slippage_bps").map(|value| value / 10_000.0))
        .or_else(|| numeric_param(params, "max_slippage_pct"))
        .or_else(|| numeric_param(params, "max_slippage"))
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(|value| value.clamp(0.0, 1.0))
}

pub(crate) fn check_max_adverse_slippage(
    max_slippage: Option<f64>,
    side: &str,
    reference_price: f64,
    bid_px: Option<f64>,
    ask_px: Option<f64>,
) -> (bool, String) {
    let Some(max_slippage) = max_slippage else {
        return (true, String::new());
    };
    if !reference_price.is_finite() || reference_price <= 0.0 {
        return (false, "动作参考价无效，无法检查滑点".to_string());
    }

    let normalized_side = side.trim().to_ascii_lowercase();
    let execution_price = match normalized_side.as_str() {
        "buy" | "long" => ask_px,
        "sell" | "short" => bid_px,
        _ => {
            return (
                false,
                format!("交易方向 {side} 无法用于检查滑点，已拦截订单"),
            );
        }
    };
    let Some(execution_price) = execution_price.filter(|value| value.is_finite() && *value > 0.0)
    else {
        return (
            false,
            "策略设置了最大滑点，但缺少可用 bid/ask 报价，已拦截订单".to_string(),
        );
    };

    let adverse_slippage = match normalized_side.as_str() {
        "buy" | "long" => ((execution_price - reference_price) / reference_price).max(0.0),
        "sell" | "short" => ((reference_price - execution_price) / reference_price).max(0.0),
        _ => 0.0,
    };
    if adverse_slippage > max_slippage {
        return (
            false,
            format!(
                "预估滑点 {:.2} bps 超过策略允许 {:.2} bps",
                adverse_slippage * 10_000.0,
                max_slippage * 10_000.0
            ),
        );
    }

    (true, String::new())
}

fn numeric_param(params: &Value, key: &str) -> Option<f64> {
    params.get(key).and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_i64().map(|item| item as f64))
            .or_else(|| value.as_u64().map(|item| item as f64))
    })
}

pub(crate) fn runtime_order_can_rest(status: &str, order_type: &str) -> bool {
    runtime_order_status_can_rest(status) && runtime_order_type_can_rest(order_type)
}

pub(crate) fn planned_exit_entry_remainder_cancel_needed(status: &str, order_type: &str) -> bool {
    runtime_order_can_rest(status, order_type)
}

pub(crate) fn protective_order_kind_from_order_type(
    order_type: &str,
) -> Option<ProtectiveOrderKind> {
    match normalize_runtime_order_type_text(order_type).as_str() {
        "stop_market" | "stop_loss_market" | "sl_market" | "stop_loss" | "stop" => {
            Some(ProtectiveOrderKind::StopLoss)
        }
        "take_profit_market" | "tp_market" | "take_profit" | "take_profit_limit" => {
            Some(ProtectiveOrderKind::TakeProfit)
        }
        _ => None,
    }
}

pub(crate) fn standalone_risk_order_type(order_type: &str) -> String {
    if order_type.trim().is_empty() {
        "place_risk_order".to_string()
    } else {
        normalize_runtime_order_type_text(order_type)
    }
}

pub(crate) fn select_single_standalone_risk_order<T>(
    risk_orders: &[T],
) -> Result<&T, StandaloneRiskOrderSelectionError> {
    match risk_orders {
        [] => Err(StandaloneRiskOrderSelectionError::Missing),
        [risk] => Ok(risk),
        _ => Err(StandaloneRiskOrderSelectionError::Multiple {
            count: risk_orders.len(),
        }),
    }
}

pub(crate) const LIVE_ALGO_TARGET_ORDER_TYPES: &[&str] = &[
    "stop_market",
    "stop_loss_market",
    "sl_market",
    "stop_loss",
    "stop",
    "take_profit_market",
    "tp_market",
    "take_profit",
];

pub(crate) fn live_algo_target_order_kind_from_order_type(
    order_type: &str,
) -> Option<ProtectiveOrderKind> {
    match normalize_runtime_order_type_text(order_type).as_str() {
        "stop_market" | "stop_loss_market" | "sl_market" | "stop_loss" | "stop" => {
            Some(ProtectiveOrderKind::StopLoss)
        }
        "take_profit_market" | "tp_market" | "take_profit" => Some(ProtectiveOrderKind::TakeProfit),
        _ => None,
    }
}

pub(crate) const RUNTIME_ACTIONS_V1_SUPPORTED_ACTIONS: &[&str] = &[
    "open_position",
    "close_position",
    "place_risk_order",
    "cancel_order",
    "modify_order",
    "hold",
];

pub(crate) const RUNTIME_ACTIONS_V1_REMOVED_ACTION_ALIAS_FIELDS: &[&str] =
    &["intent_action", "signal_type", "type"];

pub(crate) const RUNTIME_ACTIONS_V1_REMOVED_FIELD_ALIASES: &[(&str, &str)] = &[
    ("size", "position_size"),
    ("quantity", "position_size"),
    ("qty", "position_size"),
    ("sz", "position_size"),
    ("exchange_sz", "exchange_size"),
    ("okx_size", "exchange_size"),
    ("okx_sz", "exchange_size"),
    ("order_size", "exchange_size"),
    ("order_sz", "exchange_size"),
    ("reference_px", "reference_price"),
    ("valuation_price", "reference_price"),
    ("valuation_px", "reference_price"),
    ("mark_price", "reference_price"),
    ("mark_px", "reference_price"),
    ("_price_source", "price_source"),
    ("exit_time", "planned_exit_time"),
    ("exit_reason", "planned_exit_reason"),
    ("action_contract_version", "planned_exit_contract"),
    ("new_sz", "new_size"),
    ("newSz", "new_size"),
    ("target_size", "new_size"),
    ("amend_size", "new_size"),
    ("amend_sz", "new_size"),
    ("new_px", "new_price"),
    ("newPx", "new_price"),
    ("target_price", "new_price"),
    ("amend_price", "new_price"),
    ("amend_px", "new_price"),
    ("cxl_on_fail", "cancel_on_fail"),
    ("cxlOnFail", "cancel_on_fail"),
    ("cancelOnFail", "cancel_on_fail"),
    ("req_id", "request_id"),
    ("reqId", "request_id"),
    ("ord_id", "order_id"),
    ("ordId", "order_id"),
    ("target_order_id", "order_id"),
    ("target_ord_id", "order_id"),
    ("cl_ord_id", "client_order_id"),
    ("clOrdId", "client_order_id"),
    ("target_client_order_id", "client_order_id"),
    ("target_cl_ord_id", "client_order_id"),
];

pub(crate) const RUNTIME_ACTIONS_V1_ENGINE_CONTROLLED_FIELDS: &[(&str, &str)] = &[
    (
        "leverage",
        "leverage 由运行参数控制，不允许策略 action 覆盖",
    ),
    ("lever", "leverage 由运行参数控制，不允许策略 action 覆盖"),
    ("td_mode", "td_mode 由运行参数控制，不允许策略 action 覆盖"),
    ("tdMode", "td_mode 由运行参数控制，不允许策略 action 覆盖"),
    ("mgn_mode", "td_mode 由运行参数控制，不允许策略 action 覆盖"),
    ("mgnMode", "td_mode 由运行参数控制，不允许策略 action 覆盖"),
    (
        "margin_mode",
        "td_mode 由运行参数控制，不允许策略 action 覆盖",
    ),
    (
        "marginMode",
        "td_mode 由运行参数控制，不允许策略 action 覆盖",
    ),
    (
        "pos_side",
        "posSide 由 OKX 持仓模式和执行层推导，不允许策略 action 覆盖",
    ),
    (
        "posSide",
        "posSide 由 OKX 持仓模式和执行层推导，不允许策略 action 覆盖",
    ),
    (
        "reduce_only",
        "reduceOnly 由 action 类型推导，不允许策略 action 覆盖",
    ),
    (
        "reduceOnly",
        "reduceOnly 由 action 类型推导，不允许策略 action 覆盖",
    ),
    (
        "tgt_ccy",
        "tgtCcy 由执行层按交易类型处理，不允许策略 action 覆盖",
    ),
    (
        "tgtCcy",
        "tgtCcy 由执行层按交易类型处理，不允许策略 action 覆盖",
    ),
];

pub(crate) const RUNTIME_ACTIONS_V1_JSON_NUMBER_FIELDS: &[&str] = &[
    "price",
    "reference_price",
    "timestamp",
    "position_size",
    "strength",
    "planned_exit_time",
    "stop_loss_bps",
    "stop_loss_pct",
    "stop_loss",
    "take_profit_bps",
    "take_profit_pct",
    "take_profit",
    "max_slippage_bps",
    "max_slippage_pct",
    "max_slippage",
    "trigger_price",
    "stop_price",
    "sl_trigger_px",
    "slTriggerPx",
    "take_profit_price",
    "tp_trigger_px",
    "tpTriggerPx",
];

pub(crate) const RUNTIME_ACTIONS_V1_JSON_BOOL_FIELDS: &[&str] = &["cancel_on_fail"];

pub(crate) const RUNTIME_ACTIONS_V1_JSON_TEXT_FIELDS: &[&str] = &[
    "action",
    "symbol",
    "side",
    "order_type",
    "reason",
    "order_side",
    "close_side",
    "inst_type",
    "timeframe",
    "price_source",
    "exchange_size",
    "planned_exit_reason",
    "planned_exit_contract",
    "order_id",
    "client_order_id",
    "new_size",
    "new_price",
    "request_id",
    "target_order_kind",
    "target_order_type",
];

pub(crate) const RUNTIME_ACTIONS_V1_TARGET_ORDER_KINDS: &[&str] = &["any", "exchange", "algo"];

pub(crate) fn order_management_any_target_kind_collision_reason(action_name: &str) -> String {
    format!(
        "{action_name} target_order_kind=any 同时命中普通订单和保护单，已拒绝执行；请显式提供 target_order_kind=exchange/algo 以避免误撤或误改"
    )
}

pub(crate) fn resolve_protective_order_kind(
    order_type: &str,
    stop_loss: Option<f64>,
    take_profit: Option<f64>,
) -> AppResult<ProtectiveOrderKind> {
    if let Some(kind) = protective_order_kind_from_order_type(order_type) {
        return Ok(kind);
    }
    let has_stop_loss = positive_ratio(stop_loss);
    let has_take_profit = positive_ratio(take_profit);
    match (has_stop_loss, has_take_profit) {
        (true, false) => Ok(ProtectiveOrderKind::StopLoss),
        (false, true) => Ok(ProtectiveOrderKind::TakeProfit),
        _ => Err(AppError::Validation(format!(
            "暂不支持将保护单 order_type={} 映射为 OKX 附加止盈止损，已拒绝裸开仓",
            order_type
        ))),
    }
}

fn positive_ratio(value: Option<f64>) -> bool {
    value.is_some_and(|value| value.is_finite() && value > 0.0)
}

pub(crate) fn expected_protective_close_side(entry_side: &str) -> Option<&'static str> {
    match entry_side.trim().to_ascii_lowercase().as_str() {
        "buy" | "long" => Some("sell"),
        "sell" | "short" => Some("buy"),
        _ => None,
    }
}

pub(crate) fn action_position_side(side: &str) -> Option<(&'static str, i32)> {
    match side.trim().to_ascii_lowercase().as_str() {
        "long" | "buy" => Some(("long", 1)),
        "short" | "sell" => Some(("short", -1)),
        _ => None,
    }
}

pub(crate) fn entry_order_side_for_action_side(side: &str) -> Option<&'static str> {
    match action_position_side(side) {
        Some(("short", _)) => Some("sell"),
        Some(("long", _)) => Some("buy"),
        _ => None,
    }
}

pub(crate) fn close_order_side_from_position_text(side: &str) -> Option<&'static str> {
    match side.trim().to_ascii_lowercase().as_str() {
        "buy" => Some("buy"),
        "sell" => Some("sell"),
        "long" => Some("sell"),
        "short" => Some("buy"),
        _ => None,
    }
}

pub(crate) fn entry_side_for_protective_close_side(order_side: &str) -> Option<&'static str> {
    match order_side.trim().to_ascii_lowercase().as_str() {
        "sell" => Some("buy"),
        "buy" => Some("sell"),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CloseTargetPositionSide {
    Long,
    Short,
}

impl CloseTargetPositionSide {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Long => "long",
            Self::Short => "short",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Long => "多头",
            Self::Short => "空头",
        }
    }
}

pub(crate) fn close_target_position_side(order_side: &str) -> Option<CloseTargetPositionSide> {
    match entry_side_for_protective_close_side(order_side)? {
        "buy" => Some(CloseTargetPositionSide::Long),
        "sell" => Some(CloseTargetPositionSide::Short),
        _ => None,
    }
}

pub(crate) fn close_target_position_side_label(order_side: &str) -> &'static str {
    close_target_position_side(order_side)
        .map(CloseTargetPositionSide::label)
        .unwrap_or("目标方向")
}

pub(crate) fn close_order_side_for_target_position_side(
    position_side: CloseTargetPositionSide,
) -> &'static str {
    match position_side {
        CloseTargetPositionSide::Long => "sell",
        CloseTargetPositionSide::Short => "buy",
    }
}

pub(crate) fn infer_single_close_target_position_side(
    has_long: bool,
    has_short: bool,
    no_position_message: impl Into<String>,
    ambiguous_message: impl Into<String>,
) -> AppResult<CloseTargetPositionSide> {
    match (has_long, has_short) {
        (true, false) => Ok(CloseTargetPositionSide::Long),
        (false, true) => Ok(CloseTargetPositionSide::Short),
        (false, false) => Err(AppError::Validation(no_position_message.into())),
        (true, true) => Err(AppError::Validation(ambiguous_message.into())),
    }
}

pub(crate) fn position_side_from_okx_position(
    pos_side: &str,
    pos: f64,
) -> Option<CloseTargetPositionSide> {
    match pos_side.trim().to_ascii_lowercase().as_str() {
        "long" => Some(CloseTargetPositionSide::Long),
        "short" => Some(CloseTargetPositionSide::Short),
        _ if pos > 0.0 => Some(CloseTargetPositionSide::Long),
        _ if pos < 0.0 => Some(CloseTargetPositionSide::Short),
        _ => None,
    }
}

pub(crate) fn remaining_quantity(total: f64, filled: f64) -> f64 {
    if !total.is_finite() || total <= 0.0 {
        return 0.0;
    }
    if !filled.is_finite() || filled <= 0.0 {
        return total;
    }
    (total - filled).max(0.0)
}

pub(crate) fn planned_exit_residual_exchange_quantity(
    entry_filled: f64,
    exit_filled: f64,
) -> AppResult<f64> {
    if !entry_filled.is_finite() || entry_filled < 0.0 {
        return Err(AppError::Validation(format!(
            "计划退出入口成交数量无效: {entry_filled}"
        )));
    }
    if !exit_filled.is_finite() || exit_filled < 0.0 {
        return Err(AppError::Validation(format!(
            "计划退出已退出成交数量无效: {exit_filled}"
        )));
    }
    Ok(remaining_quantity(entry_filled, exit_filled))
}

pub(crate) fn planned_exit_reference_price(
    mid_px: Option<f64>,
    bid_px: Option<f64>,
    ask_px: Option<f64>,
    close_side: &str,
    entry_price: f64,
) -> Option<f64> {
    positive_price(mid_px)
        .or_else(|| match close_side.trim().to_ascii_lowercase().as_str() {
            "buy" => positive_price(ask_px),
            "sell" => positive_price(bid_px),
            _ => None,
        })
        .or_else(|| positive_price(Some(entry_price)))
}

fn positive_price(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite() && *value > 0.0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum CloseQuantityLimitDecision {
    UseAvailable,
    CapTo(f64),
    RejectAboveAvailable,
}

pub(crate) fn close_quantity_limit_decision(
    available_quantity: f64,
    requested_quantity: f64,
    reject_when_above_available: bool,
) -> AppResult<CloseQuantityLimitDecision> {
    if !available_quantity.is_finite() || available_quantity <= 0.0 {
        return Err(AppError::Validation(format!(
            "可平数量必须是有效正数: {available_quantity}"
        )));
    }
    if !requested_quantity.is_finite() || requested_quantity <= 0.0 {
        return Err(AppError::Validation(format!(
            "请求平仓数量必须是有效正数: {requested_quantity}"
        )));
    }
    if requested_quantity > available_quantity + f64::EPSILON {
        return Ok(if reject_when_above_available {
            CloseQuantityLimitDecision::RejectAboveAvailable
        } else {
            CloseQuantityLimitDecision::UseAvailable
        });
    }
    if requested_quantity + f64::EPSILON < available_quantity {
        return Ok(CloseQuantityLimitDecision::CapTo(requested_quantity));
    }
    Ok(CloseQuantityLimitDecision::UseAvailable)
}

pub(crate) fn validate_attached_protective_order_scope(
    risk_symbol: &str,
    risk_side: &str,
    entry_symbol: &str,
    entry_side: &str,
) -> AppResult<&'static str> {
    let expected_close_side = expected_protective_close_side(entry_side).ok_or_else(|| {
        AppError::Validation(format!("无法根据开仓方向 {entry_side} 推导保护单平仓方向"))
    })?;
    if !risk_symbol.trim().is_empty() && !risk_symbol.eq_ignore_ascii_case(entry_symbol) {
        return Err(AppError::Validation(format!(
            "保护单 symbol={} 与开仓 symbol={} 不一致，已拒绝裸开仓",
            risk_symbol, entry_symbol
        )));
    }
    let normalized_risk_side = risk_side.trim().to_ascii_lowercase();
    if !normalized_risk_side.is_empty() && normalized_risk_side != expected_close_side {
        return Err(AppError::Validation(format!(
            "保护单方向 {} 与开仓方向 {} 不匹配，期望保护平仓方向为 {}，已拒绝裸开仓",
            risk_side, entry_side, expected_close_side
        )));
    }
    Ok(expected_close_side)
}

pub(crate) fn validate_standalone_risk_order_symbol(
    risk_symbol: &str,
    execution_symbol: &str,
) -> AppResult<()> {
    if !risk_symbol.trim().is_empty() && !risk_symbol.eq_ignore_ascii_case(execution_symbol) {
        return Err(AppError::Validation(format!(
            "保护单 symbol={} 与执行 symbol={} 不一致，已拒绝提交",
            risk_symbol, execution_symbol
        )));
    }
    Ok(())
}

pub(crate) fn derived_protective_trigger_price(
    kind: ProtectiveOrderKind,
    entry_side: &str,
    entry_price: f64,
    stop_loss: Option<f64>,
    take_profit: Option<f64>,
) -> Option<f64> {
    if !entry_price.is_finite() || entry_price <= 0.0 {
        return None;
    }
    let ratio = match kind {
        ProtectiveOrderKind::StopLoss => stop_loss?,
        ProtectiveOrderKind::TakeProfit => take_profit?,
    };
    if !ratio.is_finite() || ratio <= 0.0 {
        return None;
    }
    match (kind, entry_side.trim().to_ascii_lowercase().as_str()) {
        (ProtectiveOrderKind::StopLoss, "buy" | "long") => Some(entry_price * (1.0 - ratio)),
        (ProtectiveOrderKind::StopLoss, "sell" | "short") => Some(entry_price * (1.0 + ratio)),
        (ProtectiveOrderKind::TakeProfit, "buy" | "long") => Some(entry_price * (1.0 + ratio)),
        (ProtectiveOrderKind::TakeProfit, "sell" | "short") => Some(entry_price * (1.0 - ratio)),
        _ => None,
    }
}

pub(crate) fn resolve_valid_protective_trigger_price(
    kind: ProtectiveOrderKind,
    entry_side: Option<&str>,
    entry_price: Option<f64>,
    explicit_trigger_price: Option<f64>,
    stop_loss: Option<f64>,
    take_profit: Option<f64>,
    missing_message: impl Into<String>,
) -> AppResult<f64> {
    let derived = match (entry_side, entry_price) {
        (Some(entry_side), Some(entry_price)) => {
            derived_protective_trigger_price(kind, entry_side, entry_price, stop_loss, take_profit)
        }
        _ => None,
    };
    explicit_trigger_price
        .or(derived)
        .filter(|price| price.is_finite() && *price > 0.0)
        .ok_or_else(|| AppError::Validation(missing_message.into()))
}

pub(crate) fn resolve_attached_protective_trigger_price(
    kind: ProtectiveOrderKind,
    entry_side: &str,
    entry_price: f64,
    explicit_trigger_price: Option<f64>,
    stop_loss: Option<f64>,
    take_profit: Option<f64>,
    reason: &str,
) -> AppResult<f64> {
    let trigger_price = resolve_valid_protective_trigger_price(
        kind,
        Some(entry_side),
        Some(entry_price),
        explicit_trigger_price,
        stop_loss,
        take_profit,
        format!("保护单缺少有效触发价，reason={reason}，已拒绝裸开仓"),
    )?;
    validate_protective_trigger_direction(kind, entry_side, entry_price, trigger_price, reason)?;
    Ok(trigger_price)
}

pub(crate) fn resolve_standalone_protective_trigger_price(
    kind: ProtectiveOrderKind,
    close_order_side: &str,
    reference_price: Option<f64>,
    explicit_trigger_price: Option<f64>,
    stop_loss: Option<f64>,
    take_profit: Option<f64>,
    reason: &str,
) -> AppResult<f64> {
    let entry_side = entry_side_for_protective_close_side(close_order_side);
    let trigger_price = resolve_valid_protective_trigger_price(
        kind,
        entry_side,
        reference_price,
        explicit_trigger_price,
        stop_loss,
        take_profit,
        format!("独立保护单缺少有效触发价，reason={reason}"),
    )?;
    if let (Some(reference_price), Some(entry_side)) = (reference_price, entry_side) {
        validate_protective_trigger_direction(
            kind,
            entry_side,
            reference_price,
            trigger_price,
            reason,
        )?;
    }
    Ok(trigger_price)
}

pub(crate) fn protective_trigger_direction_valid(
    kind: ProtectiveOrderKind,
    entry_side: &str,
    entry_price: f64,
    trigger_price: f64,
) -> bool {
    if !entry_price.is_finite()
        || entry_price <= 0.0
        || !trigger_price.is_finite()
        || trigger_price <= 0.0
    {
        return false;
    }
    match (kind, entry_side.trim().to_ascii_lowercase().as_str()) {
        (ProtectiveOrderKind::StopLoss, "buy" | "long") => trigger_price < entry_price,
        (ProtectiveOrderKind::StopLoss, "sell" | "short") => trigger_price > entry_price,
        (ProtectiveOrderKind::TakeProfit, "buy" | "long") => trigger_price > entry_price,
        (ProtectiveOrderKind::TakeProfit, "sell" | "short") => trigger_price < entry_price,
        _ => false,
    }
}

pub(crate) fn validate_protective_trigger_direction(
    kind: ProtectiveOrderKind,
    entry_side: &str,
    entry_price: f64,
    trigger_price: f64,
    reason: &str,
) -> AppResult<()> {
    if !entry_price.is_finite() || entry_price <= 0.0 {
        return Err(AppError::Validation(format!(
            "保护单无法校验开仓价，reason={reason}，已拒绝裸开仓"
        )));
    }
    if protective_trigger_direction_valid(kind, entry_side, entry_price, trigger_price) {
        return Ok(());
    }
    Err(AppError::Validation(format!(
        "保护单触发价方向无效: {} trigger_price={trigger_price:.8}, entry_side={entry_side}, entry_price={entry_price:.8}, reason={reason}，已拒绝裸开仓",
        kind.label()
    )))
}

pub(crate) fn symbol_currencies(symbol: &str) -> (String, String) {
    let mut parts = symbol
        .trim()
        .split('-')
        .filter(|part| !part.trim().is_empty())
        .map(|part| part.trim().to_ascii_uppercase());
    let base = parts.next().unwrap_or_default();
    let quote = parts.next().unwrap_or_default();
    (base, quote)
}

pub(crate) fn parse_positive_decimal_text(value: &str, field: &str) -> AppResult<f64> {
    let parsed = value
        .trim()
        .parse::<f64>()
        .map_err(|_| AppError::Validation(format!("OKX {field} 必须是有效正数")))?;
    if parsed.is_finite() && parsed > 0.0 {
        Ok(parsed)
    } else {
        Err(AppError::Validation(format!("OKX {field} 必须是有效正数")))
    }
}

pub(crate) fn validate_price_matches_tick_size(
    rules: &InstrumentTradeRules,
    price: f64,
    label: &str,
    consequence: &str,
) -> AppResult<()> {
    if !price.is_finite() || price <= 0.0 {
        return Err(AppError::Validation(format!(
            "OKX instrument {} {} 必须是有效正数",
            rules.inst_id, label
        )));
    }
    let tick_sz = rules.tick_sz.ok_or_else(|| {
        AppError::Validation(format!(
            "OKX instrument {} 缺少 tickSz，无法校验{}",
            rules.inst_id, label
        ))
    })?;
    if !is_aligned_to_step(price, tick_sz) {
        return Err(AppError::Validation(format!(
            "OKX instrument {} {} {:.12} 不符合 tickSz {:.12}，{}",
            rules.inst_id, label, price, tick_sz, consequence
        )));
    }
    Ok(())
}

pub(crate) fn validate_explicit_exchange_size(
    rules: &InstrumentTradeRules,
    size: f64,
    label: &str,
    consequence: &str,
) -> AppResult<()> {
    if !size.is_finite() || size <= 0.0 {
        return Err(AppError::Validation(format!(
            "OKX instrument {} {} 必须是有效正数",
            rules.inst_id, label
        )));
    }
    let lot_sz = rules.lot_sz.ok_or_else(|| {
        AppError::Validation(format!(
            "OKX instrument {} 缺少 lotSz，无法校验{}",
            rules.inst_id, label
        ))
    })?;
    if !is_aligned_to_step(size, lot_sz) {
        return Err(AppError::Validation(format!(
            "OKX instrument {} {} {:.12} 不符合 lotSz {:.12}，{}",
            rules.inst_id, label, size, lot_sz, consequence
        )));
    }
    if let Some(min_sz) = rules.min_sz {
        if size + f64::EPSILON < min_sz {
            return Err(AppError::Validation(format!(
                "OKX instrument {} {} {:.12} 小于 minSz {:.12}，{}",
                rules.inst_id, label, size, min_sz, consequence
            )));
        }
    }
    Ok(())
}

pub(crate) fn validate_modify_order_has_change(
    new_size: Option<&str>,
    new_price: Option<&str>,
    fields: ModifyOrderFieldSemantics,
) -> AppResult<()> {
    if new_size.is_none() && new_price.is_none() {
        return Err(AppError::Validation(fields.missing_message.to_string()));
    }
    Ok(())
}

pub(crate) fn validate_modify_order_size_field(
    rules: &InstrumentTradeRules,
    raw_new_size: &str,
    fields: ModifyOrderFieldSemantics,
) -> AppResult<f64> {
    let size = parse_positive_decimal_text(raw_new_size, fields.size_field)?;
    validate_explicit_exchange_size(rules, size, fields.size_label, fields.size_consequence)?;
    Ok(size)
}

pub(crate) fn validate_modify_order_price_field(
    rules: &InstrumentTradeRules,
    raw_new_price: &str,
    fields: ModifyOrderFieldSemantics,
) -> AppResult<f64> {
    let price = parse_positive_decimal_text(raw_new_price, fields.price_field)?;
    validate_price_matches_tick_size(rules, price, fields.price_label, fields.price_consequence)?;
    Ok(price)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_explicit_exchange_quantity(
    rules: &InstrumentTradeRules,
    inst_type: &str,
    symbol: &str,
    raw_exchange_size: &str,
    price: f64,
    size_label: &str,
    size_consequence: &str,
    invalid_exchange_message: &str,
    invalid_price_message: &str,
) -> AppResult<ResolvedExchangeQuantity> {
    let exchange_quantity = parse_positive_decimal_text(raw_exchange_size, "exchange_size")?;
    validate_explicit_exchange_size(rules, exchange_quantity, size_label, size_consequence)?;
    let base_quantity = base_quantity_from_exchange_quantity(
        inst_type,
        symbol,
        rules,
        exchange_quantity,
        price,
        invalid_exchange_message,
        invalid_price_message,
    )?;
    Ok(ResolvedExchangeQuantity {
        exchange_quantity,
        base_quantity,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_exchange_quantity_from_base_order_quantity(
    rules: &InstrumentTradeRules,
    inst_type: &str,
    symbol: &str,
    base_quantity: f64,
    price: f64,
    invalid_base_message: &str,
    invalid_price_message: &str,
    invalid_exchange_message: &str,
) -> AppResult<ResolvedExchangeQuantity> {
    let raw_exchange_quantity = exchange_quantity_from_base_quantity(
        inst_type,
        symbol,
        rules,
        base_quantity,
        price,
        invalid_base_message,
        invalid_price_message,
    )?;
    let exchange_quantity = apply_instrument_size_rules(rules, raw_exchange_quantity)?;
    let base_quantity = base_quantity_from_exchange_quantity(
        inst_type,
        symbol,
        rules,
        exchange_quantity,
        price,
        invalid_exchange_message,
        invalid_price_message,
    )?;
    Ok(ResolvedExchangeQuantity {
        exchange_quantity,
        base_quantity,
    })
}

pub(crate) fn exchange_quantity_from_base_quantity(
    inst_type: &str,
    symbol: &str,
    rules: &InstrumentTradeRules,
    base_quantity: f64,
    price: f64,
    invalid_base_message: &str,
    invalid_price_message: &str,
) -> AppResult<f64> {
    if !base_quantity.is_finite() || base_quantity <= 0.0 {
        return Err(AppError::Validation(invalid_base_message.to_string()));
    }
    if !is_contract_inst_type(inst_type) {
        return Ok(base_quantity);
    }
    contract_count_from_base_quantity(symbol, rules, base_quantity, price, invalid_price_message)
}

pub(crate) fn contract_count_from_base_quantity(
    symbol: &str,
    rules: &InstrumentTradeRules,
    base_quantity: f64,
    price: f64,
    invalid_price_message: &str,
) -> AppResult<f64> {
    let ct_val = rules.ct_val.ok_or_else(|| {
        AppError::Validation(format!(
            "OKX instrument {} 缺少 ctVal，无法把策略数量换算为合约张数",
            rules.inst_id
        ))
    })?;
    let (base_ccy, quote_ccy) = symbol_currencies(symbol);
    if rules.ct_val_ccy.eq_ignore_ascii_case(&base_ccy) {
        return Ok(base_quantity / ct_val);
    }
    if rules.ct_val_ccy.eq_ignore_ascii_case(&quote_ccy) {
        if !price.is_finite() || price <= 0.0 {
            return Err(AppError::Validation(invalid_price_message.to_string()));
        }
        return Ok(base_quantity * price / ct_val);
    }
    Err(AppError::Validation(format!(
        "OKX instrument {} 的 ctValCcy={} 无法和 symbol={} 对齐，已拒绝交易下单",
        rules.inst_id, rules.ct_val_ccy, symbol
    )))
}

pub(crate) fn base_quantity_from_exchange_quantity(
    inst_type: &str,
    symbol: &str,
    rules: &InstrumentTradeRules,
    exchange_quantity: f64,
    price: f64,
    invalid_exchange_message: &str,
    invalid_price_message: &str,
) -> AppResult<f64> {
    if !exchange_quantity.is_finite() || exchange_quantity <= 0.0 {
        return Err(AppError::Validation(invalid_exchange_message.to_string()));
    }
    if !is_contract_inst_type(inst_type) {
        return Ok(exchange_quantity);
    }
    base_quantity_from_contract_count(
        symbol,
        rules,
        exchange_quantity,
        price,
        invalid_price_message,
    )
}

pub(crate) fn base_quantity_from_contract_count(
    symbol: &str,
    rules: &InstrumentTradeRules,
    exchange_quantity: f64,
    price: f64,
    invalid_price_message: &str,
) -> AppResult<f64> {
    let ct_val = rules.ct_val.ok_or_else(|| {
        AppError::Validation(format!(
            "OKX instrument {} 缺少 ctVal，无法估算显式 exchange_size 的风险敞口",
            rules.inst_id
        ))
    })?;
    let (base_ccy, quote_ccy) = symbol_currencies(symbol);
    if rules.ct_val_ccy.eq_ignore_ascii_case(&base_ccy) {
        return Ok(exchange_quantity * ct_val);
    }
    if rules.ct_val_ccy.eq_ignore_ascii_case(&quote_ccy) {
        if !price.is_finite() || price <= 0.0 {
            return Err(AppError::Validation(invalid_price_message.to_string()));
        }
        return Ok(exchange_quantity * ct_val / price);
    }
    Err(AppError::Validation(format!(
        "OKX instrument {} 的 ctValCcy={} 无法和 symbol={} 对齐，已拒绝交易下单",
        rules.inst_id, rules.ct_val_ccy, symbol
    )))
}

pub(crate) fn exchange_quantity_value(
    inst_type: &str,
    symbol: &str,
    rules: &InstrumentTradeRules,
    exchange_quantity: f64,
    price: f64,
) -> AppResult<f64> {
    if !exchange_quantity.is_finite() || exchange_quantity <= 0.0 {
        return Err(AppError::Validation(
            "OKX 下单数量必须是有效正数".to_string(),
        ));
    }
    if !price.is_finite() || price <= 0.0 {
        return Err(AppError::Validation(
            "价格无效，无法计算成交名义价值".to_string(),
        ));
    }
    if !is_contract_inst_type(inst_type) {
        return Ok(exchange_quantity * price);
    }
    let ct_val = rules.ct_val.ok_or_else(|| {
        AppError::Validation(format!(
            "OKX instrument {} 缺少 ctVal，无法计算合约名义价值",
            rules.inst_id
        ))
    })?;
    let (base_ccy, quote_ccy) = symbol_currencies(symbol);
    if rules.ct_val_ccy.eq_ignore_ascii_case(&base_ccy) {
        return Ok(exchange_quantity * ct_val * price);
    }
    if rules.ct_val_ccy.eq_ignore_ascii_case(&quote_ccy) {
        return Ok(exchange_quantity * ct_val);
    }
    Err(AppError::Validation(format!(
        "OKX instrument {} 的 ctValCcy={} 无法和 symbol={} 对齐，已拒绝交易下单",
        rules.inst_id, rules.ct_val_ccy, symbol
    )))
}

pub(crate) fn apply_instrument_size_rules(
    rules: &InstrumentTradeRules,
    raw_quantity: f64,
) -> AppResult<f64> {
    if !raw_quantity.is_finite() || raw_quantity <= 0.0 {
        return Err(AppError::Validation(format!(
            "OKX instrument {} 换算后的下单数量无效",
            rules.inst_id
        )));
    }
    let quantity = round_down_to_step(raw_quantity, rules.lot_sz.unwrap_or(0.0));
    let min_sz = rules.min_sz.unwrap_or(0.0);
    if min_sz > 0.0 && quantity + f64::EPSILON < min_sz {
        return Err(AppError::Validation(format!(
            "OKX instrument {} 换算数量 {:.8} 小于最小下单数量 {:.8}",
            rules.inst_id, quantity, min_sz
        )));
    }
    if quantity <= 0.0 {
        return Err(AppError::Validation(format!(
            "OKX instrument {} 按 lotSz 取整后数量为 0",
            rules.inst_id
        )));
    }
    Ok(quantity)
}

pub(crate) fn is_aligned_to_step(value: f64, step: f64) -> bool {
    if !value.is_finite() || !step.is_finite() || value <= 0.0 || step <= 0.0 {
        return false;
    }
    let units = value / step;
    let nearest = units.round();
    (units - nearest).abs() <= 1e-8_f64.max(nearest.abs() * 1e-12)
}

pub(crate) fn round_down_to_step(value: f64, step: f64) -> f64 {
    if !value.is_finite() || value <= 0.0 {
        return 0.0;
    }
    if !step.is_finite() || step <= 0.0 {
        return value;
    }
    let units = (value / step + 1e-12).floor();
    let rounded = units * step;
    if rounded.is_finite() && rounded > 0.0 {
        rounded
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_order_type_text_normalizes_aliases() {
        assert_eq!(
            normalize_runtime_order_type_text(" post-only "),
            "post_only"
        );
        assert_eq!(
            normalize_runtime_order_type_text("MMP-AND-POST-ONLY"),
            "mmp_and_post_only"
        );
        assert_eq!(normalize_runtime_order_type_text(""), "market");
    }

    #[test]
    fn runtime_order_can_rest_requires_resting_status_and_type() {
        assert!(runtime_order_can_rest("open", "limit"));
        assert!(runtime_order_can_rest("partially-filled", "post-only"));
        assert!(runtime_order_can_rest(
            "modify_requested",
            "mmp_and_post_only"
        ));
        assert!(!runtime_order_can_rest("filled", "limit"));
        assert!(!runtime_order_can_rest("open", "market"));
        assert!(!runtime_order_can_rest("partially_filled", "ioc"));
    }

    #[test]
    fn order_type_attributes_normalize_runtime_aliases() {
        assert!(order_type_omits_price("market"));
        assert!(order_type_omits_price("optimal-limit-ioc"));
        assert!(!order_type_omits_price("limit"));
        assert!(immediate_remainder_order_type("ioc"));
        assert!(immediate_remainder_order_type("fok"));
        assert!(!immediate_remainder_order_type("post_only"));
        assert!(fill_or_kill_order_type("FOK"));
        assert!(!fill_or_kill_order_type("ioc"));
        assert!(maker_only_order_type("post-only"));
        assert!(maker_only_order_type("mmp-and-post-only"));
        assert!(!maker_only_order_type("limit"));
    }

    #[test]
    fn exchange_order_type_requires_price_normalizes_and_rejects_unknown() {
        assert_eq!(
            normalized_exchange_order_type("post-only").unwrap(),
            "post_only"
        );
        assert!(exchange_order_type_requires_price("post-only").unwrap());
        assert!(exchange_order_type_requires_price("ioc").unwrap());
        assert!(!exchange_order_type_requires_price("market").unwrap());
        assert!(!exchange_order_type_requires_price("optimal-limit-ioc").unwrap());
        assert!(exchange_order_type_requires_price("unknown").is_err());
        assert!(!exchange_order_type_requires_price_safe("unknown"));
    }

    #[test]
    fn exchange_order_price_shape_validates_required_price_and_tick() {
        let rules = InstrumentTradeRules {
            inst_id: "BTC-USDT-SWAP".to_string(),
            min_sz: Some(0.01),
            lot_sz: Some(0.01),
            tick_sz: Some(0.1),
            ct_val: Some(0.01),
            ct_val_ccy: "BTC".to_string(),
        };

        assert!(validate_exchange_order_price_shape(
            &rules,
            "market",
            "buy",
            f64::NAN,
            |order_type| format!("OKX {order_type} 订单需要有效价格"),
            "已拒绝测试下单以避免静默改价",
        )
        .is_ok());
        assert!(validate_exchange_order_price_shape(
            &rules,
            "limit",
            "buy",
            0.0,
            |order_type| format!("OKX {order_type} 订单需要有效价格"),
            "已拒绝测试下单以避免静默改价",
        )
        .unwrap_err()
        .to_string()
        .contains("需要有效价格"));
        assert!(validate_exchange_order_price_shape(
            &rules,
            "limit",
            "buy",
            100.05,
            |order_type| format!("OKX {order_type} 订单需要有效价格"),
            "已拒绝测试下单以避免静默改价",
        )
        .unwrap_err()
        .to_string()
        .contains("tickSz"));
        validate_exchange_order_price_shape(
            &rules,
            "limit",
            "buy",
            100.1,
            |order_type| format!("OKX {order_type} 订单需要有效价格"),
            "已拒绝测试下单以避免静默改价",
        )
        .unwrap();
    }

    #[test]
    fn modify_order_fields_validate_shared_size_and_price_rules() {
        let rules = InstrumentTradeRules {
            inst_id: "BTC-USDT-SWAP".to_string(),
            min_sz: Some(0.01),
            lot_sz: Some(0.01),
            tick_sz: Some(0.1),
            ct_val: Some(0.01),
            ct_val_ccy: "BTC".to_string(),
        };

        assert!(
            validate_modify_order_has_change(None, None, EXCHANGE_MODIFY_ORDER_FIELDS)
                .unwrap_err()
                .to_string()
                .contains("newSz/newPx")
        );
        validate_modify_order_has_change(Some("1"), None, EXCHANGE_MODIFY_ORDER_FIELDS).unwrap();
        assert_eq!(
            validate_modify_order_size_field(&rules, "1.25", EXCHANGE_MODIFY_ORDER_FIELDS).unwrap(),
            1.25
        );
        assert!(
            validate_modify_order_size_field(&rules, "1.255", EXCHANGE_MODIFY_ORDER_FIELDS)
                .unwrap_err()
                .to_string()
                .contains("lotSz")
        );
        assert_eq!(
            validate_modify_order_price_field(&rules, "100.2", ALGO_MODIFY_ORDER_FIELDS).unwrap(),
            100.2
        );
        assert!(
            validate_modify_order_price_field(&rules, "100.25", ALGO_MODIFY_ORDER_FIELDS)
                .unwrap_err()
                .to_string()
                .contains("保护单改单触发价")
        );
        assert!(
            validate_modify_order_price_field(&rules, "bad", ALGO_MODIFY_ORDER_FIELDS)
                .unwrap_err()
                .to_string()
                .contains("newTriggerPx")
        );
    }

    #[test]
    fn explicit_exchange_quantity_resolves_size_and_base_quantity() {
        let rules = InstrumentTradeRules {
            inst_id: "BTC-USDT-SWAP".to_string(),
            min_sz: Some(0.01),
            lot_sz: Some(0.01),
            tick_sz: Some(0.1),
            ct_val: Some(10.0),
            ct_val_ccy: "USDT".to_string(),
        };

        let resolved = resolve_explicit_exchange_quantity(
            &rules,
            "SWAP",
            "BTC-USDT-SWAP",
            "2.5",
            100.0,
            "策略显式 exchange_size",
            "已拒绝测试下单以避免静默改量",
            "测试 exchange_size 必须是有效正数",
            "测试价格无效，无法估算显式 exchange_size 的风险敞口",
        )
        .unwrap();
        assert_eq!(resolved.exchange_quantity, 2.5);
        assert_eq!(resolved.base_quantity, 0.25);

        assert!(resolve_explicit_exchange_quantity(
            &rules,
            "SWAP",
            "BTC-USDT-SWAP",
            "2.505",
            100.0,
            "策略显式 exchange_size",
            "已拒绝测试下单以避免静默改量",
            "测试 exchange_size 必须是有效正数",
            "测试价格无效，无法估算显式 exchange_size 的风险敞口",
        )
        .unwrap_err()
        .to_string()
        .contains("lotSz"));
        assert!(resolve_explicit_exchange_quantity(
            &rules,
            "SWAP",
            "BTC-USDT-SWAP",
            "2.5",
            0.0,
            "策略显式 exchange_size",
            "已拒绝测试下单以避免静默改量",
            "测试 exchange_size 必须是有效正数",
            "测试价格无效，无法估算显式 exchange_size 的风险敞口",
        )
        .unwrap_err()
        .to_string()
        .contains("测试价格无效"));
    }

    #[test]
    fn base_order_quantity_resolves_actual_base_after_lot_rounding() {
        let rules = InstrumentTradeRules {
            inst_id: "BTC-USDT-SWAP".to_string(),
            min_sz: Some(0.01),
            lot_sz: Some(0.01),
            tick_sz: Some(0.1),
            ct_val: Some(0.01),
            ct_val_ccy: "BTC".to_string(),
        };

        let resolved = resolve_exchange_quantity_from_base_order_quantity(
            &rules,
            "SWAP",
            "BTC-USDT-SWAP",
            0.12345,
            100.0,
            "测试基础币数量必须是有效正数",
            "测试价格无效，无法把基础币数量换算为 OKX 合约张数",
            "测试 OKX 下单数量必须是有效正数",
        )
        .unwrap();
        assert_eq!(resolved.exchange_quantity, 12.34);
        assert_eq!(resolved.base_quantity, 0.1234);
    }

    #[test]
    fn max_slippage_and_adverse_slippage_are_shared_runtime_semantics() {
        assert_eq!(
            max_slippage_from_params(&serde_json::json!({"max_slippage_bps": 25.0})),
            Some(0.0025)
        );
        assert_eq!(
            max_slippage_from_params(&serde_json::json!({"max_slippage_pct": 0.01})),
            Some(0.01)
        );
        assert_eq!(
            max_slippage_from_params(&serde_json::json!({"_runtime_max_slippage": 2.0})),
            Some(1.0)
        );
        assert_eq!(max_slippage_from_params(&serde_json::json!({})), None);

        assert!(check_max_adverse_slippage(Some(0.01), "buy", 100.0, Some(99.0), Some(100.5)).0);
        assert!(!check_max_adverse_slippage(Some(0.01), "buy", 100.0, Some(99.0), Some(102.0)).0);
        assert!(!check_max_adverse_slippage(Some(0.01), "sell", 100.0, Some(98.0), Some(101.0)).0);
        assert!(!check_max_adverse_slippage(Some(0.01), "hold", 100.0, Some(99.0), Some(101.0)).0);
        assert!(!check_max_adverse_slippage(Some(0.01), "buy", 100.0, None, None).0);
        assert!(check_max_adverse_slippage(None, "hold", f64::NAN, None, None).0);
    }

    #[test]
    fn protective_order_kind_accepts_hyphen_aliases() {
        assert_eq!(
            protective_order_kind_from_order_type("stop-market"),
            Some(ProtectiveOrderKind::StopLoss)
        );
        assert_eq!(
            protective_order_kind_from_order_type("take-profit"),
            Some(ProtectiveOrderKind::TakeProfit)
        );
    }

    #[test]
    fn standalone_risk_order_preflight_is_shared_runtime_semantics() {
        let orders = [1_u8];
        assert_eq!(select_single_standalone_risk_order(&orders).copied(), Ok(1));
        assert_eq!(
            select_single_standalone_risk_order::<u8>(&[]).unwrap_err(),
            StandaloneRiskOrderSelectionError::Missing
        );
        assert_eq!(
            select_single_standalone_risk_order(&[1_u8, 2]).unwrap_err(),
            StandaloneRiskOrderSelectionError::Multiple { count: 2 }
        );

        assert_eq!(standalone_risk_order_type(""), "place_risk_order");
        assert_eq!(standalone_risk_order_type("stop-market"), "stop_market");

        assert!(validate_standalone_risk_order_symbol("", "BTC-USDT-SWAP").is_ok());
        assert!(validate_standalone_risk_order_symbol("btc-usdt-swap", "BTC-USDT-SWAP").is_ok());
        assert!(
            validate_standalone_risk_order_symbol("ETH-USDT-SWAP", "BTC-USDT-SWAP")
                .unwrap_err()
                .to_string()
                .contains("与执行 symbol")
        );
    }

    #[test]
    fn standalone_protective_trigger_price_uses_close_side_and_reference_price() {
        assert_eq!(
            resolve_attached_protective_trigger_price(
                ProtectiveOrderKind::TakeProfit,
                "long",
                100.0,
                None,
                None,
                Some(0.08),
                "unit_attached_take_profit"
            )
            .unwrap(),
            108.0
        );
        assert!(resolve_attached_protective_trigger_price(
            ProtectiveOrderKind::TakeProfit,
            "long",
            100.0,
            Some(95.0),
            None,
            None,
            "unit_attached_wrong_direction"
        )
        .unwrap_err()
        .to_string()
        .contains("触发价方向无效"));

        assert_eq!(
            resolve_standalone_protective_trigger_price(
                ProtectiveOrderKind::StopLoss,
                "sell",
                Some(100.0),
                None,
                Some(0.05),
                None,
                "unit_stop"
            )
            .unwrap(),
            95.0
        );
        assert_eq!(
            resolve_standalone_protective_trigger_price(
                ProtectiveOrderKind::StopLoss,
                "sell",
                None,
                Some(94.0),
                Some(0.05),
                None,
                "unit_explicit"
            )
            .unwrap(),
            94.0
        );
        assert!(resolve_standalone_protective_trigger_price(
            ProtectiveOrderKind::StopLoss,
            "sell",
            None,
            None,
            Some(0.05),
            None,
            "unit_missing"
        )
        .unwrap_err()
        .to_string()
        .contains("独立保护单缺少有效触发价"));
        assert!(resolve_standalone_protective_trigger_price(
            ProtectiveOrderKind::StopLoss,
            "sell",
            Some(100.0),
            Some(105.0),
            None,
            None,
            "unit_wrong_direction"
        )
        .unwrap_err()
        .to_string()
        .contains("触发价方向无效"));
    }

    #[test]
    fn action_side_mappings_are_shared_runtime_semantics() {
        assert_eq!(action_position_side(" buy "), Some(("long", 1)));
        assert_eq!(action_position_side("LONG"), Some(("long", 1)));
        assert_eq!(action_position_side("sell"), Some(("short", -1)));
        assert_eq!(action_position_side("short"), Some(("short", -1)));
        assert_eq!(action_position_side("hold"), None);

        assert_eq!(entry_order_side_for_action_side("long"), Some("buy"));
        assert_eq!(entry_order_side_for_action_side("short"), Some("sell"));
        assert_eq!(entry_order_side_for_action_side("hold"), None);

        assert_eq!(close_order_side_from_position_text("long"), Some("sell"));
        assert_eq!(close_order_side_from_position_text("short"), Some("buy"));
        assert_eq!(close_order_side_from_position_text("sell"), Some("sell"));
        assert_eq!(close_order_side_from_position_text("hold"), None);
    }

    #[test]
    fn close_target_position_side_maps_order_side_to_position_side() {
        assert_eq!(
            close_target_position_side(" sell "),
            Some(CloseTargetPositionSide::Long)
        );
        assert_eq!(
            close_target_position_side("BUY"),
            Some(CloseTargetPositionSide::Short)
        );
        assert_eq!(close_target_position_side("hold"), None);
        assert_eq!(close_target_position_side_label("sell"), "多头");
        assert_eq!(close_target_position_side_label("hold"), "目标方向");
        assert_eq!(
            close_order_side_for_target_position_side(CloseTargetPositionSide::Long),
            "sell"
        );
        assert_eq!(
            close_order_side_for_target_position_side(CloseTargetPositionSide::Short),
            "buy"
        );
    }

    #[test]
    fn single_close_target_inference_rejects_empty_or_ambiguous_positions() {
        assert_eq!(
            infer_single_close_target_position_side(true, false, "none", "ambiguous").unwrap(),
            CloseTargetPositionSide::Long
        );
        assert_eq!(
            infer_single_close_target_position_side(false, true, "none", "ambiguous").unwrap(),
            CloseTargetPositionSide::Short
        );
        assert!(
            infer_single_close_target_position_side(false, false, "none", "ambiguous")
                .unwrap_err()
                .to_string()
                .contains("none")
        );
        assert!(
            infer_single_close_target_position_side(true, true, "none", "ambiguous")
                .unwrap_err()
                .to_string()
                .contains("ambiguous")
        );
    }

    #[test]
    fn okx_position_side_falls_back_to_signed_position() {
        assert_eq!(
            position_side_from_okx_position("long", -1.0),
            Some(CloseTargetPositionSide::Long)
        );
        assert_eq!(
            position_side_from_okx_position("", 2.0),
            Some(CloseTargetPositionSide::Long)
        );
        assert_eq!(
            position_side_from_okx_position("", -2.0),
            Some(CloseTargetPositionSide::Short)
        );
        assert_eq!(position_side_from_okx_position("", 0.0), None);
    }

    #[test]
    fn planned_exit_residual_rejects_invalid_quantities() {
        assert_eq!(
            planned_exit_residual_exchange_quantity(2.0, 0.75).unwrap(),
            1.25
        );
        assert_eq!(
            planned_exit_residual_exchange_quantity(0.75, 2.0).unwrap(),
            0.0
        );
        assert!(planned_exit_residual_exchange_quantity(f64::NAN, 0.0).is_err());
        assert!(planned_exit_residual_exchange_quantity(1.0, -1.0).is_err());
    }

    #[test]
    fn planned_exit_reference_price_prefers_market_quote_then_entry() {
        assert_eq!(
            planned_exit_reference_price(Some(100.0), Some(99.5), Some(100.5), "buy", 90.0),
            Some(100.0)
        );
        assert_eq!(
            planned_exit_reference_price(None, Some(99.5), Some(100.5), "buy", 90.0),
            Some(100.5)
        );
        assert_eq!(
            planned_exit_reference_price(None, Some(99.5), Some(100.5), "sell", 90.0),
            Some(99.5)
        );
        assert_eq!(
            planned_exit_reference_price(None, None, None, "sell", 90.0),
            Some(90.0)
        );
        assert_eq!(
            planned_exit_reference_price(None, None, None, "sell", 0.0),
            None
        );
    }

    #[test]
    fn close_quantity_limit_decision_caps_or_rejects_by_policy() {
        assert_eq!(
            close_quantity_limit_decision(2.0, 1.25, true).unwrap(),
            CloseQuantityLimitDecision::CapTo(1.25)
        );
        assert_eq!(
            close_quantity_limit_decision(2.0, 2.0, true).unwrap(),
            CloseQuantityLimitDecision::UseAvailable
        );
        assert_eq!(
            close_quantity_limit_decision(2.0, 3.0, true).unwrap(),
            CloseQuantityLimitDecision::RejectAboveAvailable
        );
        assert_eq!(
            close_quantity_limit_decision(2.0, 3.0, false).unwrap(),
            CloseQuantityLimitDecision::UseAvailable
        );
        assert!(close_quantity_limit_decision(0.0, 1.0, true).is_err());
        assert!(close_quantity_limit_decision(1.0, f64::NAN, true).is_err());
    }

    #[test]
    fn funding_rate_cost_applies_to_perpetual_swaps_only() {
        assert!(has_periodic_funding_rate("SWAP"));
        assert!(has_periodic_funding_rate(" swap "));
        assert!(!has_periodic_funding_rate("FUTURES"));
        assert!(!has_periodic_funding_rate("SPOT"));
    }
}
