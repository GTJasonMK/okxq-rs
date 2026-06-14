use serde_json::Value;
use sqlx::{sqlite::SqliteRow, Row};

use crate::error::{AppError, AppResult};

pub(super) fn inference_row_to_json(row: SqliteRow) -> AppResult<Value> {
    let payload_text = row.try_get::<String, _>("payload_json")?;
    let mut payload = json_object_from_text(&payload_text, "inference payload_json")?;
    payload.insert(
        "inst_id".to_string(),
        Value::String(row.try_get::<String, _>("inst_id")?),
    );
    payload.insert(
        "created_at".to_string(),
        Value::from(row.try_get::<f64, _>("created_at")?),
    );
    Ok(Value::Object(payload))
}

pub(super) fn feature_bar_row_to_json(row: SqliteRow) -> AppResult<Value> {
    let payload_text = row.try_get::<String, _>("payload_json")?;
    let mut payload = json_object_from_text(&payload_text, "feature bar payload_json")?;
    let ts = row.try_get::<i64, _>("ts")?;
    payload.insert(
        "inst_id".to_string(),
        Value::String(row.try_get::<String, _>("inst_id")?),
    );
    payload.insert("ts".to_string(), Value::from(ts));
    payload.insert("second_bucket".to_string(), Value::from(ts));
    payload.insert(
        "created_at".to_string(),
        Value::from(row.try_get::<f64, _>("created_at")?),
    );
    Ok(Value::Object(payload))
}

pub(super) fn factor_score_row_to_json(row: SqliteRow) -> AppResult<Value> {
    let payload_text = row.try_get::<String, _>("payload_json")?;
    let mut payload = json_object_from_text(&payload_text, "factor score payload_json")?;
    payload.insert(
        "inst_id".to_string(),
        Value::String(row.try_get::<String, _>("inst_id")?),
    );
    payload.insert(
        "factor_name".to_string(),
        Value::String(row.try_get::<String, _>("factor_name")?),
    );
    payload.insert(
        "created_at".to_string(),
        Value::from(row.try_get::<f64, _>("created_at")?),
    );
    Ok(Value::Object(payload))
}

fn json_object_from_text(text: &str, context: &str) -> AppResult<serde_json::Map<String, Value>> {
    let value = serde_json::from_str::<Value>(text)?;
    match value {
        Value::Object(object) => Ok(object),
        _ => Err(AppError::Runtime(format!("{context} 不是 JSON 对象"))),
    }
}

#[cfg(test)]
mod tests {
    use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

    use super::*;

    async fn test_pool() -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory sqlite")
    }

    #[tokio::test]
    async fn feature_bar_row_rejects_invalid_payload_json() {
        let pool = test_pool().await;
        let row = sqlx::query(
            r#"
            SELECT
              'BTC-USDT-SWAP' AS inst_id,
              1700000000 AS ts,
              'not-json' AS payload_json,
              1.0 AS created_at
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("feature row");

        assert!(feature_bar_row_to_json(row).is_err());
    }

    #[tokio::test]
    async fn inference_row_rejects_non_object_payload_json() {
        let pool = test_pool().await;
        let row = sqlx::query(
            r#"
            SELECT
              'BTC-USDT-SWAP' AS inst_id,
              '[]' AS payload_json,
              1.0 AS created_at
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("inference row");

        let error = inference_row_to_json(row).expect_err("non-object payload should fail");

        assert!(error.to_string().contains("JSON 对象"), "{error}");
    }
}
