use serde_json::{json, Value};
use sqlx::{sqlite::SqliteRow, Row};

use crate::error::{AppError, AppResult};

pub(super) fn live_order_row_to_json(row: SqliteRow) -> AppResult<Value> {
    let status = required_text(&row, "status")?;
    let success = row_success_flag(&row)?;
    let size = row_positive_f64(&row, "size")?;
    let price = row_positive_f64(&row, "price")?;
    let value = size.zip(price).map(|(size, price)| size * price);
    let order_type = required_text(&row, "order_type")?;
    let stored_action = required_text(&row, "action")?;
    let action = local_order_action(&stored_action)?;
    let timestamp = row_positive_i64(&row, "action_timestamp")?;
    let created_at = required_text(&row, "created_at")?;
    Ok(json!({
        "id": row.try_get::<i64, _>("id")?,
        "order_id": optional_text(&row, "order_id")?,
        "client_order_id": optional_text(&row, "client_order_id")?,
        "parent_order_id": optional_text(&row, "parent_order_id")?,
        "parent_client_order_id": optional_text(&row, "parent_client_order_id")?,
        "actual_order_id": optional_text(&row, "actual_order_id")?,
        "actual_client_order_id": optional_text(&row, "actual_client_order_id")?,
        "inst_id": required_text(&row, "inst_id")?,
        "inst_type": required_text(&row, "inst_type")?,
        "symbol": required_text(&row, "symbol")?,
        "side": required_text(&row, "side")?,
        "order_type": order_type,
        "action": action,
        "size": size,
        "price": price,
        "value": value,
        "success": success,
        "status": status,
        "error_message": optional_text(&row, "error_message")?,
        "mode": required_text(&row, "mode")?,
        "strategy_id": required_text(&row, "strategy_id")?,
        "strategy_name": required_text(&row, "strategy_name")?,
        "run_id": required_text(&row, "run_id")?,
        "timestamp": timestamp,
        "arrival_ts": row.try_get::<Option<i64>, _>("arrival_ts")?,
        "arrival_mid_px": row_opt_f64(&row, "arrival_mid_px")?,
        "arrival_bid_px": row_opt_f64(&row, "arrival_bid_px")?,
        "arrival_ask_px": row_opt_f64(&row, "arrival_ask_px")?,
        "created_at": created_at
    }))
}

fn required_text(row: &SqliteRow, column: &str) -> AppResult<String> {
    let value = row.try_get::<String, _>(column)?;
    if value.trim().is_empty() {
        Err(AppError::Runtime(format!("live order {column} 不能为空")))
    } else {
        Ok(value)
    }
}

fn optional_text(row: &SqliteRow, column: &str) -> AppResult<String> {
    Ok(row
        .try_get::<Option<String>, _>(column)?
        .unwrap_or_default())
}

fn row_success_flag(row: &SqliteRow) -> AppResult<bool> {
    match row.try_get::<i64, _>("success")? {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(AppError::Runtime(format!(
            "live order success 必须是 0 或 1，当前为 {value}"
        ))),
    }
}

fn local_order_action(stored_action: &str) -> AppResult<&'static str> {
    let stored_action = stored_action.trim().to_ascii_lowercase();
    match stored_action.as_str() {
        "open_position" => Ok("open_position"),
        "close_position" => Ok("close_position"),
        "place_risk_order" => Ok("place_risk_order"),
        "cancel_order" => Ok("cancel_order"),
        "modify_order" => Ok("modify_order"),
        "hold" => Ok("hold"),
        _ => Err(AppError::Runtime(format!(
            "live order action 不是规范 actions 协议动作: {stored_action}"
        ))),
    }
}

fn row_positive_f64(row: &SqliteRow, column: &str) -> AppResult<Option<f64>> {
    let value = row.try_get::<Option<f64>, _>(column)?;
    match value {
        Some(value) if !value.is_finite() => Err(AppError::Runtime(format!(
            "live order {column} 不是有限数字"
        ))),
        Some(value) if value > 0.0 => Ok(Some(value)),
        Some(_) | None => Ok(None),
    }
}

fn row_opt_f64(row: &SqliteRow, column: &str) -> AppResult<Option<f64>> {
    let Some(value) = row.try_get::<Option<f64>, _>(column)? else {
        return Ok(None);
    };
    if value.is_finite() {
        Ok(Some(value))
    } else {
        Err(AppError::Runtime(format!(
            "live order {column} 不是有限数字"
        )))
    }
}

fn row_positive_i64(row: &SqliteRow, column: &str) -> AppResult<i64> {
    let value = row
        .try_get::<Option<i64>, _>(column)?
        .ok_or_else(|| AppError::Runtime(format!("live order {column} 缺失")))?;
    if value > 0 {
        Ok(value)
    } else {
        Err(AppError::Runtime(format!(
            "live order {column} 必须是正整数"
        )))
    }
}
