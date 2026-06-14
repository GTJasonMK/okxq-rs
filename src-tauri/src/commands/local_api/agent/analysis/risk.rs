use serde_json::{json, Value};

use crate::{
    app_state::AppState,
    commands::local_api::{
        body_i64, code_ok, okx_private_client, request_trading_mode, round2, LocalApiRequest,
    },
    error::{AppError, AppResult},
    okx::{
        okx_account_equity, okx_finite_value, okx_position_notional, okx_positive_value,
        okx_signed_position, okx_value_text,
    },
};

/// POST /api/agent/analysis/risk-budget
pub(crate) async fn analyze_risk_budget(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let risk_per_trade = body_i64(req, "risk_percent", 2).clamp(1, 10) as f64 / 100.0;
    let max_position_pct = body_i64(req, "max_position_pct", 50).clamp(10, 100) as f64 / 100.0;
    let client = okx_private_client(state, &mode).await?;
    let account_items = client.get_account_balance().await?;
    let account_equity = okx_account_equity(&account_items).ok_or_else(|| {
        AppError::Validation("OKX 账户余额未返回有效权益，无法计算风险预算".to_string())
    })?;
    let position_items = client.get_positions(None, None).await?;
    let positions = risk_budget_positions_from_okx(&position_items)?;

    Ok(code_ok(risk_budget_payload(
        mode.as_str(),
        risk_per_trade,
        max_position_pct,
        Some(account_equity),
        &positions,
    )))
}

#[derive(Clone, Debug)]
struct RiskBudgetPosition {
    symbol: String,
    position: f64,
    avg_price: Option<f64>,
    mark_price: Option<f64>,
    exposure: f64,
}

fn risk_budget_payload(
    mode: &str,
    risk_per_trade: f64,
    max_position_pct: f64,
    account_equity: Option<f64>,
    positions: &[RiskBudgetPosition],
) -> Value {
    let mut current_exposure = 0.0;
    let mut pos_details = Vec::new();
    for position in positions {
        if position.position == 0.0 || !position.exposure.is_finite() || position.exposure <= 0.0 {
            continue;
        }
        current_exposure += position.exposure;
        pos_details.push(json!({
            "symbol": position.symbol,
            "position": position.position,
            "avg_price": optional_number(position.avg_price),
            "mark_price": optional_number(position.mark_price),
            "exposure": round2(position.exposure),
        }));
    }

    let total_value = account_equity.filter(|value| value.is_finite() && *value > 0.0);
    let max_risk_amount = total_value.map(|value| value * risk_per_trade);
    let max_position_value = total_value.map(|value| value * max_position_pct);
    let remaining_capital = max_position_value.map(|value| (value - current_exposure).max(0.0));

    json!({
        "mode": mode,
        "estimated_total_value": round2_optional(total_value),
        "risk_per_trade_pct": round2(risk_per_trade * 100.0),
        "max_risk_amount": round2_optional(max_risk_amount),
        "max_position_value": round2_optional(max_position_value),
        "current_exposure": round2(current_exposure),
        "remaining_capital": round2_optional(remaining_capital),
        "current_positions": pos_details,
    })
}

fn risk_budget_positions_from_okx(items: &[Value]) -> AppResult<Vec<RiskBudgetPosition>> {
    let mut positions = Vec::new();
    for item in items {
        let symbol = okx_value_text(item, "instId").trim().to_uppercase();
        if symbol.is_empty() {
            continue;
        }
        let pos = okx_finite_value(item, "pos").ok_or_else(|| {
            AppError::Runtime(format!("OKX 持仓 {symbol} 缺少有效 pos，无法计算风险预算"))
        })?;
        if pos.abs() <= f64::EPSILON {
            continue;
        }
        let position = okx_signed_position(item, pos);
        let avg_price = okx_positive_value(item, "avgPx");
        let mark_price = okx_positive_value(item, "markPx");
        let exposure =
            okx_position_notional(item, position.abs(), &["markPx"]).ok_or_else(|| {
                AppError::Runtime(format!(
                    "OKX 持仓 {symbol} 缺少有效 notionalUsd/notional/markPx，无法计算风险预算"
                ))
            })?;
        positions.push(RiskBudgetPosition {
            symbol,
            position,
            avg_price,
            mark_price,
            exposure,
        });
    }
    Ok(positions)
}

