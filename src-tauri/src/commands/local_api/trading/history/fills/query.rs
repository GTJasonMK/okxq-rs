use super::super::super::normalize::fill_row_to_json;
use super::super::super::*;

pub(crate) async fn local_fills(state: &AppState, req: &LocalApiRequest) -> AppResult<Value> {
    Ok(Value::Array(local_fill_rows(state, req).await?))
}

pub(in crate::commands::local_api::trading::history) async fn local_fill_rows(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Vec<Value>> {
    let mode = request_trading_mode(state, req).await?;
    let ccy = param_string(req, "ccy", "");
    let inst_id = param_string(req, "inst_id", "");
    let limit = param_i64(req, "limit", 100).clamp(1, 500);
    let mut sql = "SELECT * FROM local_fills WHERE mode = ?".to_string();
    if !ccy.is_empty() {
        sql.push_str(" AND ccy = ?");
    }
    if !inst_id.is_empty() {
        sql.push_str(" AND inst_id = ?");
    }
    sql.push_str(" ORDER BY ts DESC LIMIT ?");
    let mut query = sqlx::query(&sql).bind(mode);
    if !ccy.is_empty() {
        query = query.bind(ccy);
    }
    if !inst_id.is_empty() {
        query = query.bind(inst_id);
    }
    query = query.bind(limit);
    query
        .fetch_all(&state.db)
        .await?
        .into_iter()
        .map(fill_row_to_json)
        .collect::<AppResult<Vec<_>>>()
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use sqlx::Row;

    fn temp_db_path(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        std::env::temp_dir()
            .join(format!("okxq_{name}_{}_{}", std::process::id(), suffix))
            .join("market.db")
    }

    #[tokio::test]
    async fn local_fills_recent_queries_use_recent_indexes() {
        let db_path = temp_db_path("local_fills_recent_queries_use_recent_indexes");
        let pool = crate::storage::connect_and_migrate(&db_path)
            .await
            .expect("test database should migrate");

        let mode_plan = query_plan(
            &pool,
            "EXPLAIN QUERY PLAN SELECT * FROM local_fills WHERE mode = ? ORDER BY ts DESC LIMIT ?",
            &["simulated"],
        )
        .await;
        let mode_inst_plan = query_plan(
            &pool,
            "EXPLAIN QUERY PLAN SELECT * FROM local_fills WHERE mode = ? AND inst_id = ? ORDER BY ts DESC LIMIT ?",
            &["simulated", "SYM03-USDT-SWAP"],
        )
        .await;

        assert!(
            mode_plan.contains("idx_fills_mode_ts"),
            "mode query should use recent mode index, plan: {mode_plan}"
        );
        assert!(
            mode_inst_plan.contains("idx_fills_mode_inst_ts"),
            "mode+inst_id query should use recent scoped index, plan: {mode_inst_plan}"
        );
        assert!(
            !mode_plan.contains("USE TEMP B-TREE") && !mode_inst_plan.contains("USE TEMP B-TREE"),
            "recent queries should not sort through a temp b-tree, mode plan: {mode_plan}, mode+inst plan: {mode_inst_plan}"
        );

        pool.close().await;
        if let Some(parent) = db_path.parent() {
            std::fs::remove_dir_all(parent).ok();
        }
    }

    async fn query_plan(pool: &sqlx::SqlitePool, sql: &str, values: &[&str]) -> String {
        let mut query = sqlx::query(sql);
        for value in values {
            query = query.bind(*value);
        }
        query
            .bind(100_i64)
            .fetch_all(pool)
            .await
            .expect("explain local fills query")
            .into_iter()
            .map(|row| row.try_get::<String, _>("detail").expect("plan detail"))
            .collect::<Vec<_>>()
            .join(" | ")
    }
}
