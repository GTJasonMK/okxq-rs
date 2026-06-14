use serde_json::{json, Value};

use crate::{
    app_state::AppState,
    commands::local_api::{
        body_i64, body_string, code_ok, infer_inst_type, okx_client, request_string_array,
        LocalApiRequest,
    },
    error::{AppError, AppResult},
};

use super::super::scope::{load_agent_candles, load_latest_candles_for_scope, resolve_agent_scope};

/// POST /api/agent/query/market-snapshot
pub(crate) async fn query_market_snapshot(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let inst_id = body_string(req, "inst_id", "");
    let inst_type = body_string(req, "inst_type", &infer_inst_type(&inst_id));
    if inst_id.is_empty() {
        return Err(AppError::Validation("inst_id 不能为空".to_string()));
    }
    let (inst_id, inst_type) = resolve_agent_scope(state, &inst_id, &inst_type).await?;

    let ticker = if let Ok(client) = okx_client(state).await {
        client.get_ticker(&inst_id).await.ok()
    } else {
        None
    };

    let candle = load_latest_candles_for_scope(state, &inst_id, &inst_type, "1H", 1)
        .await?
        .last()
        .map(|candle| candle.to_json());

    let last_price = resolve_market_snapshot_last_price(ticker.as_ref(), candle.as_ref());

    Ok(code_ok(json!({
        "inst_id": inst_id,
        "inst_type": inst_type,
        "last_price": last_price,
        "ticker": ticker,
        "latest_candle": candle,
    })))
}

fn resolve_market_snapshot_last_price(
    ticker: Option<&Value>,
    candle: Option<&Value>,
) -> Option<f64> {
    ticker
        .and_then(|t| t.get("last"))
        .and_then(positive_json_f64)
        .or_else(|| {
            candle
                .and_then(|c| c.get("close"))
                .and_then(positive_json_f64)
        })
}

fn positive_json_f64(value: &Value) -> Option<f64> {
    let parsed = value
        .as_f64()
        .or_else(|| value.as_str().and_then(|item| item.parse::<f64>().ok()))?;
    (parsed.is_finite() && parsed > 0.0).then_some(parsed)
}

/// POST /api/agent/query/candles
pub(crate) async fn query_candles(state: &AppState, req: &LocalApiRequest) -> AppResult<Value> {
    let inst_id = body_string(req, "inst_id", "");
    let inst_type = body_string(req, "inst_type", &infer_inst_type(&inst_id));
    let timeframes = request_string_array(req, "timeframes");
    let limit = body_i64(req, "limit", 300).clamp(20, 5000);
    if inst_id.is_empty() {
        return Err(AppError::Validation("inst_id 不能为空".to_string()));
    }

    let tf_list: Vec<String> = if timeframes.is_empty() {
        vec!["1H".to_string()]
    } else {
        timeframes
    };

    let (inst_id, inst_type) = resolve_agent_scope(state, &inst_id, &inst_type).await?;
    let mut result = Vec::new();
    for tf in &tf_list {
        let candles: Vec<Value> =
            load_latest_candles_for_scope(state, &inst_id, &inst_type, tf, limit)
                .await?
                .into_iter()
                .map(|candle| candle.to_json())
                .collect();

        result.push(json!({
            "timeframe": tf,
            "inst_id": inst_id,
            "inst_type": inst_type,
            "count": candles.len(),
            "candles": candles,
        }));
    }

    Ok(code_ok(json!({
        "inst_id": inst_id,
        "timeframes": result,
    })))
}

/// POST /api/agent/query/indicators
pub(crate) async fn query_indicators(state: &AppState, req: &LocalApiRequest) -> AppResult<Value> {
    let inst_id = body_string(req, "inst_id", "");
    let inst_type = body_string(req, "inst_type", &infer_inst_type(&inst_id));
    let timeframe = body_string(req, "timeframe", "1H");
    let limit = body_i64(req, "limit", 300).clamp(20, 5000);
    if inst_id.is_empty() {
        return Err(AppError::Validation("inst_id 不能为空".to_string()));
    }

    let (inst_id, inst_type, candles) =
        load_agent_candles(state, &inst_id, &inst_type, &timeframe, limit).await?;

    let indicators = crate::indicators::IndicatorCalculator::new(&candles).calculate_all();
    let requested_indicators = request_string_array(req, "indicators");

    let filtered = if requested_indicators.is_empty() {
        indicators
    } else if let Some(obj) = indicators.as_object() {
        let mut filtered_map = serde_json::Map::new();
        for key in &requested_indicators {
            if let Some(value) = obj.get(key) {
                filtered_map.insert(key.clone(), value.clone());
            }
        }
        Value::Object(filtered_map)
    } else {
        indicators
    };

    let close_value = resolve_indicator_last_close(&candles);
    Ok(code_ok(json!({
        "inst_id": inst_id,
        "inst_type": inst_type,
        "timeframe": timeframe,
        "last_close": close_value,
        "indicators": filtered,
    })))
}

fn resolve_indicator_last_close(candles: &[crate::okx::OkxCandle]) -> Option<f64> {
    candles
        .last()
        .map(|candle| candle.close)
        .filter(|close| close.is_finite() && *close > 0.0)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{resolve_indicator_last_close, resolve_market_snapshot_last_price};

    #[test]
    fn market_snapshot_last_price_is_not_fabricated_from_invalid_inputs() {
        let ticker = json!({"last": "bad-last"});
        let candle = json!({"close": "bad-close"});

        let last_price = resolve_market_snapshot_last_price(Some(&ticker), Some(&candle));

        assert!(last_price.is_none());
    }

    #[test]
    fn market_snapshot_last_price_falls_back_to_valid_candle_close() {
        let ticker = json!({"last": "0"});
        let candle = json!({"close": 100.5});

        let last_price = resolve_market_snapshot_last_price(Some(&ticker), Some(&candle));

        assert_eq!(last_price, Some(100.5));
    }

    #[test]
    fn indicator_last_close_is_not_fabricated_for_empty_candles() {
        let candles = Vec::new();

        let last_close = resolve_indicator_last_close(&candles);

        assert!(last_close.is_none());
    }

    #[test]
    fn indicator_last_close_keeps_valid_close() {
        let candles = vec![crate::okx::OkxCandle {
            timestamp: 1700000000000,
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close: 100.5,
            volume: 12.0,
            volume_ccy: 1200.0,
            volume_quote: 1200.0,
            confirm: String::new(),
        }];

        let last_close = resolve_indicator_last_close(&candles);

        assert_eq!(last_close, Some(100.5));
    }
}
