use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};

use crate::{
    app_state::AppState,
    commands::local_api::{code_ok, param_string, round2, LocalApiRequest},
    error::AppResult,
};

pub(crate) async fn journal_stats(state: &AppState, req: &LocalApiRequest) -> AppResult<Value> {
    let mode = param_string(req, "mode", "");
    let group_by = param_string(req, "group_by", "tag");
    Ok(code_ok(
        journal_stats_from_pool(&state.db, &mode, &group_by).await?,
    ))
}

async fn journal_stats_from_pool(
    pool: &SqlitePool,
    mode: &str,
    group_by: &str,
) -> AppResult<Value> {
    if group_by == "strategy" {
        stats_by_strategy(pool, mode).await
    } else {
        stats_by_tags(pool, mode).await
    }
}

async fn stats_by_strategy(pool: &SqlitePool, mode: &str) -> AppResult<Value> {
    let sql = strategy_stats_sql(!mode.is_empty());
    let mut query = sqlx::query(&sql);
    if !mode.is_empty() {
        query = query.bind(mode);
    }
    let mut total_entries = 0_i64;
    let rows = query.fetch_all(pool).await?;
    let mut groups = Vec::with_capacity(rows.len());
    for row in rows {
        let group = stats_group_base(&row)?;
        total_entries += group.count;
        groups.push(strategy_group_json(group));
    }
    Ok(json!({"group_by": "strategy", "groups": groups, "total_entries": total_entries}))
}

async fn stats_by_tags(pool: &SqlitePool, mode: &str) -> AppResult<Value> {
    let total_entries = journal_stats_total_entries(pool, mode).await?;
    let sql = tag_stats_sql(!mode.is_empty());
    let mut query = sqlx::query(&sql);
    if !mode.is_empty() {
        query = query.bind(mode);
    }
    let rows = query.fetch_all(pool).await?;
    let mut groups = Vec::with_capacity(rows.len());
    for row in rows {
        let group = stats_group_base(&row)?;
        let ratings_sum = row.try_get::<i64, _>("ratings_sum")?;
        groups.push(tag_group_json(group, ratings_sum));
    }
    Ok(json!({"group_by": "tag", "groups": groups, "total_entries": total_entries}))
}

struct JournalStatsGroup {
    key: String,
    count: i64,
    total_pnl: f64,
    positive: i64,
}

fn stats_group_base(row: &sqlx::sqlite::SqliteRow) -> AppResult<JournalStatsGroup> {
    Ok(JournalStatsGroup {
        key: row.try_get::<String, _>("key")?,
        count: row.try_get::<i64, _>("count")?,
        total_pnl: row.try_get::<f64, _>("total_pnl")?,
        positive: row.try_get::<i64, _>("positive")?,
    })
}

fn strategy_group_json(group: JournalStatsGroup) -> Value {
    json!({
        "key": group.key,
        "count": group.count,
        "total_pnl": round2(group.total_pnl),
        "win_rate": win_rate(group.count, group.positive)
    })
}

fn tag_group_json(group: JournalStatsGroup, ratings_sum: i64) -> Value {
    json!({
        "key": group.key,
        "count": group.count,
        "total_pnl": round2(group.total_pnl),
        "win_rate": win_rate(group.count, group.positive),
        "avg_rating": avg_rating(group.count, ratings_sum)
    })
}

fn win_rate(count: i64, positive: i64) -> f64 {
    if count > 0 {
        round2(positive as f64 / count as f64 * 100.0)
    } else {
        0.0
    }
}

fn avg_rating(count: i64, ratings_sum: i64) -> f64 {
    if count > 0 {
        ((ratings_sum as f64 / count as f64) * 10.0).round() / 10.0
    } else {
        0.0
    }
}

async fn journal_stats_total_entries(pool: &SqlitePool, mode: &str) -> AppResult<i64> {
    let mut query = if mode.is_empty() {
        sqlx::query("SELECT COUNT(*) AS count FROM journal_entries")
    } else {
        sqlx::query("SELECT COUNT(*) AS count FROM journal_entries WHERE mode = ?")
    };
    if !mode.is_empty() {
        query = query.bind(mode);
    }
    let row = query.fetch_one(pool).await?;
    Ok(row.try_get::<i64, _>("count")?)
}

fn strategy_stats_sql(has_mode_filter: bool) -> String {
    let mode_filter = if has_mode_filter {
        "WHERE mode = ?"
    } else {
        ""
    };
    format!(
        r#"
        SELECT
          CASE WHEN strategy_id IS NULL OR strategy_id = '' THEN '未知' ELSE strategy_id END AS key,
          COUNT(*) AS count,
          COALESCE(SUM(pnl_snapshot), 0.0) AS total_pnl,
          COALESCE(SUM(CASE WHEN pnl_snapshot > 0 THEN 1 ELSE 0 END), 0) AS positive
        FROM journal_entries
        {mode_filter}
        GROUP BY key
        ORDER BY count DESC, key ASC
        "#
    )
}

