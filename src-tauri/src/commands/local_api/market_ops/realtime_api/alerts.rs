use serde_json::{json, Value};

use crate::alerts::{self as price_alert_store, TickerSnapshot};

use super::super::*;

pub(in crate::commands::local_api) async fn price_alerts(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let inst_id = param_string(req, "inst_id", "");
    let inst_type = param_string(req, "inst_type", "");
    let alerts = price_alert_store::list_alerts(
        &state.db,
        if inst_id.is_empty() {
            None
        } else {
            Some(inst_id.as_str())
        },
        if inst_type.is_empty() {
            None
        } else {
            Some(inst_type.as_str())
        },
    )
    .await?;
    Ok(code_ok(Value::Array(alerts)))
}

pub(in crate::commands::local_api) async fn create_price_alert(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let alert = price_alert_store::create_alert(&state.db, &req.body)
        .await
        .map_err(|error| AppError::Validation(error.to_string()))?;
    Ok(code_ok(alert))
}

pub(in crate::commands::local_api) async fn update_price_alert(
    state: &AppState,
    alert_id: &str,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    match price_alert_store::update_alert(&state.db, alert_id, &req.body)
        .await
        .map_err(|error| AppError::Validation(error.to_string()))?
    {
        Some(alert) => Ok(code_ok(alert)),
        None => Ok(json!({
            "code": 404,
            "message": "价格提醒不存在",
            "data": null
        })),
    }
}

pub(in crate::commands::local_api) async fn delete_price_alert(
    state: &AppState,
    alert_id: &str,
) -> AppResult<Value> {
    if price_alert_store::delete_alert(&state.db, alert_id).await? {
        return Ok(code_ok(json!({"deleted": true, "id": alert_id})));
    }
    Ok(json!({
        "code": 404,
        "message": "价格提醒不存在",
        "data": null
    }))
}

pub(in crate::commands::local_api) async fn evaluate_price_alerts(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let inst_id = request_string(req, "inst_id", "").trim().to_uppercase();
    if inst_id.is_empty() {
        return Err(AppError::Validation("inst_id 不能为空".to_string()));
    }
    let inst_type = request_string(req, "inst_type", &infer_inst_type(&inst_id))
        .trim()
        .to_uppercase();
    let last_price = request_f64(req, "last_price", request_f64(req, "last", 0.0));
    if !last_price.is_finite() || last_price <= 0.0 {
        return Err(AppError::Validation(
            "last_price 必须是大于 0 的有效价格".to_string(),
        ));
    }
    let change_24h =
        request_optional_f64(req, "change_24h").or_else(|| request_optional_f64(req, "change24h"));
    let ticker_ts = request_i64(
        req,
        "ticker_ts",
        request_i64(req, "ts", chrono::Utc::now().timestamp_millis()),
    );
    let triggered = price_alert_store::evaluate_ticker(
        &state.db,
        TickerSnapshot {
            inst_id,
            inst_type,
            last_price,
            change_24h,
            ticker_ts,
        },
    )
    .await?;
    Ok(code_ok(Value::Array(triggered)))
}
