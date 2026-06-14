use super::payload::{build_inventory_payload, InventoryBuildOptions};
use super::*;

pub(crate) async fn delete_inventory_symbol(
    state: &AppState,
    symbol: &str,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let normalized =
        normalize_symbol(symbol).ok_or_else(|| AppError::Validation("无效币种".to_string()))?;
    let watched = state
        .preferences
        .watched_symbols()
        .await?
        .into_iter()
        .any(|item| item.symbol == normalized);
    let remove_watch = param_bool(req, "remove_watch", false);
    if watched && !remove_watch {
        return Ok(json!({
            "code": 409,
            "message": "该币仍在关注列表中，请先删除关注，或传 remove_watch=true 一并移除",
            "data": null
        }));
    }
    mark_inventory_deletion_requested(&state.db, &normalized, "inventory_symbol_delete").await?;
    if watched && remove_watch {
        let _ = state.preferences.remove_watched_symbol(&normalized).await?;
    }
    let active_sync_jobs =
        cancel_related_sync_jobs(state, &normalized, "币种库存已删除，后台同步任务已取消").await?;
    let counts =
        delete_marked_symbol_related_data(&state.db, &normalized, "inventory_symbol_delete")
            .await?;
    Ok(code_ok(json!({
        "symbol": normalized,
        "removed_from_watch": watched && remove_watch,
        "deleted_counts": counts,
        "active_sync_jobs": active_sync_jobs
    })))
}

pub(crate) async fn delete_orphan_inventory(state: &AppState) -> AppResult<Value> {
    let inventory = build_inventory_payload(state, InventoryBuildOptions::default()).await?;
    let rows = inventory
        .get("rows")
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::Validation("库存 payload 缺少 rows 数组".to_string()))?;
    let mut deleted_symbols = Vec::new();
    let mut aggregate = BTreeMap::<String, i64>::new();
    let mut failed_symbols = Vec::new();

    for row in rows {
        if row.get("orphan").and_then(Value::as_bool) != Some(true) {
            continue;
        }
        let symbol = row
            .get("symbol")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::Validation("孤儿库存行缺少 symbol".to_string()))?;
        match delete_marked_symbol_related_data(&state.db, symbol, "orphan_inventory_cleanup").await
        {
            Ok(counts) => {
                deleted_symbols.push(Value::String(symbol.to_string()));
                let counts = counts.as_object().ok_or_else(|| {
                    AppError::Validation("库存删除结果必须是计数字段对象".to_string())
                })?;
                for (key, value) in counts {
                    let count = value.as_i64().ok_or_else(|| {
                        AppError::Validation(format!("库存删除结果 {key} 必须是整数"))
                    })?;
                    *aggregate.entry(key.clone()).or_insert(0) += count;
                }
            }
            Err(error) => {
                failed_symbols.push(json!({"symbol": symbol, "error": error.to_string()}))
            }
        }
    }
    let deleted_symbol_count = deleted_symbols.len();
    Ok(code_ok(json!({
        "deleted_symbols": deleted_symbols,
        "deleted_symbol_count": deleted_symbol_count,
        "deleted_counts": aggregate,
        "failed_symbols": failed_symbols
    })))
}

pub(in crate::commands::local_api::inventory) async fn deletion_marked_symbols(
    pool: &SqlitePool,
) -> AppResult<std::collections::BTreeSet<String>> {
    let rows = sqlx::query("SELECT symbol FROM inventory_deletion_marks")
        .fetch_all(pool)
        .await?;
    let mut symbols = std::collections::BTreeSet::new();
    for row in rows {
        let raw = row.try_get::<String, _>("symbol")?;
        let symbol = normalize_symbol(&raw)
            .ok_or_else(|| AppError::Validation(format!("库存删除标记包含无效 symbol: {raw}")))?;
        symbols.insert(symbol);
    }
    Ok(symbols)
}

pub(crate) async fn cancel_related_sync_jobs(
    state: &AppState,
    symbol: &str,
    reason: &str,
) -> AppResult<Vec<Value>> {
    let (_normalized, spot, swap, _base) =
        symbol_parts(symbol).ok_or_else(|| AppError::Validation("无效币种".to_string()))?;
    let inst_ids = vec![spot, swap];
    state
        .sync_jobs
        .cancel_jobs(Some(inst_ids.as_slice()), None, reason)
        .await?
        .into_iter()
        .map(|job| job.to_value().map_err(Into::into))
        .collect()
}