fn optional_number(value: Option<f64>) -> Value {
    value
        .filter(|item| item.is_finite())
        .map(Value::from)
        .unwrap_or(Value::Null)
}

fn round2_optional(value: Option<f64>) -> Value {
    value.map(round2).map(Value::from).unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    #[test]
    fn risk_budget_uses_mark_price_for_current_exposure() {
        let positions = vec![super::RiskBudgetPosition {
            symbol: "BTC-USDT-SWAP".to_string(),
            position: 2.0,
            avg_price: Some(100.0),
            mark_price: Some(120.0),
            exposure: 240.0,
        }];

        let payload = super::risk_budget_payload("simulated", 0.02, 0.5, Some(1_000.0), &positions);

        assert_eq!(payload["current_exposure"], 240.0);
        assert_eq!(payload["remaining_capital"], 260.0);
        assert_eq!(payload["current_positions"][0]["avg_price"], 100.0);
        assert_eq!(payload["current_positions"][0]["mark_price"], 120.0);
        assert_eq!(payload["current_positions"][0]["exposure"], 240.0);
    }

    #[test]
    fn risk_budget_uses_okx_positions_for_current_exposure() {
        let okx_positions = vec![serde_json::json!({
            "instId": "BTC-USDT-SWAP",
            "instType": "SWAP",
            "pos": "2",
            "posSide": "long",
            "avgPx": "100",
            "markPx": "120",
            "notionalUsd": "240"
        })];

        let positions = super::risk_budget_positions_from_okx(&okx_positions)
            .expect("OKX positions should normalize");
        let payload = super::risk_budget_payload("simulated", 0.02, 0.5, Some(1_000.0), &positions);

        assert_eq!(payload["current_exposure"], 240.0);
        assert_eq!(payload["remaining_capital"], 260.0);
        assert_eq!(payload["current_positions"][0]["symbol"], "BTC-USDT-SWAP");
        assert_eq!(payload["current_positions"][0]["position"], 2.0);
        assert_eq!(payload["current_positions"][0]["avg_price"], 100.0);
        assert_eq!(payload["current_positions"][0]["mark_price"], 120.0);
        assert_eq!(payload["current_positions"][0]["exposure"], 240.0);
    }

    #[test]
    fn risk_budget_okx_position_uses_mark_price_when_notional_is_missing() {
        let okx_positions = vec![serde_json::json!({
            "instId": "BTC-USDT-SWAP",
            "pos": "2",
            "posSide": "short",
            "avgPx": "100",
            "markPx": "120"
        })];

        let positions = super::risk_budget_positions_from_okx(&okx_positions)
            .expect("OKX position should fall back to markPx notional");

        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].symbol, "BTC-USDT-SWAP");
        assert_eq!(positions[0].position, -2.0);
        assert_eq!(positions[0].exposure, 240.0);
    }

    #[test]
    fn risk_budget_does_not_fabricate_account_equity_when_balance_is_unknown() {
        let payload = super::risk_budget_payload("simulated", 0.02, 0.5, None, &[]);

        assert!(payload["estimated_total_value"].is_null());
        assert!(payload["max_risk_amount"].is_null());
        assert!(payload["max_position_value"].is_null());
        assert!(payload["remaining_capital"].is_null());
        assert_eq!(payload["current_exposure"], 0.0);
    }

    #[test]
    fn risk_budget_account_equity_uses_okx_total_or_detail_equity() {
        let total = super::okx_account_equity(&[serde_json::json!({
            "totalEq": "1234.56",
            "details": [{"ccy": "USDT", "eqUsd": "1000"}]
        })]);
        let details = super::okx_account_equity(&[serde_json::json!({
            "details": [
                {"ccy": "USDT", "eqUsd": "900"},
                {"ccy": "BTC", "disEq": "120.5"}
            ]
        })]);
        let invalid = super::okx_account_equity(&[serde_json::json!({
            "totalEq": "bad",
            "details": [{"ccy": "USDT", "eqUsd": "bad"}]
        })]);

        assert_eq!(total, Some(1234.56));
        assert_eq!(details, Some(1020.5));
        assert_eq!(invalid, None);
    }
}
