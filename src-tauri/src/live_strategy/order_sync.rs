use serde_json::Value;
use sqlx::{Row, SqlitePool};

use crate::error::AppError;
use crate::okx::{normalized_okx_client_order_id, OkxPrivateClient};
use crate::trading_fills::{
    lookup_arrival_evidence_for_symbol, okx_text, upsert_local_fill, UpsertLocalFillRequest,
};

use super::{
    storage::{
        mark_live_attached_algo_orders_parent_terminal_unfilled,
        mark_live_planned_exit_entry_order_failed_for_mode,
        mark_live_planned_exit_entry_order_failed_for_strategy,
        mark_live_planned_exit_order_terminal_for_mode,
        mark_live_planned_exit_order_terminal_for_strategy, query_live_algo_order_sync_candidates,
        query_live_fill_sync_scopes, query_live_order_sync_candidates,
        query_submitted_live_planned_exit_order_sync_candidates,
        update_live_algo_order_actual_state_by_identity_and_symbol,
        update_live_algo_order_exchange_state_by_identity_and_symbol,
        update_live_exchange_order_state_by_identity_and_symbol, update_live_order_exchange_state,
        LiveAlgoOrderActualState, LiveOrderExchangeState, LiveOrderSyncCandidate,
        LivePlannedExitOrderSyncCandidate,
    },
    types::LiveStrategyConfig,
    LiveStrategyRuntime,
};

mod runtime;

#[cfg(test)]
mod tests;

