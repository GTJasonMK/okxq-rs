use super::*;

mod collection;
mod dataset;
mod scope;
mod training;
pub(crate) mod trend;

pub(crate) use self::collection::*;
pub(crate) use self::dataset::*;
pub(crate) use self::scope::*;
pub(crate) use self::training::*;
pub(crate) use self::trend::*;

#[cfg(test)]
mod tests {
    use sqlx::Row;
    use std::path::{Path, PathBuf};

    use crate::storage;

    #[tokio::test]
    async fn research_recent_list_queries_use_recent_indexes() {
        let db_path = test_db_path("research-recent-indexes");
        let pool = storage::connect_and_migrate(&db_path).await.unwrap();

        let query_plans = [
            (
                "training_global",
                "idx_research_training_runs_recent",
                query_plan_details(
                    &pool,
                    r#"
                    EXPLAIN QUERY PLAN
                    SELECT run_id FROM research_training_runs
                    ORDER BY updated_at DESC
                    LIMIT ?
                    "#,
                    &[("limit", "500")],
                )
                .await,
            ),
            (
                "training_dataset",
                "idx_research_training_runs_dataset_recent",
                query_plan_details(
                    &pool,
                    r#"
                    EXPLAIN QUERY PLAN
                    SELECT run_id FROM research_training_runs
                    WHERE dataset_id = ?
                    ORDER BY updated_at DESC
                    LIMIT ?
                    "#,
                    &[("dataset_id", "dataset_a"), ("limit", "500")],
                )
                .await,
            ),
            (
                "collection",
                "idx_research_collection_sessions_recent",
                query_plan_details(
                    &pool,
                    r#"
                    EXPLAIN QUERY PLAN
                    SELECT session_id FROM research_collection_sessions
                    ORDER BY updated_at DESC
                    LIMIT ?
                    "#,
                    &[("limit", "500")],
                )
                .await,
            ),
            (
                "datasets",
                "idx_research_dataset_manifests_recent",
                query_plan_details(
                    &pool,
                    r#"
                    EXPLAIN QUERY PLAN
                    SELECT dataset_id FROM research_dataset_manifests
                    ORDER BY updated_at DESC
                    LIMIT ?
                    "#,
                    &[("limit", "500")],
                )
                .await,
            ),
            (
                "inference",
                "idx_inference_snapshots_recent",
                query_plan_details(
                    &pool,
                    r#"
                    EXPLAIN QUERY PLAN
                    SELECT id FROM inference_snapshots
                    ORDER BY created_at DESC
                    LIMIT ?
                    "#,
                    &[("limit", "500")],
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

    #[tokio::test]
    async fn research_dataset_split_queries_use_dataset_split_row_index() {
        let db_path = test_db_path("research-dataset-split-index");
        let pool = storage::connect_and_migrate(&db_path).await.unwrap();

        let split_plan = query_plan_details(
            &pool,
            r#"
            EXPLAIN QUERY PLAN
            SELECT id FROM research_dataset_splits
            WHERE dataset_id = ? AND split = ?
            ORDER BY row_index ASC
            LIMIT ?
            "#,
            &[
                ("dataset_id", "dataset_a"),
                ("split", "train"),
                ("limit", "5000"),
            ],
        )
        .await;
        let dataset_plan = query_plan_details(
            &pool,
            r#"
            EXPLAIN QUERY PLAN
            SELECT id FROM research_dataset_splits
            WHERE dataset_id = ?
            ORDER BY split, row_index ASC
            LIMIT ?
            "#,
            &[("dataset_id", "dataset_a"), ("limit", "5000")],
        )
        .await;

        for (name, plan) in [("split", split_plan), ("dataset", dataset_plan)] {
            assert!(
                plan.iter().any(|detail| {
                    detail.contains("idx_research_dataset_splits_dataset_split_row")
                }),
                "{name} dataset split query should use dataset/split/row index, got {plan:?}"
            );
            assert!(
                plan.iter()
                    .all(|detail| !detail.contains("USE TEMP B-TREE")),
                "{name} dataset split query should not sort with temp b-tree, got {plan:?}"
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
