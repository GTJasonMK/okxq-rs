use futures_util::TryStreamExt;
use serde_json::Value;
use sqlx::{Row, SqlitePool};

use crate::ohlcv::Ohlcv;

#[derive(Clone, Copy)]
pub(crate) enum FeatureBarTimestampMode {
    Any,
    Positive,
}

pub(crate) struct FeatureBarRow {
    pub ts: i64,
    pub payload: Value,
    pub ohlcv: Ohlcv,
}

pub(crate) async fn load_latest_feature_bar_rows(
    db: &SqlitePool,
    inst_id: &str,
    limit: i64,
    timestamp_mode: FeatureBarTimestampMode,
) -> Result<Vec<FeatureBarRow>, sqlx::Error> {
    let target = limit.max(1) as usize;
    let mut rows = sqlx::query(
        "SELECT ts, payload_json FROM feature_bars_1s WHERE inst_id = ? ORDER BY ts DESC",
    )
    .bind(inst_id)
    .fetch(db);

    let mut bars = Vec::with_capacity(target);
    while bars.len() < target {
        let Some(row) = rows.try_next().await? else {
            break;
        };
        let ts = row.try_get::<i64, _>("ts").unwrap_or_default();
        if matches!(timestamp_mode, FeatureBarTimestampMode::Positive) && ts <= 0 {
            continue;
        }
        let text: String = row.try_get("payload_json").unwrap_or_default();
        let Ok(payload) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let Some(ohlcv) = feature_ohlcv_from_payload(&payload) else {
            continue;
        };
        bars.push(FeatureBarRow { ts, payload, ohlcv });
    }
    bars.reverse();
    Ok(bars)
}

pub(crate) fn feature_ohlcv_from_payload(payload: &Value) -> Option<Ohlcv> {
    Some(Ohlcv {
        open: positive_feature_payload_f64(payload, "open")?,
        high: positive_feature_payload_f64(payload, "high")?,
        low: positive_feature_payload_f64(payload, "low")?,
        close: positive_feature_payload_f64(payload, "close")?,
        volume: non_negative_feature_payload_f64(payload, "volume").unwrap_or(0.0),
    })
}

pub(crate) fn non_negative_feature_payload_f64(payload: &Value, key: &str) -> Option<f64> {
    let value = payload.get(key).and_then(Value::as_f64)?;
    (value.is_finite() && value >= 0.0).then_some(value)
}

fn positive_feature_payload_f64(payload: &Value, key: &str) -> Option<f64> {
    let value = payload.get(key).and_then(Value::as_f64)?;
    (value.is_finite() && value > 0.0).then_some(value)
}
