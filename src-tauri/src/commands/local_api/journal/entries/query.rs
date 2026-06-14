use serde_json::{json, Value};

use crate::{
    app_state::AppState,
    commands::local_api::{
        code_ok, param_i64, param_string, request_string_array, LocalApiRequest,
    },
    error::{AppError, AppResult},
};

use super::super::rows::{fetch_journal_entry, journal_row_to_json};

pub(crate) async fn journal_entries(state: &AppState, req: &LocalApiRequest) -> AppResult<Value> {
    let mode = param_string(req, "mode", "");
    let inst_id = param_string(req, "inst_id", "");
    let tags = request_string_array(req, "tags");
    let strategy_id = param_string(req, "strategy_id", "");
    let date_from = param_string(req, "date_from", "");
    let date_to = param_string(req, "date_to", "");
    let limit = param_i64(req, "limit", 50).clamp(1, 200);
    let offset = param_i64(req, "offset", 0).max(0);

    let mut conditions = Vec::<String>::new();
    let mut values = Vec::<String>::new();
    if !mode.is_empty() {
        conditions.push("mode = ?".to_string());
        values.push(mode);
    }
    if !inst_id.is_empty() {
        conditions.push("inst_id = ?".to_string());
        values.push(inst_id);
    }
    if !strategy_id.is_empty() {
        conditions.push("strategy_id = ?".to_string());
        values.push(strategy_id);
    }
    if !date_from.is_empty() {
        conditions.push("created_at >= ?".to_string());
        values.push(date_from);
    }
    if !date_to.is_empty() {
        conditions.push("created_at <= ?".to_string());
        values.push(date_to);
    }
    for tag in tags {
        conditions.push("tags_json LIKE ?".to_string());
        values.push(format!("%\"{}\"%", tag.trim()));
    }
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };
    let sql = format!(
        "SELECT * FROM journal_entries {where_clause} ORDER BY created_at DESC LIMIT ? OFFSET ?"
    );
    let mut query = sqlx::query(&sql);
    for value in &values {
        query = query.bind(value);
    }
    query = query.bind(limit).bind(offset);
    let entries = query
        .fetch_all(&state.db)
        .await?
        .into_iter()
        .map(journal_row_to_json)
        .collect::<AppResult<Vec<_>>>()?;

    Ok(code_ok(Value::Array(entries)))
}

pub(crate) async fn journal_entry(state: &AppState, entry_id: &str) -> AppResult<Value> {
    match fetch_journal_entry(&state.db, entry_id).await? {
        Some(entry) => Ok(code_ok(entry)),
        None => Err(AppError::Validation("日志条目不存在".to_string())),
    }
}

pub(crate) async fn deleted_entry_payload(deleted: u64) -> AppResult<Value> {
    if deleted == 0 {
        return Err(AppError::Validation("日志条目不存在".to_string()));
    }
    Ok(code_ok(json!({"deleted": true})))
}

#[cfg(test)]
mod tests {
    use sqlx::Row;
    use std::path::{Path, PathBuf};

    use crate::storage;

    #[tokio::test]
    async fn journal_recent_list_queries_use_recent_indexes() {
        let db_path = test_db_path("journal-recent-indexes");
        let pool = storage::connect_and_migrate(&db_path).await.unwrap();

        let recent_plan = query_plan_details(
            &pool,
            r#"
            EXPLAIN QUERY PLAN
            SELECT entry_id FROM journal_entries
            ORDER BY created_at DESC
            LIMIT ? OFFSET ?
            "#,
            &[("limit", "200"), ("offset", "0")],
        )
        .await;
        let strategy_plan = query_plan_details(
            &pool,
            r#"
            EXPLAIN QUERY PLAN
            SELECT entry_id FROM journal_entries
            WHERE strategy_id = ?
            ORDER BY created_at DESC
            LIMIT ? OFFSET ?
            "#,
            &[
                ("strategy_id", "strategy_a"),
                ("limit", "200"),
                ("offset", "0"),
            ],
        )
        .await;

        assert!(
            recent_plan
                .iter()
                .any(|detail| detail.contains("idx_journal_recent")),
            "global journal list should use recent index, got {recent_plan:?}"
        );
        assert!(
            strategy_plan
                .iter()
                .any(|detail| detail.contains("idx_journal_strategy_time")),
            "strategy journal list should use strategy recent index, got {strategy_plan:?}"
        );
        assert!(
            recent_plan
                .iter()
                .chain(strategy_plan.iter())
                .all(|detail| !detail.contains("USE TEMP B-TREE")),
            "journal recent list queries should not sort with temp b-tree: {recent_plan:?} {strategy_plan:?}"
        );

        cleanup_db(pool, &db_path).await;
    }

    async fn query_plan_details(
        pool: &sqlx::SqlitePool,
        sql: &str,
        bindings: &[(&str, &str)],
    ) -> Vec<String> {
        let mut query = sqlx::query(sql);
        for (name, value) in bindings {
            query = match *name {
                "limit" | "offset" => query.bind(value.parse::<i64>().unwrap()),
                _ => query.bind(*value),
            };
        }
        query
            .fetch_all(pool)
            .await
            .unwrap()
            .into_iter()
            .map(|row| row.try_get::<String, _>("detail").unwrap())
            .collect()
    }

    fn test_db_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "okxq-rs-{label}-{}-{}.db",
            std::process::id(),
            uuid::Uuid::new_v4()
        ))
    }

    async fn cleanup_db(pool: sqlx::SqlitePool, path: &Path) {
        pool.close().await;
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
    }
}
