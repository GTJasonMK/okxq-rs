//! 趋势研究因子计算引擎 — 从 feature_bars_1s 秒级K线计算多类 alpha 因子，写入 factor_scores 表。
//!
//! 数据源：feature_bars_1s（秒级 OHLCV，由 tick_collector 写入），不足时回退到 candles。
//! 输出：factor_scores（PRIMARY KEY inst_id + factor_name，payload_json 存储最新因子值）。

use serde_json::Value;
use sqlx::SqlitePool;

mod bars;
mod metrics;
mod payload;
mod persistence;

use self::{
    bars::load_factor_bars, metrics::calculate_factor_values, payload::factor_payload,
    persistence::write_factor,
};

/// 计算所有因子并写入 factor_scores。
pub async fn compute_all_factors(
    db: &SqlitePool,
    inst_id: &str,
    inst_type: &str,
    timeframe: &str,
    bar_count: i64,
) -> Result<Vec<Value>, String> {
    let bars = load_factor_bars(db, inst_id, inst_type, timeframe, bar_count).await?;
    if bars.len() < 20 {
        return Err(format!(
            "数据不足：需要至少20根bar，当前仅{0}根。请先同步行情或启动秒级采集器。",
            bars.len()
        ));
    }

    let now = chrono::Utc::now().timestamp_millis() as f64 / 1000.0;
    let mut results = Vec::new();
    for (name, value) in calculate_factor_values(&bars) {
        let payload = factor_payload(inst_id, name, value, &bars);
        write_factor(db, inst_id, name, &payload, now).await?;
        results.push(payload);
    }

    Ok(results)
}
