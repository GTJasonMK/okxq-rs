use anyhow::Result;
use sqlx::{sqlite::SqliteRow, Row, SqlitePool};

use super::types::PriceAlert;

const PRICE_ALERT_SELECT: &str = r#"
    SELECT id, inst_id, symbol, inst_type, alert_type, direction,
           target_price, change_percent, note, enabled, trigger_once,
           cooldown_seconds, created_at, updated_at, triggered_at,
           last_value, last_trigger_value, last_trigger_ts
    FROM price_alerts
"#;

pub(in crate::alerts) async fn list_alerts(
    pool: &SqlitePool,
    inst_id: Option<String>,
    inst_type: Option<String>,
) -> Result<Vec<PriceAlert>> {
    let rows = match (inst_id, inst_type) {
        (Some(inst_id), Some(inst_type)) => {
            sqlx::query(&format!(
                "{PRICE_ALERT_SELECT} WHERE inst_id = ? AND inst_type = ? ORDER BY created_at DESC"
            ))
            .bind(inst_id)
            .bind(inst_type)
            .fetch_all(pool)
            .await?
        }
        (Some(inst_id), None) => {
            sqlx::query(&format!(
                "{PRICE_ALERT_SELECT} WHERE inst_id = ? ORDER BY created_at DESC"
            ))
            .bind(inst_id)
            .fetch_all(pool)
            .await?
        }
        (None, Some(inst_type)) => {
            sqlx::query(&format!(
                "{PRICE_ALERT_SELECT} WHERE inst_type = ? ORDER BY created_at DESC"
            ))
            .bind(inst_type)
            .fetch_all(pool)
            .await?
        }
        (None, None) => {
            sqlx::query(&format!("{PRICE_ALERT_SELECT} ORDER BY created_at DESC"))
                .fetch_all(pool)
                .await?
        }
    };

    rows.into_iter().map(price_alert_from_row).collect()
}

pub(in crate::alerts) async fn active_alerts_for_ticker(
    pool: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
) -> Result<Vec<PriceAlert>> {
    let rows = sqlx::query(&format!(
        "{PRICE_ALERT_SELECT} WHERE enabled = 1 AND inst_id = ? AND inst_type = ? ORDER BY created_at DESC"
    ))
    .bind(inst_id)
    .bind(inst_type)
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(price_alert_from_row).collect()
}

pub(in crate::alerts) async fn get_alert(
    pool: &SqlitePool,
    alert_id: &str,
) -> Result<Option<PriceAlert>> {
    let row = sqlx::query(&format!("{PRICE_ALERT_SELECT} WHERE id = ?"))
        .bind(alert_id)
        .fetch_optional(pool)
        .await?;
    row.map(price_alert_from_row).transpose()
}

pub(in crate::alerts) async fn persist_alert(pool: &SqlitePool, alert: &PriceAlert) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO price_alerts (
          id, inst_id, symbol, inst_type, alert_type, direction,
          target_price, change_percent, note, enabled, trigger_once,
          cooldown_seconds, created_at, updated_at, triggered_at,
          last_value, last_trigger_value, last_trigger_ts
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
          inst_id = excluded.inst_id,
          symbol = excluded.symbol,
          inst_type = excluded.inst_type,
          alert_type = excluded.alert_type,
          direction = excluded.direction,
          target_price = excluded.target_price,
          change_percent = excluded.change_percent,
          note = excluded.note,
          enabled = excluded.enabled,
          trigger_once = excluded.trigger_once,
          cooldown_seconds = excluded.cooldown_seconds,
          updated_at = excluded.updated_at,
          triggered_at = excluded.triggered_at,
          last_value = excluded.last_value,
          last_trigger_value = excluded.last_trigger_value,
          last_trigger_ts = excluded.last_trigger_ts
        "#,
    )
    .bind(&alert.id)
    .bind(&alert.inst_id)
    .bind(&alert.symbol)
    .bind(&alert.inst_type)
    .bind(&alert.alert_type)
    .bind(&alert.direction)
    .bind(alert.target_price)
    .bind(alert.change_percent)
    .bind(&alert.note)
    .bind(bool_i64(alert.enabled))
    .bind(bool_i64(alert.trigger_once))
    .bind(alert.cooldown_seconds)
    .bind(&alert.created_at)
    .bind(&alert.updated_at)
    .bind(&alert.triggered_at)
    .bind(alert.last_value)
    .bind(alert.last_trigger_value)
    .bind(alert.last_trigger_ts)
    .execute(pool)
    .await?;
    Ok(())
}

pub(in crate::alerts) async fn delete_alert(pool: &SqlitePool, alert_id: &str) -> Result<bool> {
    let result = sqlx::query("DELETE FROM price_alerts WHERE id = ?")
        .bind(alert_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

fn price_alert_from_row(row: SqliteRow) -> Result<PriceAlert> {
    Ok(PriceAlert {
        id: row.try_get("id")?,
        inst_id: row.try_get("inst_id")?,
        symbol: row.try_get("symbol")?,
        inst_type: row.try_get("inst_type")?,
        alert_type: row.try_get("alert_type")?,
        direction: row.try_get("direction")?,
        target_price: row.try_get::<Option<f64>, _>("target_price").ok().flatten(),
        change_percent: row
            .try_get::<Option<f64>, _>("change_percent")
            .ok()
            .flatten(),
        note: row.try_get("note")?,
        enabled: row.try_get::<i64, _>("enabled").unwrap_or(1) != 0,
        trigger_once: row.try_get::<i64, _>("trigger_once").unwrap_or(1) != 0,
        cooldown_seconds: row.try_get::<i64, _>("cooldown_seconds").unwrap_or(300),
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        triggered_at: row
            .try_get::<Option<String>, _>("triggered_at")
            .ok()
            .flatten(),
        last_value: row.try_get::<Option<f64>, _>("last_value").ok().flatten(),
        last_trigger_value: row
            .try_get::<Option<f64>, _>("last_trigger_value")
            .ok()
            .flatten(),
        last_trigger_ts: row.try_get::<i64, _>("last_trigger_ts").unwrap_or(0),
    })
}

fn bool_i64(flag: bool) -> i64 {
    if flag {
        1
    } else {
        0
    }
}