pub(crate) async fn delete_marked_symbol_related_data(
    pool: &SqlitePool,
    symbol: &str,
    reason: &str,
) -> AppResult<Value> {
    let normalized =
        normalize_symbol(symbol).ok_or_else(|| AppError::Validation("无效币种".to_string()))?;
    mark_inventory_deletion_requested(pool, &normalized, reason).await?;
    match delete_symbol_related_data(pool, &normalized).await {
        Ok(counts) => {
            let residual_count = symbol_related_data_count(pool, &normalized).await?;
            if residual_count == 0 {
                clear_inventory_deletion_mark(pool, &normalized).await?;
            }
            Ok(counts)
        }
        Err(error) => {
            record_inventory_deletion_error(pool, &normalized, &error.to_string()).await?;
            Err(error)
        }
    }
}

pub(crate) async fn delete_symbol_related_data(
    pool: &SqlitePool,
    symbol: &str,
) -> AppResult<Value> {
    let (normalized, spot, swap, base) =
        symbol_parts(symbol).ok_or_else(|| AppError::Validation("无效币种".to_string()))?;
    let inst_ids = [spot.as_str(), swap.as_str()];
    let mut counts = BTreeMap::<String, i64>::new();

    counts.insert(
        "candles".to_string(),
        delete_by_inst_ids(pool, "candles", "inst_id", &inst_ids).await?,
    );
    counts.insert(
        "feature_bars_1s".to_string(),
        delete_by_inst_ids(pool, "feature_bars_1s", "inst_id", &inst_ids).await?,
    );
    counts.insert(
        "sync_records".to_string(),
        delete_by_inst_ids(pool, "sync_records", "inst_id", &inst_ids).await?,
    );
    counts.insert(
        "market_ticker_snapshots".to_string(),
        delete_by_inst_ids(pool, "market_ticker_snapshots", "inst_id", &inst_ids).await?,
    );
    counts.insert(
        "market_recent_trades".to_string(),
        delete_by_inst_ids(pool, "market_recent_trades", "inst_id", &inst_ids).await?,
    );
    counts.insert(
        "live_order_records".to_string(),
        delete_by_inst_ids(pool, "live_order_records", "symbol", &inst_ids).await?,
    );
    counts.insert(
        "backtest_results".to_string(),
        sqlx::query("DELETE FROM backtest_results WHERE symbol IN (?, ?) OR symbol = ?")
            .bind(&spot)
            .bind(&swap)
            .bind(&normalized)
            .execute(pool)
            .await?
            .rows_affected() as i64,
    );
    counts.insert(
        "local_fills".to_string(),
        sqlx::query(
            "DELETE FROM local_fills WHERE inst_id IN (?, ?) OR (ccy = ? AND (inst_id IS NULL OR inst_id = ''))",
        )
        .bind(&spot)
        .bind(&swap)
        .bind(&base)
        .execute(pool)
        .await?
        .rows_affected() as i64,
    );
    counts.insert(
        "cost_basis".to_string(),
        sqlx::query("DELETE FROM cost_basis WHERE ccy = ?")
            .bind(&base)
            .execute(pool)
            .await?
            .rows_affected() as i64,
    );
    let total = counts.values().sum::<i64>();
    counts.insert("total".to_string(), total);
    Ok(json!(counts))
}

pub(crate) async fn mark_inventory_deletion_requested(
    pool: &SqlitePool,
    symbol: &str,
    reason: &str,
) -> AppResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO inventory_deletion_marks (symbol, requested_at, reason, last_error)
        VALUES (?, ?, ?, '')
        ON CONFLICT(symbol) DO UPDATE SET
          requested_at = excluded.requested_at,
          reason = excluded.reason,
          last_error = ''
        "#,
    )
    .bind(symbol)
    .bind(now)
    .bind(reason)
    .execute(pool)
    .await?;
    Ok(())
}

async fn record_inventory_deletion_error(
    pool: &SqlitePool,
    symbol: &str,
    error: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE inventory_deletion_marks SET last_error = ? WHERE symbol = ?")
        .bind(error)
        .bind(symbol)
        .execute(pool)
        .await?;
    Ok(())
}

async fn clear_inventory_deletion_mark(pool: &SqlitePool, symbol: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM inventory_deletion_marks WHERE symbol = ?")
        .bind(symbol)
        .execute(pool)
        .await?;
    Ok(())
}

async fn symbol_related_data_count(pool: &SqlitePool, symbol: &str) -> AppResult<i64> {
    Ok(symbol_related_storage_counts(pool, symbol)
        .await?
        .values()
        .sum::<i64>())
}

