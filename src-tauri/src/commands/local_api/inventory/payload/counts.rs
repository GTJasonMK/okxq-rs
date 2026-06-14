use super::entries::{add_i64_field, ensure_inventory_entry};
use super::*;
use crate::commands::local_api::inventory::deletion::symbol_related_storage_counts;

pub(super) async fn apply_count_table(
    pool: &SqlitePool,
    entries: &mut BTreeMap<String, Value>,
    table: &str,
    column: &str,
    count_key: &str,
) -> AppResult<()> {
    let sql =
        format!("SELECT {column} AS symbol, COUNT(*) AS count FROM {table} GROUP BY {column}");
    for row in sqlx::query(&sql).fetch_all(pool).await? {
        let raw = row.try_get::<String, _>("symbol")?;
        let Some(symbol) = normalize_symbol(&raw) else {
            continue;
        };
        let count = row.try_get::<i64, _>("count")?;
        let entry = ensure_inventory_entry(entries, &symbol);
        if let Some(counts) = entry
            .get_mut("storage_counts")
            .and_then(Value::as_object_mut)
        {
            add_i64_field(counts, count_key, count);
        }
    }
    Ok(())
}

pub(super) async fn apply_cost_basis_counts(
    pool: &SqlitePool,
    entries: &mut BTreeMap<String, Value>,
) -> AppResult<()> {
    for row in sqlx::query("SELECT ccy, COUNT(*) AS count FROM cost_basis GROUP BY ccy")
        .fetch_all(pool)
        .await?
    {
        let ccy = row.try_get::<String, _>("ccy")?;
        let symbol = format!("{}-USDT", ccy.trim().to_uppercase());
        let count = row.try_get::<i64, _>("count")?;
        let entry = ensure_inventory_entry(entries, &symbol);
        if let Some(counts) = entry
            .get_mut("storage_counts")
            .and_then(Value::as_object_mut)
        {
            add_i64_field(counts, "cost_basis", count);
        }
    }
    Ok(())
}

pub(super) async fn apply_deletion_mark_residual_counts(
    pool: &SqlitePool,
    entries: &mut BTreeMap<String, Value>,
    deletion_marks: &std::collections::BTreeSet<String>,
) -> AppResult<()> {
    for symbol in deletion_marks {
        let counts = symbol_related_storage_counts(pool, symbol).await?;
        let entry = ensure_inventory_entry(entries, symbol);
        if let Some(storage_counts) = entry
            .get_mut("storage_counts")
            .and_then(Value::as_object_mut)
        {
            for (key, count) in counts {
                storage_counts.insert(key, Value::Number(count.into()));
            }
        }
    }
    Ok(())
}

pub(super) async fn apply_local_fills_counts(
    pool: &SqlitePool,
    entries: &mut BTreeMap<String, Value>,
) -> AppResult<()> {
    let rows = sqlx::query(
        r#"
        SELECT CASE
          WHEN inst_id IS NULL OR inst_id = '' THEN ccy || '-USDT'
          ELSE inst_id
        END AS symbol, COUNT(*) AS count
        FROM local_fills
        GROUP BY symbol
        "#,
    )
    .fetch_all(pool)
    .await?;
    for row in rows {
        let raw = row.try_get::<String, _>("symbol")?;
        let Some(symbol) = normalize_symbol(&raw) else {
            continue;
        };
        let count = row.try_get::<i64, _>("count")?;
        let entry = ensure_inventory_entry(entries, &symbol);
        if let Some(counts) = entry
            .get_mut("storage_counts")
            .and_then(Value::as_object_mut)
        {
            add_i64_field(counts, "local_fills", count);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn memory_pool() -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory sqlite")
    }

    #[tokio::test]
    async fn apply_count_table_reads_fixed_aliases_without_defaults() {
        let pool = memory_pool().await;
        sqlx::query("CREATE TABLE sample_counts (inst_id TEXT NOT NULL)")
            .execute(&pool)
            .await
            .expect("create table");
        sqlx::query("INSERT INTO sample_counts (inst_id) VALUES ('btc-usdt'), ('BTC-USDT')")
            .execute(&pool)
            .await
            .expect("insert rows");

        let mut entries = BTreeMap::new();
        apply_count_table(
            &pool,
            &mut entries,
            "sample_counts",
            "inst_id",
            "sample_counts",
        )
        .await
        .expect("apply counts");

        let entry = entries.get("BTC-USDT").expect("BTC entry");
        assert_eq!(entry["storage_counts"]["sample_counts"], 2);
    }

    #[tokio::test]
    async fn apply_count_table_rejects_dirty_symbol_alias() {
        let pool = memory_pool().await;
        sqlx::query("CREATE TABLE sample_counts (inst_id TEXT NOT NULL)")
            .execute(&pool)
            .await
            .expect("create table");
        sqlx::query("INSERT INTO sample_counts (inst_id) VALUES ('BTC-USDT')")
            .execute(&pool)
            .await
            .expect("insert rows");

        let mut entries = BTreeMap::new();
        let result = apply_count_table(
            &pool,
            &mut entries,
            "sample_counts",
            "x'ff'",
            "sample_counts",
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn apply_cost_basis_counts_rejects_dirty_ccy() {
        let pool = memory_pool().await;
        sqlx::query("CREATE TABLE cost_basis (ccy TEXT)")
            .execute(&pool)
            .await
            .expect("create table");
        sqlx::query("INSERT INTO cost_basis (ccy) VALUES (x'ff')")
            .execute(&pool)
            .await
            .expect("insert dirty row");

        let mut entries = BTreeMap::new();
        let result = apply_cost_basis_counts(&pool, &mut entries).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn apply_local_fills_counts_rejects_dirty_generated_symbol() {
        let pool = memory_pool().await;
        sqlx::query(
            r#"
            CREATE TABLE local_fills (
              inst_id TEXT,
              ccy TEXT
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create table");
        sqlx::query("INSERT INTO local_fills (inst_id, ccy) VALUES ('', x'ff')")
            .execute(&pool)
            .await
            .expect("insert dirty row");

        let mut entries = BTreeMap::new();
        let result = apply_local_fills_counts(&pool, &mut entries).await;

        assert!(result.is_err());
    }
}
