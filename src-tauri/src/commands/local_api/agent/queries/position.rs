use serde_json::{json, Value};

use crate::{
    app_state::AppState,
    commands::local_api::{
        infer_inst_type, okx_private_client, request_trading_mode, LocalApiRequest,
    },
    error::{AppError, AppResult},
    okx::{
        okx_finite_value, okx_position_side_label, okx_positive_value, okx_signed_position,
        okx_value_text,
    },
};

/// POST /api/agent/query/position
pub(crate) async fn query_position(state: &AppState, req: &LocalApiRequest) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    query_position_snapshot(state, &mode).await
}

pub(super) async fn query_position_snapshot(state: &AppState, mode: &str) -> AppResult<Value> {
    let client = okx_private_client(state, mode).await?;
    let items = client.get_positions(None, None).await?;
    position_snapshot_payload(mode, &items)
}

fn position_snapshot_payload(mode: &str, items: &[Value]) -> AppResult<Value> {
    let positions = positions_from_okx_items(mode, items)?;
    Ok(json!({
        "mode": mode,
        "source": "okx_account_positions",
        "count": positions.len(),
        "positions": positions,
    }))
}

fn positions_from_okx_items(mode: &str, items: &[Value]) -> AppResult<Vec<Value>> {
    let mut positions = Vec::new();
    for item in items {
        let symbol = okx_value_text(item, "instId").trim().to_uppercase();
        if symbol.is_empty() {
            continue;
        }
        let raw_pos = okx_finite_value(item, "pos").ok_or_else(|| {
            AppError::Runtime(format!(
                "OKX 持仓 {symbol} 缺少有效 pos，无法生成当前持仓摘要"
            ))
        })?;
        if raw_pos.abs() <= f64::EPSILON {
            continue;
        }
        let position = okx_signed_position(item, raw_pos);
        let inst_type = okx_value_text(item, "instType").trim().to_uppercase();
        let inst_type = if inst_type.is_empty() {
            infer_inst_type(&symbol)
        } else {
            inst_type
        };
        let avg_price = okx_positive_value(item, "avgPx");
        let mark_price = okx_positive_value(item, "markPx");
        positions.push(json!({
            "symbol": symbol,
            "inst_type": inst_type,
            "mode": mode,
            "position": position,
            "pos_side": okx_position_side_label(item, position),
            "avg_price": optional_number(avg_price),
            "mark_price": optional_number(mark_price),
            "unrealized_pnl": optional_number(okx_finite_value(item, "upl")),
            "unrealized_pnl_pct": optional_number(okx_finite_value(item, "uplRatio")),
            "margin": optional_number(okx_positive_value(item, "margin")),
            "lever": optional_number(okx_positive_value(item, "lever")),
            "source": "okx_account_positions",
        }));
    }
    Ok(positions)
}

fn optional_number(value: Option<f64>) -> Value {
    value
        .filter(|item| item.is_finite())
        .map(Value::from)
        .unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn position_snapshot_uses_okx_positions_as_current_state() {
        let payload = super::position_snapshot_payload(
            "simulated",
            &[json!({
                "instId": "BTC-USDT-SWAP",
                "instType": "SWAP",
                "pos": "2",
                "posSide": "long",
                "avgPx": "100",
                "markPx": "120",
                "upl": "40",
                "uplRatio": "0.2",
                "margin": "50",
                "lever": "3"
            })],
        )
        .expect("OKX position payload should normalize");

        assert_eq!(payload["source"], "okx_account_positions");
        assert_eq!(payload["count"], 1);
        assert_eq!(payload["positions"][0]["symbol"], "BTC-USDT-SWAP");
        assert_eq!(payload["positions"][0]["inst_type"], "SWAP");
        assert_eq!(payload["positions"][0]["mode"], "simulated");
        assert_eq!(payload["positions"][0]["position"], 2.0);
        assert_eq!(payload["positions"][0]["pos_side"], "long");
        assert_eq!(payload["positions"][0]["avg_price"], 100.0);
        assert_eq!(payload["positions"][0]["mark_price"], 120.0);
        assert_eq!(payload["positions"][0]["unrealized_pnl"], 40.0);
        assert_eq!(payload["positions"][0]["unrealized_pnl_pct"], 0.2);
        assert_eq!(payload["positions"][0]["margin"], 50.0);
        assert_eq!(payload["positions"][0]["lever"], 3.0);
    }

    #[test]
    fn position_snapshot_signs_short_positions_from_okx_pos_side() {
        let payload = super::position_snapshot_payload(
            "live",
            &[json!({
                "instId": "ETH-USDT-SWAP",
                "instType": "SWAP",
                "pos": "3",
                "posSide": "short",
                "avgPx": "2000",
                "markPx": "1900"
            })],
        )
        .expect("OKX short position should normalize");

        assert_eq!(payload["positions"][0]["position"], -3.0);
        assert_eq!(payload["positions"][0]["pos_side"], "short");
    }

    #[test]
    fn position_snapshot_skips_zero_okx_positions() {
        let payload = super::position_snapshot_payload(
            "simulated",
            &[json!({
                "instId": "BTC-USDT-SWAP",
                "instType": "SWAP",
                "pos": "0",
                "posSide": "long",
                "avgPx": "100",
                "markPx": "120"
            })],
        )
        .expect("zero OKX position should be ignored");

        assert_eq!(payload["count"], 0);
        assert_eq!(payload["positions"].as_array().map(Vec::len), Some(0));
    }

    #[test]
    fn position_snapshot_keeps_invalid_okx_prices_unknown() {
        let payload = super::position_snapshot_payload(
            "simulated",
            &[json!({
                "instId": "BTC-USDT-SWAP",
                "pos": "2",
                "posSide": "long",
                "avgPx": "bad-avg",
                "markPx": "bad-mark",
                "upl": "bad-upl",
                "uplRatio": "bad-ratio",
                "margin": "bad-margin",
                "lever": "bad-lever"
            })],
        )
        .expect("invalid optional OKX economics should not block current quantity");

        assert_eq!(payload["positions"][0]["position"], 2.0);
        assert!(payload["positions"][0]["avg_price"].is_null());
        assert!(payload["positions"][0]["mark_price"].is_null());
        assert!(payload["positions"][0]["unrealized_pnl"].is_null());
        assert!(payload["positions"][0]["unrealized_pnl_pct"].is_null());
        assert!(payload["positions"][0]["margin"].is_null());
        assert!(payload["positions"][0]["lever"].is_null());
    }

    #[test]
    fn position_snapshot_rejects_active_okx_position_without_valid_pos() {
        let error = super::position_snapshot_payload(
            "simulated",
            &[json!({
                "instId": "BTC-USDT-SWAP",
                "pos": "bad-pos",
                "posSide": "long"
            })],
        )
        .expect_err("active OKX row without valid pos must not become a fake flat position");

        assert!(error.to_string().contains("缺少有效 pos"));
    }
}
