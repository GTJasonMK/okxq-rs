use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use sqlx::SqlitePool;

use super::{
    input::{infer_inst_type, normalize_inst_id, now_text},
    storage,
    types::{PriceAlert, TickerSnapshot},
};

pub async fn evaluate_ticker(pool: &SqlitePool, ticker: TickerSnapshot) -> Result<Vec<Value>> {
    if !ticker.last_price.is_finite() || ticker.last_price <= 0.0 {
        return Err(anyhow!("last_price 必须是大于 0 的有效价格"));
    }
    let inst_id = normalize_inst_id(&ticker.inst_id)?;
    let inst_type = infer_inst_type(&ticker.inst_id, &ticker.inst_type);
    let alerts = storage::active_alerts_for_ticker(pool, &inst_id, &inst_type).await?;

    let now_ms = chrono::Utc::now().timestamp_millis();
    let mut triggered = Vec::new();
    for mut alert in alerts {
        let current_value = if alert.alert_type == "price" {
            Some(ticker.last_price)
        } else {
            ticker.change_24h
        };
        let Some(current_value) = current_value.filter(|value| value.is_finite()) else {
            continue;
        };
        let Some(threshold) = threshold_value(&alert) else {
            continue;
        };
        let previous_value = alert.last_value;
        let crossed = match alert.direction.as_str() {
            "above" => {
                current_value >= threshold
                    && previous_value
                        .map(|value| value < threshold)
                        .unwrap_or(true)
            }
            "below" => {
                current_value <= threshold
                    && previous_value
                        .map(|value| value > threshold)
                        .unwrap_or(true)
            }
            _ => false,
        };

        let previous_stored_value = alert.last_value;
        alert.last_value = Some(current_value);
        let cooldown_ms = alert.cooldown_seconds.max(0) * 1000;
        let cooldown_ready =
            cooldown_ms == 0 || now_ms.saturating_sub(alert.last_trigger_ts) >= cooldown_ms;

        if crossed && cooldown_ready {
            let triggered_at = now_text();
            alert.last_trigger_ts = now_ms;
            alert.last_trigger_value = Some(current_value);
            alert.triggered_at = Some(triggered_at.clone());
            alert.updated_at = triggered_at;
            if alert.trigger_once {
                alert.enabled = false;
            }
            triggered.push(build_trigger_payload(
                &alert,
                current_value,
                ticker.last_price,
                ticker.change_24h,
                ticker.ticker_ts,
            ));
            storage::persist_alert(pool, &alert).await?;
        } else if previous_stored_value != alert.last_value {
            storage::persist_alert(pool, &alert).await?;
        }
    }

    Ok(triggered)
}

fn threshold_value(alert: &PriceAlert) -> Option<f64> {
    if alert.alert_type == "price" {
        alert.target_price
    } else {
        alert.change_percent
    }
}

fn build_trigger_payload(
    alert: &PriceAlert,
    current_value: f64,
    last_price: f64,
    change_24h: Option<f64>,
    ticker_ts: i64,
) -> Value {
    let is_price_alert = alert.alert_type == "price";
    let target_value = threshold_value(alert).unwrap_or(0.0);
    let direction_text = if alert.direction == "above" {
        "上破"
    } else {
        "下破"
    };
    let market_label = if alert.inst_type == "SWAP" {
        "永续合约"
    } else {
        "现货"
    };
    let mut message = if is_price_alert {
        format!(
            "{}（{}）{}{:.4}USDT",
            alert.inst_id, market_label, direction_text, target_value
        )
    } else {
        format!(
            "{}（{}）24H 涨跌幅{}{:.2}%",
            alert.inst_id, market_label, direction_text, target_value
        )
    };
    if !alert.note.is_empty() {
        message = format!("{message}｜备注：{}", alert.note);
    }

    json!({
        "id": alert.id,
        "inst_id": alert.inst_id,
        "symbol": alert.symbol,
        "inst_type": alert.inst_type,
        "alert_type": alert.alert_type,
        "direction": alert.direction,
        "target_value": target_value,
        "current_value": current_value,
        "last_price": last_price,
        "change_24h": change_24h,
        "triggered_at": alert.triggered_at,
        "ticker_ts": ticker_ts,
        "title": format!("{} 价格提醒", alert.inst_id),
        "message": message,
        "note": alert.note
    })
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;
    use crate::alerts::{storage, types::PriceAlert};

    #[tokio::test]
    async fn price_alert_rejects_zero_last_price_instead_of_triggering() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory sqlite");
        sqlx::query(
            r#"
            CREATE TABLE price_alerts (
              id TEXT PRIMARY KEY,
              inst_id TEXT NOT NULL,
              symbol TEXT NOT NULL,
              inst_type TEXT NOT NULL,
              alert_type TEXT NOT NULL,
              direction TEXT NOT NULL,
              target_price REAL,
              change_percent REAL,
              note TEXT DEFAULT '',
              enabled INTEGER DEFAULT 1,
              trigger_once INTEGER DEFAULT 1,
              cooldown_seconds INTEGER DEFAULT 300,
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL,
              triggered_at TEXT,
              last_value REAL,
              last_trigger_value REAL,
              last_trigger_ts INTEGER DEFAULT 0
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create price_alerts");

        storage::persist_alert(
            &pool,
            &PriceAlert {
                id: "pa_zero_last".to_string(),
                inst_id: "BTC-USDT-SWAP".to_string(),
                symbol: "BTC-USDT".to_string(),
                inst_type: "SWAP".to_string(),
                alert_type: "price".to_string(),
                direction: "below".to_string(),
                target_price: Some(100.0),
                change_percent: None,
                note: String::new(),
                enabled: true,
                trigger_once: true,
                cooldown_seconds: 0,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:00:00Z".to_string(),
                triggered_at: None,
                last_value: Some(120.0),
                last_trigger_value: None,
                last_trigger_ts: 0,
            },
        )
        .await
        .expect("persist alert");

        let result = evaluate_ticker(
            &pool,
            TickerSnapshot {
                inst_id: "BTC-USDT-SWAP".to_string(),
                inst_type: "SWAP".to_string(),
                last_price: 0.0,
                change_24h: None,
                ticker_ts: 1_700_000_000_000,
            },
        )
        .await;

        assert!(result.is_err());
    }
}
