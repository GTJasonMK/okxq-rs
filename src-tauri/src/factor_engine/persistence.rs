use serde_json::Value;
use sqlx::SqlitePool;

/// 写入单条因子到 factor_scores（INSERT OR REPLACE）。
pub(super) async fn write_factor(
    db: &SqlitePool,
    inst_id: &str,
    factor_name: &str,
    payload: &Value,
    now: f64,
) -> Result<(), String> {
    sqlx::query(
        "INSERT OR REPLACE INTO factor_scores (inst_id, factor_name, payload_json, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(inst_id)
    .bind(factor_name)
    .bind(serde_json::to_string(payload).unwrap_or_default())
    .bind(now)
    .execute(db)
    .await
    .map_err(|e| format!("写入因子 {factor_name} 失败: {e}"))?;
    Ok(())
}
