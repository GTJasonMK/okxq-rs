// 市场相关性计算模块
// 基于多个币种的收盘价序列计算 Pearson 相关系数矩阵

use std::collections::BTreeMap;

use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::app_state::AppState;
use crate::error::AppResult;
use crate::market_candle_rows::load_latest_basic_candle_closes;

/// Pearson 相关系数
fn pearson_correlation(xs: &[f64], ys: &[f64]) -> f64 {
    let n = xs.len().min(ys.len());
    if n < 3 {
        return 0.0;
    }
    let mean_x = xs[..n].iter().sum::<f64>() / n as f64;
    let mean_y = ys[..n].iter().sum::<f64>() / n as f64;
    let mut cov = 0.0;
    let mut var_x = 0.0;
    let mut var_y = 0.0;
    for i in 0..n {
        let dx = xs[i] - mean_x;
        let dy = ys[i] - mean_y;
        cov += dx * dy;
        var_x += dx * dx;
        var_y += dy * dy;
    }
    let denom = (var_x * var_y).sqrt();
    if denom < 1e-12 {
        0.0
    } else {
        (cov / denom).clamp(-1.0, 1.0)
    }
}

async fn load_correlation_closes(
    db: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    limit: i64,
) -> AppResult<Vec<f64>> {
    load_latest_basic_candle_closes(db, inst_id, inst_type, timeframe, limit)
        .await
        .map_err(Into::into)
}

/// 计算一组币种之间的价格相关性矩阵
pub async fn compute_correlation_matrix(
    state: &AppState,
    inst_ids: &[String],
    inst_type: &str,
    timeframe: &str,
    lookback: i64,
) -> AppResult<Value> {
    let limit = lookback.clamp(20, 200);
    let mut price_series: BTreeMap<String, Vec<f64>> = BTreeMap::new();

    for inst_id in inst_ids {
        let closes =
            load_correlation_closes(&state.db, inst_id, inst_type, timeframe, limit).await?;

        if closes.len() >= 3 {
            price_series.insert(inst_id.clone(), closes);
        }
    }

    let symbols: Vec<&String> = price_series.keys().collect();
    let n = symbols.len();
    if n < 2 {
        return Ok(json!({
            "symbols": Vec::<String>::new(),
            "matrix": Vec::<Vec<f64>>::new(),
            "top_positive": Vec::<Value>::new(),
            "lowest": Vec::<Value>::new(),
            "data_points": 0
        }));
    }

    let mut matrix = vec![vec![1.0_f64; n]; n];
    let mut pairs = Vec::new();

    for i in 0..n {
        for j in (i + 1)..n {
            let si = symbols[i];
            let sj = symbols[j];
            let corr = pearson_correlation(&price_series[si], &price_series[sj]);
            matrix[i][j] = corr;
            matrix[j][i] = corr;
            pairs.push((si.clone(), sj.clone(), corr));
        }
    }

    pairs.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    let top_positive: Vec<Value> = pairs
        .iter()
        .filter(|(_, _, corr)| *corr > 0.0)
        .take(10)
        .map(|(a, b, corr)| json!({"a": a, "b": b, "correlation": round4(*corr)}))
        .collect();

    let lowest: Vec<Value> = pairs
        .iter()
        .take(10)
        .map(|(a, b, corr)| json!({"a": a, "b": b, "correlation": round4(*corr)}))
        .collect();

    Ok(json!({
        "symbols": symbols.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        "matrix": matrix,
        "top_positive": top_positive,
        "lowest": lowest,
        "data_points": limit
    }))
}

fn round4(value: f64) -> f64 {
    if value.is_finite() {
        (value * 10_000.0).round() / 10_000.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("create sqlite pool");
        sqlx::query(
            r#"
            CREATE TABLE candles (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              inst_id TEXT NOT NULL,
              inst_type TEXT NOT NULL DEFAULT 'SPOT',
              timeframe TEXT NOT NULL,
              timestamp INTEGER NOT NULL,
              open REAL NOT NULL,
              high REAL NOT NULL,
              low REAL NOT NULL,
              close REAL NOT NULL,
              volume REAL NOT NULL,
              volume_ccy REAL DEFAULT 0,
              created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
              UNIQUE(inst_id, inst_type, timeframe, timestamp)
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create candles table");
        pool
    }

    async fn insert_candle(pool: &SqlitePool, timestamp: i64, close_sql: &str) {
        let sql = format!(
            r#"
            INSERT INTO candles (
              inst_id, inst_type, timeframe, timestamp,
              open, high, low, close, volume, volume_ccy
            ) VALUES ('BTC-USDT-SWAP', 'SWAP', '1m', ?, 100, 101, 99, {close_sql}, 1, 1)
            "#
        );
        sqlx::query(&sql)
            .bind(timestamp)
            .execute(pool)
            .await
            .expect("insert candle");
    }

    #[tokio::test]
    async fn correlation_closes_apply_limit_after_filtering_invalid_market_rows() {
        let pool = test_pool().await;
        insert_candle(&pool, 60_000, "100").await;
        insert_candle(&pool, 120_000, "101").await;
        insert_candle(&pool, 180_000, "'bad-close'").await;

        let closes = load_correlation_closes(&pool, "BTC-USDT-SWAP", "SWAP", "1m", 2)
            .await
            .expect("load correlation closes");

        assert_eq!(closes, vec![100.0, 101.0]);
    }
}
