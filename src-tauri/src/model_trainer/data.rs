use serde_json::Value;
use sqlx::{Row, SqlitePool};

use super::FEATURE_NAMES;

/// 加载数据集的某个 split，返回 (特征矩阵 N×M, 标签向量 N, ts列表)。
pub(super) async fn load_split_data(
    db: &SqlitePool,
    dataset_id: &str,
    split: &str,
) -> Result<(Vec<Vec<f64>>, Vec<f64>, Vec<i64>), String> {
    let rows = sqlx::query(
        "SELECT features_json, label_json, ts FROM research_dataset_splits WHERE dataset_id = ? AND split = ? ORDER BY row_index ASC",
    )
    .bind(dataset_id)
    .bind(split)
    .fetch_all(db)
    .await
    .map_err(|e| format!("加载 {split} 数据失败: {e}"))?;

    let mut features = Vec::with_capacity(rows.len());
    let mut labels = Vec::with_capacity(rows.len());
    let mut timestamps = Vec::with_capacity(rows.len());

    for row in &rows {
        let f_text: String = row
            .try_get("features_json")
            .map_err(|e| format!("读取 {split} features_json 失败: {e}"))?;
        let l_text: String = row
            .try_get("label_json")
            .map_err(|e| format!("读取 {split} label_json 失败: {e}"))?;
        let ts: i64 = row
            .try_get("ts")
            .map_err(|e| format!("读取 {split} ts 失败: {e}"))?;

        let f = serde_json::from_str::<Value>(&f_text)
            .map_err(|e| format!("解析 {split} features_json 失败: {e}"))?;
        let l = serde_json::from_str::<Value>(&l_text)
            .map_err(|e| format!("解析 {split} label_json 失败: {e}"))?;
        let feat_vec = FEATURE_NAMES
            .iter()
            .map(|field| required_finite_f64(&f, field, split))
            .collect::<Result<Vec<_>, String>>()?;
        let label = required_finite_f64(&l, "label_1m", split)?;

        labels.push(label);
        features.push(feat_vec);
        timestamps.push(ts);
    }

    if features.is_empty() {
        return Err(format!("{split} 切分无有效数据"));
    }
    Ok((features, labels, timestamps))
}

fn required_finite_f64(value: &Value, field: &str, split: &str) -> Result<f64, String> {
    let number = value
        .get(field)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("{split} 数据字段 {field} 缺失或不是数字"))?;
    if !number.is_finite() {
        return Err(format!("{split} 数据字段 {field} 不是有限数字"));
    }
    Ok(number)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

    use super::*;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory sqlite");
        sqlx::query(
            r#"
            CREATE TABLE research_dataset_splits (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              dataset_id TEXT NOT NULL,
              split TEXT NOT NULL,
              row_index INTEGER NOT NULL,
              inst_id TEXT NOT NULL,
              ts INTEGER NOT NULL,
              features_json TEXT NOT NULL,
              label_json TEXT NOT NULL,
              created_at REAL NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create research_dataset_splits");
        pool
    }

    fn valid_features_json() -> String {
        let mut features = serde_json::Map::new();
        for field in FEATURE_NAMES {
            features.insert((*field).to_string(), json!(1.0));
        }
        Value::Object(features).to_string()
    }

    async fn insert_split_row(pool: &SqlitePool, features_json: &str, label_json: &str) {
        sqlx::query(
            r#"
            INSERT INTO research_dataset_splits (
              dataset_id, split, row_index, inst_id, ts, features_json, label_json, created_at
            ) VALUES ('dataset_a', 'train', 0, 'BTC-USDT-SWAP', 1700000000, ?, ?, 123.0)
            "#,
        )
        .bind(features_json)
        .bind(label_json)
        .execute(pool)
        .await
        .expect("insert split row");
    }

    #[tokio::test]
    async fn load_split_data_rejects_invalid_feature_json() {
        let pool = test_pool().await;
        insert_split_row(&pool, "not-json", r#"{"label_1m":0.1}"#).await;

        let error = load_split_data(&pool, "dataset_a", "train")
            .await
            .expect_err("invalid feature json should fail");

        assert!(error.contains("features_json"), "{error}");
    }

    #[tokio::test]
    async fn load_split_data_rejects_missing_label() {
        let pool = test_pool().await;
        let features_json = valid_features_json();
        insert_split_row(&pool, &features_json, "{}").await;

        let error = load_split_data(&pool, "dataset_a", "train")
            .await
            .expect_err("missing label should fail");

        assert!(error.contains("label_1m"), "{error}");
    }
}
