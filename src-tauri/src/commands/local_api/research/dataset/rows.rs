use sqlx::{sqlite::SqliteRow, Row};

use super::super::*;

pub(crate) fn dataset_row_to_json(row: SqliteRow) -> AppResult<Value> {
    let manifest_text = row.try_get::<String, _>("manifest_json")?;
    let mut manifest = json_object_from_text(&manifest_text, "dataset manifest_json")?;
    let dataset_id = row.try_get::<String, _>("dataset_id")?;
    let status = row.try_get::<String, _>("status")?;
    manifest.insert("dataset_id".to_string(), Value::String(dataset_id));
    manifest.insert("status".to_string(), Value::String(status.clone()));
    manifest.insert("dataset_status".to_string(), Value::String(status));
    manifest.insert(
        "created_at".to_string(),
        Value::from(row.try_get::<f64, _>("created_at")?),
    );
    manifest.insert(
        "updated_at".to_string(),
        Value::from(row.try_get::<f64, _>("updated_at")?),
    );
    Ok(Value::Object(manifest))
}

fn json_object_from_text(text: &str, context: &str) -> AppResult<Map<String, Value>> {
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
    async fn dataset_row_rejects_invalid_manifest_json() {
        let pool = test_pool().await;
        let row = sqlx::query(
            r#"
            SELECT
              'dataset_a' AS dataset_id,
              'ready' AS status,
              'not-json' AS manifest_json,
              1.0 AS created_at,
              1.0 AS updated_at
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("dataset row");

        assert!(dataset_row_to_json(row).is_err());
    }
}
