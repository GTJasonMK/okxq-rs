mod level_snapshots;
mod order_drafts;
mod patrol;
mod rows;
mod sessions;

pub(crate) use self::level_snapshots::{
    assistant_level_snapshot, assistant_level_snapshots, create_assistant_level_snapshot,
};
pub(crate) use self::order_drafts::{
    assistant_order_draft, assistant_order_drafts, confirm_assistant_order_draft,
    create_assistant_order_draft,
};
pub(crate) use self::patrol::{
    assistant_patrol_config, assistant_patrol_run, assistant_patrol_runs, assistant_patrol_status,
    assistant_run_patrol_now, assistant_update_patrol_config,
};
pub(super) use self::sessions::{
    append_assistant_message, assistant_session_detail_value, create_assistant_session_record,
    fetch_assistant_session,
};
pub(crate) use self::sessions::{
    assistant_agent_session_detail, assistant_agent_sessions, create_assistant_agent_session,
};

#[cfg(test)]
mod tests {
    use sqlx::Row;
    use std::path::{Path, PathBuf};

    use crate::storage;

    #[tokio::test]
    async fn assistant_recent_record_queries_use_recent_indexes() {
        let db_path = test_db_path("assistant-recent-record-indexes");
        let pool = storage::connect_and_migrate(&db_path).await.unwrap();

        let query_plans = [
            (
                "agent_sessions",
                "idx_assistant_sessions_kind_updated",
                query_plan_details(
                    &pool,
                    r#"
                    EXPLAIN QUERY PLAN
                    SELECT id FROM assistant_sessions
                    WHERE kind = 'agent'
                    ORDER BY updated_at DESC
                    LIMIT ?
                    "#,
                    &[("limit", "30")],
                )
                .await,
            ),
            (
                "session_messages",
                "idx_assistant_messages_session",
                query_plan_details(
                    &pool,
                    r#"
                    EXPLAIN QUERY PLAN
                    SELECT id FROM assistant_messages
                    WHERE session_id = ?
                    ORDER BY created_at ASC
                    "#,
                    &[("session_id", "session_a")],
                )
                .await,
            ),
            (
                "session_steps",
                "idx_assistant_steps_session",
                query_plan_details(
                    &pool,
                    r#"
                    EXPLAIN QUERY PLAN
                    SELECT id FROM assistant_steps
                    WHERE session_id = ?
                    ORDER BY created_at ASC
                    "#,
                    &[("session_id", "session_a")],
                )
                .await,
            ),
            (
                "drafts_global",
                "idx_assistant_order_drafts_recent",
                query_plan_details(
                    &pool,
                    r#"
                    EXPLAIN QUERY PLAN
                    SELECT id FROM assistant_order_drafts
                    ORDER BY created_at DESC
                    LIMIT ?
                    "#,
                    &[("limit", "200")],
                )
                .await,
            ),
            (
                "drafts_session",
                "idx_assistant_order_drafts_session_recent",
                query_plan_details(
                    &pool,
                    r#"
                    EXPLAIN QUERY PLAN
                    SELECT id FROM assistant_order_drafts
                    WHERE session_id = ?
                    ORDER BY created_at DESC
                    LIMIT 50
                    "#,
                    &[("session_id", "session_a")],
                )
                .await,
            ),
            (
                "levels_global",
                "idx_assistant_level_snapshots_recent",
                query_plan_details(
                    &pool,
                    r#"
                    EXPLAIN QUERY PLAN
                    SELECT id FROM assistant_level_snapshots
                    ORDER BY created_at DESC
                    LIMIT ?
                    "#,
                    &[("limit", "200")],
                )
                .await,
            ),
            (
                "levels_session",
                "idx_assistant_level_snapshots_session_recent",
                query_plan_details(
                    &pool,
                    r#"
                    EXPLAIN QUERY PLAN
                    SELECT id FROM assistant_level_snapshots
                    WHERE session_id = ?
                    ORDER BY created_at DESC
                    LIMIT 50
                    "#,
                    &[("session_id", "session_a")],
                )
                .await,
            ),
        ];

        for (name, index_name, plan) in &query_plans {
            assert!(
                plan.iter().any(|detail| detail.contains(index_name)),
                "{name} should use {index_name}, got {plan:?}"
            );
            assert!(
                plan.iter()
                    .all(|detail| !detail.contains("USE TEMP B-TREE")),
                "{name} should not sort with temp b-tree, got {plan:?}"
            );
        }

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
                "limit" => query.bind(value.parse::<i64>().unwrap()),
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