pub(in crate::commands::local_api::inventory) async fn symbol_related_storage_counts(
    pool: &SqlitePool,
    symbol: &str,
) -> AppResult<BTreeMap<String, i64>> {
    let (normalized, spot, swap, base) =
        symbol_parts(symbol).ok_or_else(|| AppError::Validation("无效币种".to_string()))?;
    let inst_ids = [spot.as_str(), swap.as_str()];
    let mut counts = BTreeMap::<String, i64>::new();

    counts.insert(
        "candles".to_string(),
        count_by_inst_ids(pool, "candles", "inst_id", &inst_ids).await?,
    );
    counts.insert(
        "feature_bars_1s".to_string(),
        count_by_inst_ids(pool, "feature_bars_1s", "inst_id", &inst_ids).await?,
    );
    counts.insert(
        "sync_records".to_string(),
        count_by_inst_ids(pool, "sync_records", "inst_id", &inst_ids).await?,
    );
    counts.insert(
        "market_ticker_snapshots".to_string(),
        count_by_inst_ids(pool, "market_ticker_snapshots", "inst_id", &inst_ids).await?,
    );
    counts.insert(
        "market_recent_trades".to_string(),
        count_by_inst_ids(pool, "market_recent_trades", "inst_id", &inst_ids).await?,
    );
    counts.insert(
        "live_order_records".to_string(),
        count_by_inst_ids(pool, "live_order_records", "symbol", &inst_ids).await?,
    );
    counts.insert(
        "backtest_results".to_string(),
        sqlx::query(
            "SELECT COUNT(*) AS count FROM backtest_results WHERE symbol IN (?, ?) OR symbol = ?",
        )
        .bind(&spot)
        .bind(&swap)
        .bind(&normalized)
        .fetch_one(pool)
        .await?
        .try_get::<i64, _>("count")?,
    );
    counts.insert(
        "local_fills".to_string(),
        sqlx::query(
            "SELECT COUNT(*) AS count FROM local_fills WHERE inst_id IN (?, ?) OR (ccy = ? AND (inst_id IS NULL OR inst_id = ''))",
        )
        .bind(&spot)
        .bind(&swap)
        .bind(&base)
        .fetch_one(pool)
        .await?
        .try_get::<i64, _>("count")?,
    );
    counts.insert(
        "cost_basis".to_string(),
        sqlx::query("SELECT COUNT(*) AS count FROM cost_basis WHERE ccy = ?")
            .bind(&base)
            .fetch_one(pool)
            .await?
            .try_get::<i64, _>("count")?,
    );
    Ok(counts)
}
async fn delete_by_inst_ids(
    pool: &SqlitePool,
    table: &str,
    column: &str,
    inst_ids: &[&str; 2],
) -> AppResult<i64> {
    let sql = format!("DELETE FROM {table} WHERE {column} IN (?, ?)");
    Ok(sqlx::query(&sql)
        .bind(inst_ids[0])
        .bind(inst_ids[1])
        .execute(pool)
        .await?
        .rows_affected() as i64)
}

async fn count_by_inst_ids(
    pool: &SqlitePool,
    table: &str,
    column: &str,
    inst_ids: &[&str; 2],
) -> AppResult<i64> {
    let sql = format!("SELECT COUNT(*) AS count FROM {table} WHERE {column} IN (?, ?)");
    Ok(sqlx::query(&sql)
        .bind(inst_ids[0])
        .bind(inst_ids[1])
        .fetch_one(pool)
        .await?
        .try_get::<i64, _>("count")?)
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;

    async fn deletion_marks_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("create sqlite pool");
        sqlx::query(
            r#"
            CREATE TABLE inventory_deletion_marks (
              symbol TEXT PRIMARY KEY,
              requested_at TEXT NOT NULL,
              reason TEXT NOT NULL DEFAULT '',
              last_error TEXT NOT NULL DEFAULT ''
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create deletion marks table");
        pool
    }

    #[tokio::test]
    async fn deletion_marked_symbols_normalizes_internal_marks() {
        let pool = deletion_marks_pool().await;
        sqlx::query(
            "INSERT INTO inventory_deletion_marks (symbol, requested_at) VALUES ('btc-usdt-swap', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("insert mark");

        let symbols = deletion_marked_symbols(&pool)
            .await
            .expect("deletion marks");

        assert_eq!(
            symbols,
            std::collections::BTreeSet::from(["BTC-USDT".to_string()])
        );
    }

    #[tokio::test]
    async fn deletion_marked_symbols_rejects_invalid_internal_mark() {
        let pool = deletion_marks_pool().await;
        sqlx::query(
            "INSERT INTO inventory_deletion_marks (symbol, requested_at) VALUES ('', '2026-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("insert dirty mark");

        let result = deletion_marked_symbols(&pool).await;

        assert!(result.is_err());
    }
}
