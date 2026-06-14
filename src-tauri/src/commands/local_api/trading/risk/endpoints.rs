use super::super::*;
use super::metrics::{
    equities_from_snapshots, historical_var, max_drawdown, parametric_var, returns_from_equities,
    sharpe_ratio, sortino_ratio, std_dev,
};
use super::snapshots::portfolio_snapshots;

pub(crate) async fn risk_snapshots(state: &AppState, req: &LocalApiRequest) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let days = param_i64(req, "days", 90);
    Ok(code_ok(Value::Array(
        portfolio_snapshots(state, &mode, days).await?,
    )))
}

pub(crate) async fn risk_metrics(state: &AppState, req: &LocalApiRequest) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let days = param_i64(req, "days", 90).clamp(7, 365);
    let snapshots = portfolio_snapshots(state, &mode, days).await?;
    Ok(code_ok(metrics_payload(&snapshots)))
}

pub(crate) async fn risk_var(state: &AppState, req: &LocalApiRequest) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let confidence = param_f64(req, "confidence", 0.95).clamp(0.8, 0.999);
    let days = param_i64(req, "days", 90).clamp(7, 365);
    let snapshots = portfolio_snapshots(state, &mode, days).await?;
    let returns = returns_from_equities(&equities_from_snapshots(&snapshots));
    Ok(code_ok(var_payload(&returns, confidence)))
}

pub(crate) async fn risk_drawdown(state: &AppState, req: &LocalApiRequest) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let days = param_i64(req, "days", 90).clamp(7, 365);
    let snapshots = portfolio_snapshots(state, &mode, days).await?;
    Ok(code_ok(drawdown_payload_from_snapshots(&snapshots)))
}

pub(crate) async fn risk_rolling(state: &AppState, req: &LocalApiRequest) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let window = param_i64(req, "window", 30).clamp(5, 90) as usize;
    let days = param_i64(req, "days", 90).clamp(7, 365);
    let snapshots = portfolio_snapshots(state, &mode, days).await?;
    Ok(code_ok(rolling_payload_from_snapshots(&snapshots, window)))
}

pub(crate) async fn risk_overview(state: &AppState, req: &LocalApiRequest) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let snapshot_days = param_i64(req, "days", 90).clamp(1, 365) as usize;
    let analysis_days = param_i64(req, "days", 90).clamp(7, 365) as usize;
    let window = param_i64(req, "window", 30).clamp(5, 90) as usize;
    let load_days = snapshot_days.max(analysis_days) as i64;
    let snapshots = portfolio_snapshots(state, &mode, load_days).await?;
    Ok(code_ok(risk_overview_payload(
        &snapshots,
        snapshot_days,
        analysis_days,
        window,
    )))
}

fn metrics_payload(snapshots: &[Value]) -> Value {
    let equities = equities_from_snapshots(snapshots);
    let returns = returns_from_equities(&equities);
    if returns.len() < 2 {
        return json!({
            "has_data": false,
            "message": "权益快照不足，请先确保系统已运行数天并记录了每日快照",
            "data_points": equities.len()
        });
    }
    let drawdown = max_drawdown(&equities);
    json!({
        "has_data": true,
        "data_points": equities.len(),
        "var_95": historical_var(&returns, 0.95),
        "var_99": historical_var(&returns, 0.99),
        "parametric_var_95": parametric_var(&returns, 0.95),
        "sharpe_ratio": round4(sharpe_ratio(&returns)),
        "sortino_ratio": round4(sortino_ratio(&returns)),
        "max_drawdown": drawdown.max_drawdown,
        "max_drawdown_duration": drawdown.max_drawdown_duration,
        "current_drawdown": drawdown.current_drawdown,
        "peak_equity": drawdown.peak,
        "latest_equity": equities.last().copied().unwrap_or(0.0)
    })
}

fn drawdown_payload_from_snapshots(snapshots: &[Value]) -> Value {
    drawdown_payload(
        dates_from_snapshots(snapshots),
        equities_from_snapshots(snapshots),
    )
}

