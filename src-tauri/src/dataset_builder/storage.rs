use serde_json::{json, Value};
use sqlx::{sqlite::SqliteRow, QueryBuilder, Row, Sqlite, SqlitePool};

use super::features::FeatureRow;
use super::round6;

const DATASET_SPLIT_INSERT_CHUNK: usize = 500;

struct EncodedSplitRow<'a> {
    row: &'a FeatureRow,
    features_json: String,
    label_json: String,
}

/// 将一个 split 写入 research_dataset_splits 表。
pub(crate) async fn write_split(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    dataset_id: &str,
    split: &str,
    rows: &[&FeatureRow],
    start_index: i64,
    now: f64,
) -> Result<(), String> {
    for (chunk_index, chunk) in rows.chunks(DATASET_SPLIT_INSERT_CHUNK).enumerate() {
        let chunk_start = chunk_index * DATASET_SPLIT_INSERT_CHUNK;
        let encoded_rows = chunk
            .iter()
            .map(|row| {
                let row = *row;
                Ok(EncodedSplitRow {
                    row,
                    features_json: feature_json(row)?,
                    label_json: label_json(row)?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let mut query = QueryBuilder::<Sqlite>::new(
            "INSERT INTO research_dataset_splits (dataset_id, split, row_index, inst_id, ts, features_json, label_json, created_at) ",
        );
        query.push_values(
            encoded_rows.iter().enumerate(),
            |mut row_builder, (offset, encoded)| {
                row_builder
                    .push_bind(dataset_id)
                    .push_bind(split)
                    .push_bind(start_index + (chunk_start + offset) as i64)
                    .push_bind(&encoded.row.inst_id)
                    .push_bind(encoded.row.ts)
                    .push_bind(&encoded.features_json)
                    .push_bind(&encoded.label_json)
                    .push_bind(now);
            },
        );
        query
            .build()
            .execute(&mut **tx)
            .await
            .map_err(|e| format!("写入数据集行失败: {e}"))?;
    }
    Ok(())
}

/// 写入/更新 manifest 到 research_dataset_manifests。
pub(crate) async fn write_manifest(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    dataset_id: &str,
    summary: &Value,
    now: f64,
) -> Result<(), String> {
    let manifest_json =
        serde_json::to_string(summary).map_err(|e| format!("序列化 manifest 失败: {e}"))?;
    sqlx::query(
        "INSERT OR REPLACE INTO research_dataset_manifests (dataset_id, status, manifest_json, created_at, updated_at) VALUES (?, 'ready', ?, ?, ?)",
    )
    .bind(dataset_id)
    .bind(&manifest_json)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|e| format!("写入 manifest 失败: {e}"))?;
    Ok(())
}

fn feature_json(row: &FeatureRow) -> Result<String, String> {
    let features = json!({
        "open": round6_finite(row.open, "open")?,
        "high": round6_finite(row.high, "high")?,
        "low": round6_finite(row.low, "low")?,
        "close": round6_finite(row.close, "close")?,
        "volume": round6_finite(row.volume, "volume")?,
        "mid_price": round6_finite(row.mid_price, "mid_price")?,
        "ret_1": round6_finite(row.ret_1, "ret_1")?,
        "ret_5": round6_finite(row.ret_5, "ret_5")?,
        "ret_20": round6_finite(row.ret_20, "ret_20")?,
        "ma5_dev": round6_finite(row.ma5_dev, "ma5_dev")?,
        "ma20_dev": round6_finite(row.ma20_dev, "ma20_dev")?,
        "vol_5": round6_finite(row.vol_5, "vol_5")?,
        "vol_20": round6_finite(row.vol_20, "vol_20")?,
        "vol_ratio": round6_finite(row.vol_ratio, "vol_ratio")?,
        "high_low_range": round6_finite(row.high_low_range, "high_low_range")?,
    });
    serde_json::to_string(&features).map_err(|e| format!("序列化 features_json 失败: {e}"))
}

fn label_json(row: &FeatureRow) -> Result<String, String> {
    let labels = json!({
        "label_1m": round6_optional(row.label_1m, "label_1m")?,
        "label_5m": round6_optional(row.label_5m, "label_5m")?,
        "label_15m": round6_optional(row.label_15m, "label_15m")?,
        "label_direction": row.label_direction,
    });
    serde_json::to_string(&labels).map_err(|e| format!("序列化 label_json 失败: {e}"))
}

fn round6_finite(value: f64, field: &str) -> Result<f64, String> {
    if !value.is_finite() {
        return Err(format!("数据集字段 {field} 不是有限数字"));
    }
    Ok(round6(value))
}

fn round6_optional(value: Option<f64>, field: &str) -> Result<Option<f64>, String> {
    value.map(|number| round6_finite(number, field)).transpose()
}

/// 查询数据集的 split 数据。
pub async fn get_dataset_splits(
    db: &SqlitePool,
    dataset_id: &str,
    split: Option<&str>,
    limit: i64,
) -> Result<Vec<Value>, String> {
    let rows = if let Some(s) = split {
        sqlx::query(
            "SELECT * FROM research_dataset_splits WHERE dataset_id = ? AND split = ? ORDER BY row_index ASC LIMIT ?",
        )
        .bind(dataset_id)
        .bind(s)
        .bind(limit.clamp(1, 5000))
        .fetch_all(db)
        .await
    } else {
        sqlx::query(
            "SELECT * FROM research_dataset_splits WHERE dataset_id = ? ORDER BY split, row_index ASC LIMIT ?",
        )
        .bind(dataset_id)
        .bind(limit.clamp(1, 5000))
        .fetch_all(db)
        .await
    }
    .map_err(|e| format!("查询数据集 splits 失败: {e}"))?;

    let result = rows
        .iter()
        .map(dataset_split_row_to_json)
        .collect::<Result<Vec<_>, String>>()?;

    Ok(result)
}

fn dataset_split_row_to_json(row: &SqliteRow) -> Result<Value, String> {
    let features_json: String = row
        .try_get("features_json")
        .map_err(|e| format!("读取 dataset split features_json 失败: {e}"))?;
    let label_json: String = row
        .try_get("label_json")
        .map_err(|e| format!("读取 dataset split label_json 失败: {e}"))?;
    let features: Value = serde_json::from_str(&features_json)
        .map_err(|e| format!("解析 dataset split features_json 失败: {e}"))?;
    let label: Value = serde_json::from_str(&label_json)
        .map_err(|e| format!("解析 dataset split label_json 失败: {e}"))?;

    Ok(json!({
        "id": row.try_get::<i64, _>("id").map_err(|e| format!("读取 dataset split id 失败: {e}"))?,
        "dataset_id": row.try_get::<String, _>("dataset_id").map_err(|e| format!("读取 dataset split dataset_id 失败: {e}"))?,
        "split": row.try_get::<String, _>("split").map_err(|e| format!("读取 dataset split split 失败: {e}"))?,
        "row_index": row.try_get::<i64, _>("row_index").map_err(|e| format!("读取 dataset split row_index 失败: {e}"))?,
        "inst_id": row.try_get::<String, _>("inst_id").map_err(|e| format!("读取 dataset split inst_id 失败: {e}"))?,
        "ts": row.try_get::<i64, _>("ts").map_err(|e| format!("读取 dataset split ts 失败: {e}"))?,
        "features": features,
        "label": label,
    }))
}

#[cfg(test)]
mod tests {
    use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};

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
              features_json TEXT NOT NULL DEFAULT '{}',
              label_json TEXT NOT NULL DEFAULT '{}',
              created_at REAL NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create research_dataset_splits");
        sqlx::query(
            "CREATE INDEX idx_research_dataset_splits_dataset_split_row ON research_dataset_splits(dataset_id, split, row_index)",
        )
        .execute(&pool)
        .await
        .expect("create dataset split index");
        sqlx::query(
            r#"
            CREATE TABLE research_dataset_manifests (
              dataset_id TEXT PRIMARY KEY,
              status TEXT NOT NULL DEFAULT 'created',
              manifest_json TEXT NOT NULL DEFAULT '{}',
              created_at REAL NOT NULL,
              updated_at REAL NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create research_dataset_manifests");
        pool
    }

    fn sample_row(index: i64) -> FeatureRow {
        let base = index as f64;
        FeatureRow {
            inst_id: "BTC-USDT-SWAP".to_string(),
            ts: 1_700_000_000 + index,
            open: 100.0 + base,
            high: 101.0 + base,
            low: 99.0 + base,
            close: 100.5 + base,
            volume: 10.0 + base,
            mid_price: 100.0 + base,
            ret_1: 0.001,
            ret_5: 0.002,
            ret_20: 0.003,
            ma5_dev: 0.004,
            ma20_dev: 0.005,
            vol_5: 0.006,
            vol_20: 0.007,
            vol_ratio: 1.2,
            high_low_range: 0.02,
            label_1m: Some(0.001),
            label_5m: Some(0.002),
            label_15m: Some(0.003),
            label_direction: Some(1),
        }
    }

    #[tokio::test]
    async fn transactional_split_write_preserves_rows_and_manifest() {
        let pool = test_pool().await;
        let rows = (0..3).map(sample_row).collect::<Vec<_>>();
        let refs = rows.iter().collect::<Vec<_>>();
        let summary = json!({
            "dataset_id": "dataset_a",
            "labeled_sample_count": refs.len()
        });

        let mut tx = pool.begin().await.expect("begin dataset tx");
        write_split(&mut tx, "dataset_a", "train", &refs, 10, 123.0)
            .await
            .expect("write split");
        write_manifest(&mut tx, "dataset_a", &summary, 123.0)
            .await
            .expect("write manifest");
        tx.commit().await.expect("commit dataset tx");

        let split_rows = get_dataset_splits(&pool, "dataset_a", Some("train"), 10)
            .await
            .expect("read split rows");
        assert_eq!(split_rows.len(), 3);
        assert_eq!(split_rows[0]["row_index"], 10);
        assert_eq!(split_rows[2]["row_index"], 12);
        assert_eq!(split_rows[0]["features"]["close"], 100.5);
        assert_eq!(split_rows[0]["label"]["label_direction"], 1);

        let manifest: String = sqlx::query(
            "SELECT manifest_json FROM research_dataset_manifests WHERE dataset_id = ?",
        )
        .bind("dataset_a")
        .fetch_one(&pool)
        .await
        .expect("manifest row")
        .try_get("manifest_json")
        .expect("manifest json");
        assert_eq!(
            serde_json::from_str::<Value>(&manifest).expect("manifest value")["dataset_id"],
            "dataset_a"
        );
    }

    #[tokio::test]
    async fn split_write_rejects_non_finite_feature_values() {
        let pool = test_pool().await;
        let mut row = sample_row(0);
        row.close = f64::NAN;
        let refs = vec![&row];

        let mut tx = pool.begin().await.expect("begin dataset tx");
        let error = write_split(&mut tx, "dataset_a", "train", &refs, 0, 123.0)
            .await
            .expect_err("non-finite feature should fail");

        assert!(error.contains("close"), "{error}");
    }

    #[tokio::test]
    async fn split_read_rejects_invalid_json_columns() {
        let pool = test_pool().await;
        sqlx::query(
            r#"
            INSERT INTO research_dataset_splits (
              dataset_id, split, row_index, inst_id, ts, features_json, label_json, created_at
            ) VALUES ('dataset_a', 'train', 0, 'BTC-USDT-SWAP', 1700000000, 'not-json', '{}', 123.0)
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert invalid split row");

        let error = get_dataset_splits(&pool, "dataset_a", Some("train"), 10)
            .await
            .expect_err("invalid split json should fail");

        assert!(error.contains("features_json"), "{error}");
    }
}