fn tag_stats_sql(has_mode_filter: bool) -> String {
    let mode_filter = if has_mode_filter {
        "WHERE mode = ?"
    } else {
        ""
    };
    format!(
        r#"
WITH filtered AS (
  SELECT tags_json, pnl_snapshot, rating
  FROM journal_entries
  {mode_filter}
),
valid_tags AS (
  SELECT
    CASE
      WHEN json_valid(COALESCE(tags_json, '[]')) = 1 THEN COALESCE(tags_json, '[]')
      ELSE '[]'
    END AS raw_tags_json,
    pnl_snapshot,
    rating
  FROM filtered
),
array_tags AS (
  SELECT
    CASE WHEN json_type(raw_tags_json) = 'array' THEN raw_tags_json ELSE '[]' END AS tags_json,
    pnl_snapshot,
    rating
  FROM valid_tags
),
expanded AS (
  SELECT
    CAST(tag.value AS TEXT) AS key,
    pnl_snapshot,
    rating
  FROM array_tags
  JOIN json_each(
    CASE WHEN json_array_length(tags_json) > 0 THEN tags_json ELSE '["未标记"]' END
  ) AS tag
  WHERE tag.type = 'text'
)
SELECT
  key,
  COUNT(*) AS count,
  COALESCE(SUM(pnl_snapshot), 0.0) AS total_pnl,
  COALESCE(SUM(CASE WHEN pnl_snapshot > 0 THEN 1 ELSE 0 END), 0) AS positive,
  COALESCE(SUM(COALESCE(rating, 0)), 0) AS ratings_sum
FROM expanded
GROUP BY key
ORDER BY count DESC, key ASC
"#
    )
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use serde_json::json;

    use super::*;
    use crate::storage;

    #[tokio::test]
    async fn journal_stats_sql_aggregation_returns_expected_contract() {
        let db_path = test_db_path("journal-stats-contract");
        let pool = storage::connect_and_migrate(&db_path).await.unwrap();
        insert_contract_rows(&pool).await;

        for (mode, group_by, expected) in [
            (
                "",
                "tag",
                json!({
                    "group_by": "tag",
                    "groups": [
                        {"key": "trend", "count": 2, "total_pnl": 12.5, "win_rate": 50.0, "avg_rating": 3.0},
                        {"key": "未标记", "count": 2, "total_pnl": 3.0, "win_rate": 50.0, "avg_rating": 3.5},
                        {"key": "breakout", "count": 1, "total_pnl": 12.5, "win_rate": 100.0, "avg_rating": 5.0}
                    ],
                    "total_entries": 5
                }),
            ),
            (
                "simulated",
                "tag",
                json!({
                    "group_by": "tag",
                    "groups": [
                        {"key": "未标记", "count": 2, "total_pnl": 3.0, "win_rate": 50.0, "avg_rating": 3.5},
                        {"key": "breakout", "count": 1, "total_pnl": 12.5, "win_rate": 100.0, "avg_rating": 5.0},
                        {"key": "trend", "count": 1, "total_pnl": 12.5, "win_rate": 100.0, "avg_rating": 5.0}
                    ],
                    "total_entries": 4
                }),
            ),
            (
                "",
                "strategy",
                json!({
                    "group_by": "strategy",
                    "groups": [
                        {"key": "strategy_a", "count": 2, "total_pnl": 19.5, "win_rate": 100.0},
                        {"key": "strategy_b", "count": 1, "total_pnl": 0.0, "win_rate": 0.0},
                        {"key": "strategy_c", "count": 1, "total_pnl": 8.0, "win_rate": 100.0},
                        {"key": "未知", "count": 1, "total_pnl": -4.0, "win_rate": 0.0}
                    ],
                    "total_entries": 5
                }),
            ),
            (
                "simulated",
                "strategy",
                json!({
                    "group_by": "strategy",
                    "groups": [
                        {"key": "strategy_a", "count": 2, "total_pnl": 19.5, "win_rate": 100.0},
                        {"key": "strategy_c", "count": 1, "total_pnl": 8.0, "win_rate": 100.0},
                        {"key": "未知", "count": 1, "total_pnl": -4.0, "win_rate": 0.0}
                    ],
                    "total_entries": 4
                }),
            ),
        ] {
            let actual = journal_stats_from_pool(&pool, mode, group_by)
                .await
                .expect("journal stats");
            assert_eq!(
                actual, expected,
                "journal stats contract changed for mode={mode:?} group_by={group_by:?}"
            );
        }

        cleanup_db(pool, &db_path).await;
    }

    async fn insert_contract_rows(pool: &sqlx::SqlitePool) {
        sqlx::query(
            r#"
            INSERT INTO journal_entries (
              entry_id, title, content, mode, inst_id, inst_type,
              trade_ids_json, order_ids_json, tags_json,
              strategy_id, strategy_name, rating, emotion,
              screenshots_json, pnl_snapshot, metadata_json,
              created_at, updated_at
            ) VALUES
              ('je_1', 'one', 'content', 'simulated', 'BTC-USDT-SWAP', 'SWAP',
               '[]', '[]', '["trend","breakout"]',
               'strategy_a', 'A', 5, '', '[]', 12.5, '{}',
               '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'),
              ('je_2', 'two', 'content', 'simulated', 'ETH-USDT-SWAP', 'SWAP',
               '[]', '[]', '[]',
               '', '', 3, '', '[]', -4.0, '{}',
               '2026-01-02T00:00:00Z', '2026-01-02T00:00:00Z'),
              ('je_3', 'three', 'content', 'live', 'BTC-USDT-SWAP', 'SWAP',
               '[]', '[]', '["trend"]',
               'strategy_b', 'B', 1, '', '[]', 0.0, '{}',
               '2026-01-03T00:00:00Z', '2026-01-03T00:00:00Z'),
              ('je_4', 'invalid tags', 'content', 'simulated', 'SOL-USDT-SWAP', 'SWAP',
               '[]', '[]', 'not-json',
               'strategy_a', 'A', 4, '', '[]', 7.0, '{}',
               '2026-01-04T00:00:00Z', '2026-01-04T00:00:00Z'),
              ('je_5', 'non string tags', 'content', 'simulated', 'XRP-USDT-SWAP', 'SWAP',
               '[]', '[]', '[1,false]',
               'strategy_c', 'C', 2, '', '[]', 8.0, '{}',
               '2026-01-05T00:00:00Z', '2026-01-05T00:00:00Z')
            "#,
        )
        .execute(pool)
        .await
        .unwrap();
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