fn rolling_payload_from_snapshots(snapshots: &[Value], window: usize) -> Value {
    let returns = returns_from_equities(&equities_from_snapshots(snapshots));
    let dates = snapshots
        .iter()
        .skip(1)
        .filter_map(|item| {
            item.get("date")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .collect::<Vec<_>>();
    let mut sharpe = Vec::new();
    let mut volatility = Vec::new();
    let mut var_95 = Vec::new();
    for idx in 0..returns.len() {
        if idx + 1 < window {
            sharpe.push(Value::Null);
            volatility.push(Value::Null);
            var_95.push(Value::Null);
            continue;
        }
        let slice = &returns[(idx + 1 - window)..=idx];
        sharpe.push(json!(round4(sharpe_ratio(slice))));
        volatility.push(json!(round4(std_dev(slice) * 252.0_f64.sqrt())));
        var_95.push(json!(historical_var(slice, 0.95)));
    }
    json!({
        "dates": dates.into_iter().take(returns.len()).collect::<Vec<_>>(),
        "sharpe": sharpe,
        "volatility": volatility,
        "var_95": var_95
    })
}

fn risk_overview_payload(
    snapshots: &[Value],
    snapshot_days: usize,
    analysis_days: usize,
    window: usize,
) -> Value {
    let visible_snapshots = latest_snapshots(snapshots, snapshot_days);
    let analysis_snapshots = latest_snapshot_slice(snapshots, analysis_days);
    json!({
        "snapshots": visible_snapshots,
        "metrics": metrics_payload(analysis_snapshots),
        "drawdown": drawdown_payload_from_snapshots(analysis_snapshots),
        "rolling": rolling_payload_from_snapshots(analysis_snapshots, window)
    })
}

fn dates_from_snapshots(snapshots: &[Value]) -> Vec<String> {
    snapshots
        .iter()
        .filter_map(|item| {
            item.get("date")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .collect()
}

fn latest_snapshots(snapshots: &[Value], limit: usize) -> Vec<Value> {
    latest_snapshot_slice(snapshots, limit).to_vec()
}

fn latest_snapshot_slice(snapshots: &[Value], limit: usize) -> &[Value] {
    if limit == 0 {
        return &[];
    }
    let start = snapshots.len().saturating_sub(limit);
    &snapshots[start..]
}

fn var_payload(returns: &[f64], confidence: f64) -> Value {
    if returns.is_empty() {
        return json!({
            "has_data": false,
            "historical_var": null,
            "parametric_var": null,
            "confidence": confidence,
            "data_points": 0
        });
    }
    json!({
        "has_data": true,
        "historical_var": historical_var(returns, confidence),
        "parametric_var": parametric_var(returns, confidence),
        "confidence": confidence,
        "data_points": returns.len()
    })
}

fn drawdown_payload(dates: Vec<String>, equities: Vec<f64>) -> Value {
    if equities.is_empty() {
        return json!({
            "dates": dates,
            "equities": equities,
            "max_drawdown": null,
            "max_drawdown_duration": null,
            "current_drawdown": null,
            "peak": null,
            "series": []
        });
    }
    let drawdown = max_drawdown(&equities);
    json!({
        "dates": dates,
        "equities": equities,
        "max_drawdown": drawdown.max_drawdown,
        "max_drawdown_duration": drawdown.max_drawdown_duration,
        "current_drawdown": drawdown.current_drawdown,
        "peak": drawdown.peak,
        "series": drawdown.series
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn var_payload_keeps_empty_returns_unknown() {
        let payload = var_payload(&[], 0.95);

        assert_eq!(payload["confidence"], 0.95);
        assert_eq!(payload["data_points"], 0);
        assert_eq!(payload["has_data"], false);
        assert!(payload["historical_var"].is_null());
        assert!(payload["parametric_var"].is_null());
    }

    #[test]
    fn drawdown_payload_keeps_empty_equity_unknown() {
        let payload = drawdown_payload(Vec::new(), Vec::new());

        assert_eq!(payload["dates"].as_array().unwrap().len(), 0);
        assert_eq!(payload["equities"].as_array().unwrap().len(), 0);
        assert!(payload["max_drawdown"].is_null());
        assert!(payload["max_drawdown_duration"].is_null());
        assert!(payload["current_drawdown"].is_null());
        assert!(payload["peak"].is_null());
        assert_eq!(payload["series"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn risk_overview_payload_matches_existing_risk_endpoint_payloads() {
        let snapshots = sample_snapshots(10);
        let snapshot_days = 3;
        let analysis_days = 7;
        let window = 5;
        let overview = risk_overview_payload(&snapshots, snapshot_days, analysis_days, window);
        let analysis_snapshots = latest_snapshot_slice(&snapshots, analysis_days);

        assert_eq!(
            overview["snapshots"],
            Value::Array(latest_snapshots(&snapshots, snapshot_days))
        );
        assert_eq!(overview["metrics"], metrics_payload(analysis_snapshots));
        assert_eq!(
            overview["drawdown"],
            drawdown_payload_from_snapshots(analysis_snapshots)
        );
        assert_eq!(
            overview["rolling"],
            rolling_payload_from_snapshots(analysis_snapshots, window)
        );
    }

    fn sample_snapshots(count: usize) -> Vec<Value> {
        (1..=count)
            .map(|day| {
                json!({
                    "mode": "simulated",
                    "date": format!("2026-05-{day:02}T00:00:00.000Z"),
                    "total_equity": 10_000.0 + day as f64 * 37.0,
                    "spot_value": 2_000.0,
                    "contract_value": 3_000.0,
                    "cash_value": 5_000.0,
                    "positions": {},
                    "metadata": {},
                    "created_at": format!("2026-05-{day:02}T00:00:00.000Z")
                })
            })
            .collect()
    }
}
