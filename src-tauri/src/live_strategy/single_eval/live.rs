use sqlx::SqlitePool;

use serde_json::Value;

use crate::{
    error::{AppError, AppResult},
    okx::{OkxAttachedAlgoOrder, OkxPrivateClient, OkxPublicClient},
    risk_controls,
    strategy_engine::StrategyActionRecord,
    trading_semantics::{
        exchange_order_type_requires_price, expected_protective_close_side, is_contract_inst_type,
        live_algo_target_order_kind_from_order_type, live_configured_leverage_from_params,
        live_td_mode_from_params, normalize_runtime_order_type_text,
        normalized_exchange_order_type, order_management_any_target_kind_collision_reason,
        order_type_omits_price, planned_exit_entry_remainder_cancel_needed,
        planned_exit_reference_price as shared_planned_exit_reference_price,
        protective_order_kind_from_order_type, resolve_attached_protective_trigger_price,
        resolve_protective_order_kind, resolve_standalone_protective_trigger_price,
        validate_attached_protective_order_scope, ProtectiveOrderKind,
    },
};

use super::super::{
    arrival::{check_slippage_control, fetch_arrival_quote},
    decision::{
        StrategyCancelOrderIntent, StrategyIntentAction, StrategyModifyOrderIntent,
        StrategyOrderTargetKind, StrategyPlannedExitIntent, StrategyRiskOrderIntent,
    },
    runtime_helpers::{action_position_side, entry_order_side, order_quantity},
    storage::{
        claim_due_live_planned_exit, insert_live_order, insert_live_planned_exit_plan,
        live_strategy_client_order_id, mark_live_planned_exit_retry,
        mark_live_planned_exit_skipped, mark_live_planned_exit_submitted,
        mark_live_planned_exit_submitting, query_due_live_planned_exits,
        query_live_algo_order_identity_context, query_live_algo_order_identity_context_for_symbol,
        query_live_order_identity_context, query_live_order_identity_context_for_symbol,
        update_live_algo_order_exchange_state_by_identity_and_symbol,
        update_live_exchange_order_state_by_identity_and_symbol, LiveAlgoOrderIdentityContext,
        LiveOrderExchangeState, LiveOrderIdentityContext, LivePlannedExitPlan,
    },
    types::LiveStrategyConfig,
    LiveStrategyRuntime,
};
use super::status::apply_blocked_order_status;

#[cfg(test)]
use super::super::storage::insert_live_exchange_order;

mod algo_order_management;
mod close_intents;
mod close_quantity;
mod evaluate;
mod json_values;
mod order_management;
mod order_records;
mod order_rules;
mod order_submission;
mod planned_exit_scope;
mod planned_exits;
mod risk_order_intents;

use self::close_quantity::*;
use self::json_values::*;
use self::order_rules::*;
use self::planned_exit_scope::*;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
struct SubmittedExchangeOrder {
    order_id: String,
    client_order_id: String,
}

#[derive(Clone, Debug)]
enum ExchangeSubmitOutcome {
    Submitted(SubmittedExchangeOrder),
    SubmittedUnknown(SubmittedExchangeOrder),
    NotSubmitted { reason: String, retryable: bool },
}

impl ExchangeSubmitOutcome {
    fn terminal(reason: impl Into<String>) -> Self {
        Self::NotSubmitted {
            reason: reason.into(),
            retryable: false,
        }
    }