const DEFAULT_LIVE_FILL_SYNC_SYMBOL_LIMIT: i64 = 30;
const MAX_LIVE_FILL_SYNC_SYMBOL_LIMIT: i64 = 50;
const LIVE_FILL_SYNC_LIMIT_PER_SYMBOL: u32 = 100;
const SUBMIT_UNKNOWN_NOT_FOUND_GRACE_MS: i64 = 30_000;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PrivateOrderPersistOutcome {
    pub(crate) order_changed: u64,
    pub(crate) planned_exit_changed: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PrivateFillPersistOutcome {
    pub(crate) order_changed: u64,
    pub(crate) planned_exit_changed: u64,
}

pub(crate) async fn persist_private_order_event(
    db: &SqlitePool,
    order: &Value,
) -> crate::error::AppResult<PrivateOrderPersistOutcome> {
    let Some(update) = exchange_order_state_from_private_ws_order(order) else {
        return Ok(PrivateOrderPersistOutcome::default());
    };
    let mode = required_private_event_mode(order, "order")?;
    let inst_id = required_private_event_inst_id(order, "order")?;
    let order_id = update.order_id.clone();
    let client_order_id = update.client_order_id.clone();
    let changed = update_live_exchange_order_state_by_identity_and_symbol(
        db,
        mode.as_str(),
        &inst_id,
        &order_id,
        &client_order_id,
        &update,
    )
    .await?;
    let planned_exit_changed = mark_live_planned_exit_order_terminal_for_mode(
        db,
        mode.as_str(),
        &inst_id,
        &update.order_id,
        &update.client_order_id,
        &update.status,
        &update.error_message,
        next_exit_order_retry_at(),
    )
    .await?;
    let (entry_plan_changed, attached_algo_changed) = if is_unfilled_terminal_order_state(&update) {
        let entry_plan_changed = mark_live_planned_exit_entry_order_failed_for_mode(
            db,
            mode.as_str(),
            &inst_id,
            &update.order_id,
            &update.client_order_id,
            &update.status,
            &update.error_message,
        )
        .await?;
        let attached_algo_changed = mark_live_attached_algo_orders_parent_terminal_unfilled(
            db,
            mode.as_str(),
            &inst_id,
            &update.order_id,
            &update.client_order_id,
            &update.status,
            &update.error_message,
        )
        .await?;
        (entry_plan_changed, attached_algo_changed)
    } else {
        (0, 0)
    };
    let linked_algo_changed =
        persist_linked_algo_actual_order_event(db, mode.as_str(), &inst_id, order).await?;
    Ok(PrivateOrderPersistOutcome {
        order_changed: changed + linked_algo_changed + attached_algo_changed,
        planned_exit_changed: planned_exit_changed + entry_plan_changed,
    })
}

pub(crate) async fn persist_private_algo_order_event(
    db: &SqlitePool,
    order: &Value,
) -> crate::error::AppResult<PrivateOrderPersistOutcome> {
    let Some(update) = algo_order_state_from_private_ws_order(order) else {
        return Ok(PrivateOrderPersistOutcome::default());
    };
    let mode = required_private_event_mode(order, "algo order")?;
    let inst_id = required_private_event_inst_id(order, "algo order")?;
    let order_id = update.order_id.clone();
    let client_order_id = update.client_order_id.clone();
    let changed = update_live_algo_order_exchange_state_by_identity_and_symbol(
        db,
        mode.as_str(),
        &inst_id,
        &order_id,
        &client_order_id,
        &update,
    )
    .await?;
    let actual_changed =
        persist_linked_algo_actual_order_event(db, mode.as_str(), &inst_id, order).await?;
    Ok(PrivateOrderPersistOutcome {
        order_changed: changed + actual_changed,
        planned_exit_changed: 0,
    })
}

pub(crate) async fn persist_private_fill_event(
    db: &SqlitePool,
    fill: &Value,
) -> crate::error::AppResult<PrivateFillPersistOutcome> {
    let mode = required_private_event_mode(fill, "fill")?;
    let trade_id = value_text(fill, "trade_id")
        .or_else(|| fill.get("raw").and_then(|raw| value_text(raw, "tradeId")))
        .unwrap_or_default();
    if trade_id.trim().is_empty() {
        return Ok(PrivateFillPersistOutcome::default());
    }
    let inst_id = required_private_event_inst_id(fill, "fill")?;
    let order_id = value_text(fill, "ord_id")
        .or_else(|| fill.get("raw").and_then(|raw| value_text(raw, "ordId")))
        .unwrap_or_default();
    let client_order_id = value_text(fill, "cl_ord_id")
        .or_else(|| fill.get("raw").and_then(|raw| value_text(raw, "clOrdId")))
        .unwrap_or_default();
    let arrival = lookup_arrival_evidence_for_symbol(
        db,
        mode.as_str(),
        &inst_id,
        &order_id,
        &client_order_id,
    )
    .await?;
    let raw = fill
        .get("raw")
        .filter(|item| item.is_object())
        .unwrap_or(fill);
    let fill_item = if fill.get("source").is_some() {
        fill
    } else {
        raw
    };
    upsert_local_fill(UpsertLocalFillRequest {
        db,
        mode: mode.as_str(),
        trade_id: &trade_id,
        inst_id: &inst_id,
        item: fill_item,
        order_id: &order_id,
        client_order_id: &client_order_id,
        arrival: &arrival,
    })
    .await?;
    let linked_algo_changed =
        persist_linked_algo_actual_order_event(db, mode.as_str(), &inst_id, fill).await?;
    let mut outcome = apply_fill_aggregate_to_live_order(
        db,
        mode.as_str(),
        &inst_id,
        &order_id,
        &client_order_id,
    )
    .await?;
    outcome.order_changed += linked_algo_changed;
    Ok(outcome)
}

async fn persist_linked_algo_actual_order_event(
    db: &SqlitePool,
    mode: &str,
    inst_id: &str,
    event: &Value,
) -> crate::error::AppResult<u64> {
    let raw = event
        .get("raw")
        .filter(|item| item.is_object())
        .unwrap_or(event);
    let algo_order_id = linked_algo_order_id(event, raw);
    let algo_client_order_id = linked_algo_client_order_id(event, raw);
    if algo_order_id.trim().is_empty() && algo_client_order_id.trim().is_empty() {
        return Ok(0);
    }
    let actual_order_id = event_text(
        event,
        raw,
        &["actual_order_id", "actualOrdId", "ord_id", "ordId"],
    );
    let actual_client_order_id = event_text(
        event,
        raw,
        &[
            "actual_client_order_id",
            "actualClOrdId",
            "cl_ord_id",
            "clOrdId",
        ],
    );
    if actual_order_id.trim().is_empty() && actual_client_order_id.trim().is_empty() {
        return Ok(0);
    }
    let Some(state) =
        linked_algo_actual_state(event, raw, &actual_order_id, &actual_client_order_id)
    else {
        return Ok(0);
    };
    update_live_algo_order_actual_state_by_identity_and_symbol(
        db,
        mode,
        inst_id,
        &algo_order_id,
        &algo_client_order_id,
        &state,
    )
    .await
}

async fn apply_fill_aggregate_to_live_order(
    db: &SqlitePool,
    mode: &str,
    inst_id: &str,
    order_id: &str,
    client_order_id: &str,
) -> crate::error::AppResult<PrivateFillPersistOutcome> {
    let inst_id = inst_id.trim();
    let order_id = order_id.trim();
    let client_order_id = client_order_id.trim();
    if inst_id.is_empty() || (order_id.is_empty() && client_order_id.is_empty()) {
        return Ok(PrivateFillPersistOutcome::default());
    }
    let fill_size = aggregate_local_fill_size(db, mode, inst_id, order_id, client_order_id).await?;
    if !fill_size.is_finite() || fill_size <= 0.0 {
        return Ok(PrivateFillPersistOutcome::default());
    }
    let rows = sqlx::query(
        r#"
        SELECT id, strategy_id, size, action
        FROM live_order_records
        WHERE mode = ?
          AND UPPER(TRIM(inst_id)) = UPPER(TRIM(?))
          AND (
            (LENGTH(TRIM(?)) > 0 AND order_id = ?)
            OR (LENGTH(TRIM(?)) > 0 AND client_order_id = ?)
            OR (LENGTH(TRIM(?)) > 0 AND actual_order_id = ?)
            OR (LENGTH(TRIM(?)) > 0 AND actual_client_order_id = ?)
          )
          AND LOWER(TRIM(status)) IN (
            'submit_unknown', 'submitted', 'pending', 'open', 'live',
            'partially_filled', 'partial-filled', 'partially-filled',
            'algo_submitted', 'algo_submit_unknown', 'algo_live',
            'cancel_requested', 'modify_requested',
            'algo_cancel_requested', 'algo_modify_requested',
            'algo_partially_effective', 'algo_effective'
          )
        ORDER BY created_at DESC, id DESC
        "#,
    )
    .bind(mode.trim())
    .bind(inst_id)
    .bind(order_id)
    .bind(order_id)
    .bind(client_order_id)
    .bind(client_order_id)
    .bind(order_id)
    .bind(order_id)
    .bind(client_order_id)
    .bind(client_order_id)
    .fetch_all(db)
    .await?;

    if rows.len() > 1 {
        return Err(AppError::Validation(format!(
            "OKX fill 在本地命中多条活动订单记录，已拒绝聚合更新: mode={}, inst_id={}, order_id={}, client_order_id={}",
            mode.trim(),
            inst_id,
            order_id,
            client_order_id
        )));
    }

    let mut order_changed = 0_u64;
    let mut planned_exit_changed = 0_u64;
    for row in rows {
        let row_id = positive_local_order_i64(&row, "id")?;
        let strategy_id = required_local_order_text(&row, "strategy_id")?;
        let target_size = positive_local_order_f64(&row, "size")?;
        let is_complete = fill_size + fill_completion_tolerance(target_size) >= target_size;
        let action = required_local_order_text(&row, "action")?;
        let is_algo_order = action.trim().eq_ignore_ascii_case("place_risk_order");
        let status = if is_algo_order {
            if is_complete {
                "algo_effective"
            } else {
                "algo_partially_effective"
            }
        } else if is_complete {
            "filled"
        } else {
            "partially_filled"
        };
        let message = if is_complete {
            if is_algo_order {
                format!(
                    "OKX protective algo actual order filled; filled_size={fill_size}; target_size={target_size}"
                )
            } else {
                format!(
                    "OKX fill event completed order; filled_size={fill_size}; target_size={target_size}"
                )
            }
        } else {
            if is_algo_order {
                format!(
                    "OKX protective algo actual order partially filled; filled_size={fill_size}; target_size={target_size}"
                )
            } else {
                format!(
                    "OKX fill event partially filled order; filled_size={fill_size}; target_size={target_size}"
                )
            }
        };
        let changed = update_live_order_exchange_state(
            db,
            row_id,
            &LiveOrderExchangeState {
                status: status.to_string(),
                success: true,
                error_message: message.clone(),
                order_id: if is_algo_order {
                    String::new()
                } else {
                    order_id.to_string()
                },
                client_order_id: if is_algo_order {
                    String::new()
                } else {
                    client_order_id.to_string()
                },
            },
        )
        .await?;
        if changed {
            order_changed += 1;
        }
        if is_complete && !is_algo_order {
            planned_exit_changed += mark_live_planned_exit_order_terminal_for_strategy(
                db,
                mode,
                &strategy_id,
                inst_id,
                order_id,
                client_order_id,
                "filled",
                &message,
                next_exit_order_retry_at(),
            )
            .await?;
        }
    }
    Ok(PrivateFillPersistOutcome {
        order_changed,
        planned_exit_changed,
    })
}

fn required_local_order_text(
    row: &sqlx::sqlite::SqliteRow,
    column: &str,
) -> crate::error::AppResult<String> {
    let Some(value) = row.try_get::<Option<String>, _>(column)? else {
        return Err(AppError::Runtime(format!(
            "live order sync {column} 不能为空"
        )));
    };
    if value.trim().is_empty() {
        return Err(AppError::Runtime(format!(
            "live order sync {column} 不能为空"
        )));
    }
    Ok(value)
}

fn positive_local_order_i64(
    row: &sqlx::sqlite::SqliteRow,
    column: &str,
) -> crate::error::AppResult<i64> {
    let value = row.try_get::<i64, _>(column)?;
    if value <= 0 {
        return Err(AppError::Runtime(format!(
            "live order sync {column} 必须为正整数"
        )));
    }
    Ok(value)
}

fn positive_local_order_f64(
    row: &sqlx::sqlite::SqliteRow,
    column: &str,
) -> crate::error::AppResult<f64> {
    let value = row.try_get::<f64, _>(column)?;
    if !value.is_finite() || value <= 0.0 {
        return Err(AppError::Runtime(format!(
            "live order sync {column} 必须为有限正数"
        )));
    }
    Ok(value)
}

async fn aggregate_local_fill_size(
    db: &SqlitePool,
    mode: &str,
    inst_id: &str,
    order_id: &str,
    client_order_id: &str,
) -> crate::error::AppResult<f64> {
    let value = sqlx::query_scalar::<_, Option<f64>>(
        r#"
        SELECT SUM(CASE
          WHEN CAST(fill_sz AS REAL) > 0 THEN CAST(fill_sz AS REAL)
          ELSE 0
        END)
        FROM local_fills
        WHERE mode = ?
          AND UPPER(TRIM(inst_id)) = UPPER(TRIM(?))
          AND (
            (LENGTH(TRIM(?)) > 0 AND order_id = ?)
            OR (LENGTH(TRIM(?)) > 0 AND client_order_id = ?)
          )
        "#,
    )
    .bind(mode.trim())
    .bind(inst_id.trim())
    .bind(order_id.trim())
    .bind(order_id.trim())
    .bind(client_order_id.trim())
    .bind(client_order_id.trim())
    .fetch_one(db)
    .await?;
    Ok(value.unwrap_or(0.0))
}

fn fill_completion_tolerance(target_size: f64) -> f64 {
    (target_size.abs() * 1e-9).max(1e-12)
}

fn live_fill_sync_symbol_limit(config: &LiveStrategyConfig) -> i64 {
    let configured = ["live_fill_sync_symbol_limit", "fill_sync_symbol_limit"]
        .iter()
        .find_map(|key| config.params.get(*key).and_then(json_i64));
    configured
        .unwrap_or(DEFAULT_LIVE_FILL_SYNC_SYMBOL_LIMIT)
        .clamp(1, MAX_LIVE_FILL_SYNC_SYMBOL_LIMIT)
}

fn json_i64(value: &Value) -> Option<i64> {
    if let Some(item) = value.as_i64() {
        return Some(item);
    }
    if let Some(item) = value.as_u64() {
        return i64::try_from(item).ok();
    }
    let parsed = value
        .as_f64()
        .or_else(|| value.as_str()?.trim().parse::<f64>().ok())?;
    parsed.is_finite().then_some(parsed.round() as i64)
}

fn required_private_event_mode(event: &Value, event_kind: &str) -> crate::error::AppResult<String> {
    let raw = event
        .get("raw")
        .filter(|item| item.is_object())
        .unwrap_or(event);
    let mode = value_text(event, "mode").or_else(|| value_text(raw, "mode"));
    let Some(mode) = mode.map(|value| value.trim().to_ascii_lowercase()) else {
        return Err(AppError::Validation(format!(
            "OKX private {event_kind} event 缺少 mode，已拒绝持久化以避免跨模拟盘/实盘误归因"
        )));
    };
    match mode.as_str() {
        "live" => Ok("live".to_string()),
        "simulated" => Ok("simulated".to_string()),
        _ => Err(AppError::Validation(format!(
            "OKX private {event_kind} event mode={} 不受支持，必须为 live 或 simulated",
            mode
        ))),
    }
}

fn required_private_event_inst_id(
    event: &Value,
    event_kind: &str,
) -> crate::error::AppResult<String> {
    let raw = event
        .get("raw")
        .filter(|item| item.is_object())
        .unwrap_or(event);
    let inst_id = event_text(event, raw, &["inst_id", "instId"])
        .trim()
        .to_ascii_uppercase();
    if inst_id.is_empty() {
        return Err(AppError::Validation(format!(
            "OKX private {event_kind} event 缺少 inst_id，已拒绝持久化以避免跨币种误归因"
        )));
    }
    Ok(inst_id)
}

fn sync_order_identity(candidate: &LiveOrderSyncCandidate) -> Option<(String, String)> {
    order_sync_identity(&candidate.order_id, &candidate.client_order_id)
}

fn planned_exit_order_sync_identity(
    candidate: &LivePlannedExitOrderSyncCandidate,
) -> Option<(String, String)> {
    order_sync_identity(&candidate.order_id, &candidate.client_order_id)
}

fn order_sync_identity(order_id: &str, client_order_id: &str) -> Option<(String, String)> {
    let order_id = order_id.trim().to_string();
    let client_order_id = match normalized_okx_client_order_id(client_order_id) {
        Ok(Some(value)) => value.to_string(),
        Ok(None) => String::new(),
        Err(_) if !order_id.is_empty() => String::new(),
        Err(_) => return None,
    };
    if order_id.is_empty() && client_order_id.is_empty() {
        return None;
    }
    Some((order_id, client_order_id))
}

fn find_algo_order_by_identity(
    items: Vec<Value>,
    algo_id: &str,
    algo_client_order_id: &str,
) -> Option<Value> {
    let algo_id = algo_id.trim();
    let algo_client_order_id = algo_client_order_id.trim();
    items.into_iter().find(|item| {
        (!algo_id.is_empty() && value_text(item, "algoId").as_deref() == Some(algo_id))
            || (!algo_client_order_id.is_empty()
                && value_text(item, "algoClOrdId").as_deref() == Some(algo_client_order_id))
    })
}

fn find_exchange_order_by_identity(
    items: Vec<Value>,
    order_id: &str,
    client_order_id: &str,
) -> Option<Value> {
    let order_id = order_id.trim();
    let client_order_id = client_order_id.trim();
    items.into_iter().find(|item| {
        (!order_id.is_empty() && value_text(item, "ordId").as_deref() == Some(order_id))
            || (!client_order_id.is_empty()
                && value_text(item, "clOrdId").as_deref() == Some(client_order_id))
    })
}

fn algo_order_state_from_okx_order(
    order: &Value,
    candidate: &LiveOrderSyncCandidate,
) -> Option<LiveOrderExchangeState> {
    let status = normalized_algo_order_status(value_text(order, "state").as_deref()?)?;
    let order_id = value_text(order, "algoId").unwrap_or_else(|| candidate.order_id.clone());
    let client_order_id =
        value_text(order, "algoClOrdId").unwrap_or_else(|| candidate.client_order_id.clone());
    let success = matches!(
        status.as_str(),
        "algo_live" | "algo_effective" | "algo_partially_effective"
    );
    let error_message = algo_order_message(order, &status);
    Some(LiveOrderExchangeState {
        status,
        success,
        error_message,
        order_id,
        client_order_id,
    })
}

fn algo_order_state_from_private_ws_order(order: &Value) -> Option<LiveOrderExchangeState> {
    let status = normalized_algo_order_status(value_text(order, "state").as_deref()?)?;
    let raw = order
        .get("raw")
        .filter(|item| item.is_object())
        .unwrap_or(order);
    let order_id = value_text(order, "algo_id")
        .or_else(|| value_text(order, "algoId"))
        .or_else(|| value_text(raw, "algoId"))
        .unwrap_or_default();
    let client_order_id = value_text(order, "algo_cl_ord_id")
        .or_else(|| value_text(order, "algoClOrdId"))
        .or_else(|| value_text(raw, "algoClOrdId"))
        .unwrap_or_default();
    if order_id.trim().is_empty() && client_order_id.trim().is_empty() {
        return None;
    }
    let success = matches!(
        status.as_str(),
        "algo_live" | "algo_effective" | "algo_partially_effective"
    );
    let error_message = algo_order_message(raw, &status);
    Some(LiveOrderExchangeState {
        status,
        success,
        error_message,
        order_id,
        client_order_id,
    })
}

fn normalized_algo_order_status(status: &str) -> Option<String> {
    match status.trim().to_ascii_lowercase().as_str() {
        "live" => Some("algo_live".to_string()),
        "effective" => Some("algo_effective".to_string()),
        "partially_effective" | "partially-effective" => {
            Some("algo_partially_effective".to_string())
        }
        "canceled" | "cancelled" => Some("algo_canceled".to_string()),
        "order_failed" | "failed" | "fail" => Some("algo_failed".to_string()),
        _ => None,
    }
}

fn algo_order_message(order: &Value, status: &str) -> String {
    match status {
        "algo_live" => "OKX protective algo order live; waiting for trigger".to_string(),
        "algo_effective" => format!(
            "OKX protective algo order triggered; actualSz={}",
            empty_dash(&value_text(order, "actualSz").unwrap_or_default())
        ),
        "algo_partially_effective" => format!(
            "OKX protective algo order partially triggered; actualSz={}",
            empty_dash(&value_text(order, "actualSz").unwrap_or_default())
        ),
        "algo_canceled" => "OKX protective algo order canceled".to_string(),
        "algo_failed" => {
            let fail_code = value_text(order, "failCode").unwrap_or_default();
            let fail_reason = value_text(order, "failReason")
                .or_else(|| value_text(order, "sMsg"))
                .unwrap_or_default();
            format!(
                "OKX protective algo order failed; failCode={}; reason={}",
                empty_dash(&fail_code),
                empty_dash(&fail_reason)
            )
        }
        _ => format!("OKX protective algo order state synced: {status}"),
    }
}

fn exchange_order_state_from_okx_order(
    order: &Value,
    candidate: &LiveOrderSyncCandidate,
) -> Option<LiveOrderExchangeState> {
    exchange_order_state_from_okx_order_identity(
        order,
        &candidate.order_id,
        &candidate.client_order_id,
    )
}

fn exchange_order_state_from_okx_order_identity(
    order: &Value,
    fallback_order_id: &str,
    fallback_client_order_id: &str,
) -> Option<LiveOrderExchangeState> {
    let status = normalized_live_order_status(value_text(order, "state").as_deref()?)?;
    let order_id = value_text(order, "ordId").unwrap_or_else(|| fallback_order_id.to_string());
    let client_order_id =
        value_text(order, "clOrdId").unwrap_or_else(|| fallback_client_order_id.to_string());
    let success = exchange_order_success(order, &status);
    let error_message = exchange_order_message(order, &status);
    Some(LiveOrderExchangeState {
        status,
        success,
        error_message,
        order_id,
        client_order_id,
    })
}

fn exchange_order_state_from_private_ws_order(order: &Value) -> Option<LiveOrderExchangeState> {
    let status = normalized_live_order_status(value_text(order, "state").as_deref()?)?;
    let raw = order
        .get("raw")
        .filter(|item| item.is_object())
        .unwrap_or(order);
    let order_id = value_text(order, "ord_id")
        .or_else(|| value_text(order, "ordId"))
        .or_else(|| value_text(raw, "ordId"))
        .unwrap_or_default();
    let client_order_id = value_text(order, "cl_ord_id")
        .or_else(|| value_text(order, "clOrdId"))
        .or_else(|| value_text(raw, "clOrdId"))
        .unwrap_or_default();
    if order_id.trim().is_empty() && client_order_id.trim().is_empty() {
        return None;
    }
    let success = exchange_order_success(order, &status) || exchange_order_success(raw, &status);
    let error_message = exchange_order_message(raw, &status);
    Some(LiveOrderExchangeState {
        status,
        success,
        error_message,
        order_id,
        client_order_id,
    })
}

fn linked_algo_actual_state(
    event: &Value,
    raw: &Value,
    actual_order_id: &str,
    actual_client_order_id: &str,
) -> Option<LiveAlgoOrderActualState> {
    if let Some(algo_state) = algo_order_state_from_private_ws_order(event) {
        return Some(LiveAlgoOrderActualState {
            status: algo_state.status,
            success: algo_state.success,
            error_message: algo_state.error_message,
            actual_order_id: actual_order_id.to_string(),
            actual_client_order_id: actual_client_order_id.to_string(),
        });
    }
    let status = if let Some(order_status) =
        normalized_live_order_status(event_text(event, raw, &["state"]).as_str())
    {
        match order_status.as_str() {
            "filled" => "algo_effective",
            "partially_filled" | "live" => "algo_partially_effective",
            "canceled" if filled_quantity(raw).max(filled_quantity(event)) > 0.0 => {
                "algo_partially_effective"
            }
            "canceled" | "rejected" => "algo_failed",
            _ => return None,
        }
    } else if linked_event_fill_size(event, raw) > 0.0 {
        "algo_partially_effective"
    } else {
        return None;
    };
    let success = matches!(status, "algo_effective" | "algo_partially_effective");
    let error_message = linked_algo_actual_message(event, raw, status);
    Some(LiveAlgoOrderActualState {
        status: status.to_string(),
        success,
        error_message,
        actual_order_id: actual_order_id.to_string(),
        actual_client_order_id: actual_client_order_id.to_string(),
    })
}

fn linked_algo_actual_message(event: &Value, raw: &Value, status: &str) -> String {
    let actual_order_id = event_text(
        event,
        raw,
        &["actual_order_id", "actualOrdId", "ord_id", "ordId"],
    );
    let fill_sz = event_text(event, raw, &["accFillSz", "fillSz", "fill_sz"]);
    match status {
        "algo_effective" => format!(
            "OKX protective algo actual order filled; ordId={}; fillSz={}",
            empty_dash(&actual_order_id),
            empty_dash(&fill_sz)
        ),
        "algo_partially_effective" => format!(
            "OKX protective algo actual order active/partially filled; ordId={}; fillSz={}",
            empty_dash(&actual_order_id),
            empty_dash(&fill_sz)
        ),
        "algo_failed" => {
            let reason = event_text(event, raw, &["sMsg", "msg", "failReason"]);
            format!(
                "OKX protective algo actual order failed; ordId={}; reason={}",
                empty_dash(&actual_order_id),
                empty_dash(&reason)
            )
        }
        _ => format!("OKX protective algo actual order synced: {status}"),
    }
}

fn linked_algo_order_id(event: &Value, raw: &Value) -> String {
    let direct = event_text(event, raw, &["algo_id", "algoId"]);
    if direct.trim().is_empty() {
        nested_event_text(event, raw, "linkedAlgoOrd", "algoId")
    } else {
        direct
    }
}

fn linked_algo_client_order_id(event: &Value, raw: &Value) -> String {
    let direct = event_text(event, raw, &["algo_cl_ord_id", "algoClOrdId"]);
    if direct.trim().is_empty() {
        nested_event_text(event, raw, "linkedAlgoOrd", "algoClOrdId")
    } else {
        direct
    }
}

fn linked_event_fill_size(event: &Value, raw: &Value) -> f64 {
    event_text(event, raw, &["accFillSz", "fillSz", "fill_sz", "actualSz"])
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(0.0)
}

fn normalized_live_order_status(status: &str) -> Option<String> {
    let status = status.trim().to_ascii_lowercase();
    match status.as_str() {
        "live" => Some("live".to_string()),
        "partially_filled" | "partial-filled" | "partially-filled" => {
            Some("partially_filled".to_string())
        }
        "filled" | "fully_filled" | "fully-filled" => Some("filled".to_string()),
        "canceled" | "cancelled" => Some("canceled".to_string()),
        "rejected" | "reject" => Some("rejected".to_string()),
        _ => None,
    }
}

fn is_exchange_order_not_found_after_grace(
    candidate: &LiveOrderSyncCandidate,
    error_message: &str,
    now_ms: i64,
) -> bool {
    if !matches!(
        candidate.status.trim().to_ascii_lowercase().as_str(),
        "submitting" | "submit_unknown" | "submitted"
    ) {
        return false;
    }
    if !okx_order_not_found_error(error_message) {
        return false;
    }
    let elapsed_ms = now_ms.saturating_sub(candidate.created_at_ms);
    elapsed_ms >= SUBMIT_UNKNOWN_NOT_FOUND_GRACE_MS
}

fn is_planned_exit_order_not_found_after_grace(
    candidate: &LivePlannedExitOrderSyncCandidate,
    error_message: &str,
    now_ms: i64,
) -> bool {
    if !okx_order_not_found_error(error_message) {
        return false;
    }
    let elapsed_ms = now_ms.saturating_sub(candidate.updated_at_ms);
    elapsed_ms >= SUBMIT_UNKNOWN_NOT_FOUND_GRACE_MS
}

fn okx_order_not_found_error(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("51603")
        || normalized.contains("order does not exist")
        || normalized.contains("order doesn't exist")
        || normalized.contains("order not exist")
        || normalized.contains("not found")
        || normalized.contains("查无")
        || normalized.contains("不存在")
}

fn is_unfilled_terminal_order_state(update: &LiveOrderExchangeState) -> bool {
    !update.success
        && matches!(
            update.status.trim().to_ascii_lowercase().as_str(),
            "canceled" | "cancelled" | "rejected" | "reject"
        )
}

fn exchange_order_success(order: &Value, status: &str) -> bool {
    matches!(status, "live" | "partially_filled" | "filled")
        || (status == "canceled" && filled_quantity(order) > 0.0)
}

fn exchange_order_message(order: &Value, status: &str) -> String {
    let avg_px = value_text(order, "avgPx")
        .or_else(|| value_text(order, "avg_px"))
        .unwrap_or_default();
    let fill_sz = value_text(order, "accFillSz")
        .or_else(|| value_text(order, "fillSz"))
        .or_else(|| value_text(order, "fill_sz"))
        .unwrap_or_default();
    match status {
        "filled" => format!(
            "OKX order filled; avgPx={}; accFillSz={}",
            empty_dash(&avg_px),
            empty_dash(&fill_sz)
        ),
        "partially_filled" => format!(
            "OKX order partially filled; avgPx={}; accFillSz={}",
            empty_dash(&avg_px),
            empty_dash(&fill_sz)
        ),
        "live" => "OKX order live; waiting for fill".to_string(),
        "canceled" if filled_quantity(order) > 0.0 => format!(
            "OKX order canceled after partial fill; avgPx={}; accFillSz={}",
            empty_dash(&avg_px),
            empty_dash(&fill_sz)
        ),
        "canceled" => "OKX order canceled without fill".to_string(),
        "rejected" => value_text(order, "sMsg")
            .or_else(|| value_text(order, "msg"))
            .map(|message| format!("OKX order rejected: {message}"))
            .unwrap_or_else(|| "OKX order rejected".to_string()),
        _ => format!("OKX order state synced: {status}"),
    }
}
fn filled_quantity(order: &Value) -> f64 {
    ["accFillSz", "fillSz", "fill_sz"]
        .iter()
        .find_map(|key| value_text(order, key))
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(0.0)
}

fn next_exit_order_retry_at() -> i64 {
    chrono::Utc::now().timestamp_millis().saturating_add(15_000)
}

fn value_text(value: &Value, key: &str) -> Option<String> {
    match value.get(key)? {
        Value::String(item) => {
            let item = item.trim().to_string();
            (!item.is_empty()).then_some(item)
        }
        Value::Number(item) => Some(item.to_string()),
        Value::Bool(item) => Some(item.to_string()),
        _ => None,
    }
}

fn event_text(event: &Value, raw: &Value, keys: &[&str]) -> String {
    for key in keys {
        if let Some(value) = value_text(event, key).or_else(|| value_text(raw, key)) {
            return value;
        }
    }
    String::new()
}

fn nested_event_text(event: &Value, raw: &Value, object_key: &str, key: &str) -> String {
    event
        .get(object_key)
        .and_then(|value| value_text(value, key))
        .or_else(|| raw.get(object_key).and_then(|value| value_text(value, key)))
        .unwrap_or_default()
}

fn empty_dash(value: &str) -> &str {
    if value.trim().is_empty() {
        "-"
    } else {
        value
    }
}
