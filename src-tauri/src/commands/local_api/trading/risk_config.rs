use super::*;

pub(crate) async fn risk_summary(state: &AppState, req: &LocalApiRequest) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let mut params = Map::new();
    params.insert("mode".to_string(), Value::String(mode));
    params.insert("days".to_string(), Value::Number(90.into()));
    let metrics_req = LocalApiRequest {
        method: "GET".to_string(),
        path: "/api/risk/metrics".to_string(),
        params,
        body: Value::Null,
    };
    let metrics = risk_metrics(state, &metrics_req).await?;
    let config = risk_control_config(state, req).await?;
    Ok(json!({
        "summary": metrics.get("data").cloned().unwrap_or(Value::Null),
        "config": config
    }))
}

pub(crate) async fn risk_control_config(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let saved = state
        .preferences
        .get(RISK_CONTROL_SETTINGS_KEY)
        .await?
        .and_then(|value| {
            value
                .as_object()
                .and_then(|all_modes| all_modes.get(&mode))
                .and_then(Value::as_object)
                .cloned()
        });
    Ok(risk_control_config_with_saved(&mode, saved.as_ref()))
}

pub(crate) async fn update_risk_control_config(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let enabled = body_bool(req, "enabled", DEFAULT_RISK_CONTROL_ENABLED);
    let max_single_loss_ratio = request_f64(
        req,
        "max_single_loss_ratio",
        DEFAULT_RISK_MAX_SINGLE_LOSS_RATIO,
    )
    .clamp(0.0, 1.0);
    let default_stop_loss_ratio =
        request_f64(req, "default_stop_loss_ratio", DEFAULT_RISK_STOP_LOSS_RATIO).clamp(0.0, 1.0);
    let max_total_position_ratio = request_f64(
        req,
        "max_total_position_ratio",
        DEFAULT_RISK_MAX_TOTAL_POSITION_RATIO,
    )
    .clamp(0.0, 10.0);
    let max_order_value =
        request_f64(req, "max_order_value", DEFAULT_RISK_MAX_ORDER_VALUE).max(0.0);
    let max_daily_loss_pct =
        request_f64(req, "max_daily_loss_pct", DEFAULT_RISK_MAX_DAILY_LOSS_PCT).clamp(0.0, 1.0);
    let max_position_pct =
        request_f64(req, "max_position_pct", DEFAULT_RISK_MAX_POSITION_PCT).clamp(0.0, 1.0);
    let config = json!({
        "mode": mode.clone(),
        "enabled": enabled,
        "max_single_loss_ratio": max_single_loss_ratio,
        "default_stop_loss_ratio": default_stop_loss_ratio,
        "max_total_position_ratio": max_total_position_ratio,
        "max_position_pct": max_position_pct,
        "max_daily_loss_pct": max_daily_loss_pct,
        "max_order_value": max_order_value,
        "updated_at": now_text()
    });

    let mut all_modes = state
        .preferences
        .get(RISK_CONTROL_SETTINGS_KEY)
        .await?
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_else(Map::new);
    all_modes.insert(mode.clone(), config.clone());
    let mut partial = Map::new();
    partial.insert(
        RISK_CONTROL_SETTINGS_KEY.to_string(),
        Value::Object(all_modes),
    );
    state.preferences.merge(partial).await?;

    Ok(code_ok(json!({
        "message": "风控配置已保存",
        "config": config
    })))
}