    fn retryable(reason: impl Into<String>) -> Self {
        Self::NotSubmitted {
            reason: reason.into(),
            retryable: true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ResolvedEntryOrderQuantities {
    base_quantity: f64,
    exchange_quantity: f64,
    explicit_exchange_size: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum LiveActionExecutionOutcome {
    Submitted,
    NotSubmitted { reason: String, retryable: bool },
    Skipped,
}

impl LiveActionExecutionOutcome {
    fn terminal(reason: impl Into<String>) -> Self {
        Self::NotSubmitted {
            reason: reason.into(),
            retryable: false,
        }
    }

    fn retryable(reason: impl Into<String>) -> Self {
        Self::NotSubmitted {
            reason: reason.into(),
            retryable: true,
        }
    }

    pub(super) fn should_release_action_dedupe_key(&self) -> bool {
        matches!(
            self,
            Self::NotSubmitted {
                retryable: true,
                ..
            }
        )
    }
}

fn live_action_execution_outcome(outcome: ExchangeSubmitOutcome) -> LiveActionExecutionOutcome {
    match outcome {
        ExchangeSubmitOutcome::Submitted(_) | ExchangeSubmitOutcome::SubmittedUnknown(_) => {
            LiveActionExecutionOutcome::Submitted
        }
        ExchangeSubmitOutcome::NotSubmitted { reason, retryable } => {
            LiveActionExecutionOutcome::NotSubmitted { reason, retryable }
        }
    }
}

fn retryable_for_app_error(error: &AppError) -> bool {
    match error {
        AppError::Io(_) | AppError::Json(_) | AppError::Database(_) => true,
        AppError::Validation(_) => false,
        AppError::Runtime(message) => !message
            .to_ascii_lowercase()
            .contains("okx private api error"),
    }
}

fn retryable_for_close_resolution_error(error: &AppError, reason: &str) -> bool {
    retryable_for_app_error(error) || is_no_close_position_reason(reason)
}

fn config_for_planned_exit(
    base_config: &LiveStrategyConfig,
    plan: &LivePlannedExitPlan,
) -> LiveStrategyConfig {
    let mut config = base_config.clone();
    if !plan.strategy_id.trim().is_empty() {
        config.strategy_id = plan.strategy_id.clone();
    }
    if !plan.strategy_name.trim().is_empty() {
        config.strategy_name = plan.strategy_name.clone();
    }
    if !plan.mode.trim().is_empty() {
        config.mode = plan.mode.clone();
    }
    if !plan.symbol.trim().is_empty() {
        config.symbol = plan.symbol.clone();
    }
    if !plan.inst_type.trim().is_empty() {
        config.inst_type = plan.inst_type.clone();
    }
    if !plan.timeframe.trim().is_empty() {
        config.timeframe = plan.timeframe.clone();
    }
    config
}

fn config_for_algo_order(
    base_config: &LiveStrategyConfig,
    algo_order: &LiveAlgoOrderIdentityContext,
) -> LiveStrategyConfig {
    let mut config = base_config.clone();
    if !algo_order.symbol.trim().is_empty() {
        config.symbol = algo_order.symbol.clone();
    }
    config.inst_type = algo_order.inst_type.clone();
    config
}

fn config_for_live_order(
    base_config: &LiveStrategyConfig,
    order: &LiveOrderIdentityContext,
) -> LiveStrategyConfig {
    let mut config = base_config.clone();
    if !order.symbol.trim().is_empty() {
        config.symbol = order.symbol.clone();
    }
    config.inst_type = order.inst_type.clone();
    config
}

fn planned_exit_retry_delay_ms(config: &LiveStrategyConfig, attempt_count: i64) -> i64 {
    let base_ms = i64::try_from(config.check_interval.max(15))
        .unwrap_or(60)
        .saturating_mul(1_000);
    let multiplier = 2_i64.pow(attempt_count.clamp(0, 3) as u32);
    base_ms.saturating_mul(multiplier).min(300_000)
}

fn is_no_close_position_reason(reason: &str) -> bool {
    let reason = reason.trim();
    reason.contains("当前没有")
        && (reason.contains("可平") || reason.contains("可卖出") || reason.contains("可卖出的"))
}

fn td_mode_from_config(config: &LiveStrategyConfig) -> AppResult<String> {
    live_td_mode_from_params(&config.params, &config.inst_type)
}

fn explicit_configured_leverage(config: &LiveStrategyConfig) -> AppResult<Option<f64>> {
    live_configured_leverage_from_params(&config.params, &config.inst_type)
}

#[derive(Clone, Debug)]
struct ExchangeOrderContext {
    pos_side: String,
    reduce_only: bool,
}

async fn resolve_exchange_order_context(
    private_client: &OkxPrivateClient,
    config: &LiveStrategyConfig,
    order_side: &str,
    reduce_only: bool,
) -> AppResult<ExchangeOrderContext> {
    if !is_contract_inst_type(&config.inst_type) {
        return Ok(ExchangeOrderContext {
            pos_side: String::new(),
            reduce_only: false,
        });
    }
    let account_config = private_client.get_account_config().await?;
    let pos_mode = account_config
        .get("posMode")
        .or_else(|| account_config.get("pos_mode"))
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    if pos_mode == "net_mode" {
        return Ok(ExchangeOrderContext {
            pos_side: String::new(),
            reduce_only,
        });
    }
    if pos_mode == "long_short_mode" {
        let pos_side = match (order_side.trim().to_ascii_lowercase().as_str(), reduce_only) {
            ("buy", false) | ("sell", true) => "long",
            ("sell", false) | ("buy", true) => "short",
            _ => {
                return Err(AppError::Validation(
                    "无法根据策略动作 side 推导 OKX 双向持仓 posSide".to_string(),
                ));
            }
        };
        return Ok(ExchangeOrderContext {
            pos_side: pos_side.to_string(),
            reduce_only: false,
        });
    }
    Err(AppError::Validation(
        "无法从 OKX account config 读取 posMode，已拒绝实时策略合约下单".to_string(),
    ))
}

async fn resolve_entry_order_quantities(
    client: &OkxPublicClient,
    config: &LiveStrategyConfig,
    action_record: &StrategyActionRecord,
    exchange_size: Option<&str>,
) -> AppResult<ResolvedEntryOrderQuantities> {
    let price = action_record.price;
    if !price.is_finite() || price <= 0.0 {
        return Err(AppError::Validation("实时策略开仓价格无效".to_string()));
    }
    if let Some(exchange_size) = exchange_size
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let rules = fetch_instrument_order_rules(client, config).await?;
        let quantity =
            resolve_explicit_entry_exchange_quantity(config, &rules, exchange_size, price)?;
        return Ok(ResolvedEntryOrderQuantities {
            base_quantity: quantity.base_quantity,
            exchange_quantity: quantity.exchange_quantity,
            explicit_exchange_size: true,
        });
    }

    let base_quantity = order_quantity(config, action_record);
    let quantity = resolve_entry_exchange_quantity(client, config, base_quantity, price).await?;
    Ok(ResolvedEntryOrderQuantities {
        base_quantity: quantity.base_quantity,
        exchange_quantity: quantity.exchange_quantity,
        explicit_exchange_size: false,
    })
}

async fn resolve_entry_exchange_quantity(
    client: &OkxPublicClient,
    config: &LiveStrategyConfig,
    base_quantity: f64,
    price: f64,
) -> AppResult<crate::trading_semantics::ResolvedExchangeQuantity> {
    if !base_quantity.is_finite() || base_quantity <= 0.0 {
        return Err(AppError::Validation("实时策略开仓数量无效".to_string()));
    }
    if !price.is_finite() || price <= 0.0 {
        return Err(AppError::Validation("实时策略开仓价格无效".to_string()));
    }

    let rules = fetch_instrument_order_rules(client, config).await?;
    resolve_entry_exchange_quantity_from_base(config, &rules, base_quantity, price)
}

fn standalone_risk_trigger_price(
    kind: ProtectiveOrderKind,
    close_order: &ResolvedRiskCloseOrder,
    risk: &StrategyRiskOrderIntent,
) -> AppResult<f64> {
    resolve_standalone_protective_trigger_price(
        kind,
        &close_order.order_side,
        close_order.average_price,
        risk.trigger_price,
        risk.stop_loss,
        risk.take_profit,
        &risk.reason,
    )
}

fn order_size_string(quantity: f64) -> String {
    let value = if quantity.is_finite() && quantity > 0.0 {
        quantity
    } else {
        0.0
    };
    let formatted = format!("{value:.8}");
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

fn normalized_requested_order_type(order_type: &str) -> String {
    normalize_runtime_order_type_text(order_type)
}

fn order_price_string_for_order_type(order_type: &str, price: f64) -> String {
    if order_type_omits_price(order_type) {
        String::new()
    } else {
        order_size_string(price)
    }
}

fn response_text(response: &Value, key: &str) -> String {
    response
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            response
                .get("data")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(|item| item.get(key))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default()
}

fn order_submit_error(response: &Value) -> Option<String> {
    let item = response
        .get("data")
        .and_then(Value::as_array)
        .and_then(|items| items.first())?;
    let code = item
        .get("sCode")
        .or_else(|| item.get("code"))
        .and_then(Value::as_str)
        .unwrap_or("0");
    if code == "0" {
        return None;
    }
    let message = item
        .get("sMsg")
        .or_else(|| item.get("msg"))
        .and_then(Value::as_str)
        .unwrap_or("OKX order rejected");
    Some(format!("OKX 下单失败: sCode={code}; {message}"))
}

fn attached_algo_orders_from_risk_orders(
    risk_orders: &[StrategyRiskOrderIntent],
    symbol: &str,
    entry_side: &str,
    entry_price: f64,
    instrument_rules: Option<&InstrumentOrderRules>,
) -> AppResult<Vec<OkxAttachedAlgoOrder>> {
    let mut algos = Vec::new();

    for risk in risk_orders {
        validate_attached_protective_order_scope(&risk.symbol, &risk.side, symbol, entry_side)?;
        let kind = attached_risk_kind(risk)?;
        let trigger_price = resolve_attached_protective_trigger_price(
            kind,
            entry_side,
            entry_price,
            risk.trigger_price,
            risk.stop_loss,
            risk.take_profit,
            &risk.reason,
        )?;
        if let Some(rules) = instrument_rules {
            validate_price_matches_tick_size(
                rules,
                trigger_price,
                "保护单触发价",
                "已拒绝裸开仓以避免保护单静默改价",
            )?;
        }
        let trigger_price = order_size_string(trigger_price);
        if trigger_price.is_empty() || trigger_price == "0" {
            return Err(AppError::Validation(format!(
                "保护单触发价无效，reason={}，已拒绝裸开仓",
                risk.reason
            )));
        }
        let algo = match kind {
            ProtectiveOrderKind::StopLoss => OkxAttachedAlgoOrder::stop_loss_market(trigger_price),
            ProtectiveOrderKind::TakeProfit => {
                OkxAttachedAlgoOrder::take_profit_market(trigger_price)
            }
        };
        algos.push(algo);
    }

    Ok(algos)
}

fn attached_risk_kind(risk: &StrategyRiskOrderIntent) -> AppResult<ProtectiveOrderKind> {
    resolve_protective_order_kind(&risk.order_type, risk.stop_loss, risk.take_profit)
}

fn attached_risk_kind_from_order_type(order_type: &str) -> Option<ProtectiveOrderKind> {
    protective_order_kind_from_order_type(order_type)
}

fn is_unknown_okx_trade_request_error(error: &AppError) -> bool {
    let AppError::Runtime(message) = error else {
        return false;
    };
    let normalized = message.to_ascii_lowercase();
    if normalized.contains("okx private api error") {
        return false;
    }
    if normalized.contains("okx private response read failed")
        || normalized.contains("okx private response is not valid json")
        || normalized.contains("okx private response missing code")
        || normalized.contains("响应缺少 data[0]")
    {
        return true;
    }
    if normalized.contains("okx private http status 429")
        || normalized.contains("okx private http status 500")
        || normalized.contains("okx private http status 502")
        || normalized.contains("okx private http status 503")
        || normalized.contains("okx private http status 504")
    {
        return true;
    }
    normalized.contains("connection")
        || normalized.contains("timeout")
        || normalized.contains("timed out")
        || normalized.contains("network")
        || normalized.contains("proxy")
}
