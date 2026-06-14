use sqlx::{sqlite::SqliteRow, Row, SqlitePool};

use super::super::*;

const MAX_SNAPSHOT_SCAN_ROWS: i64 = 365;

pub(super) async fn portfolio_snapshots(
    state: &AppState,
    mode: &str,
    days: i64,
) -> AppResult<Vec<Value>> {
    portfolio_snapshots_from_pool(&state.db, mode, days).await
}

pub(super) async fn portfolio_snapshots_from_pool(
    pool: &SqlitePool,
    mode: &str,
    days: i64,
) -> AppResult<Vec<Value>> {
    let requested = days.clamp(1, MAX_SNAPSHOT_SCAN_ROWS);
    let mut data = fetch_portfolio_snapshot_rows(pool, mode, requested)
        .await?
        .into_iter()
        .map(portfolio_snapshot_row_to_json)
        .collect::<AppResult<Vec<_>>>()?;
    data.reverse();
    Ok(data)
}

async fn fetch_portfolio_snapshot_rows(
    pool: &SqlitePool,
    mode: &str,
    limit: i64,
) -> AppResult<Vec<SqliteRow>> {
    Ok(sqlx::query(
        r#"
        SELECT mode, date, total_equity, spot_value, contract_value,
               cash_value, positions_json, metadata_json, created_at
        FROM portfolio_snapshots
        WHERE mode = ?
        ORDER BY date DESC
        LIMIT ?
        "#,
    )
    .bind(mode)
    .bind(limit)
    .fetch_all(pool)
    .await?)
}

fn portfolio_snapshot_row_to_json(row: SqliteRow) -> AppResult<Value> {
    let total_equity = row_required_f64(&row, "total_equity")?;
    let spot_value = row_optional_f64(&row, "spot_value", 0.0)?;
    let contract_value = row_optional_f64(&row, "contract_value", 0.0)?;
    let cash_value = row_optional_f64(&row, "cash_value", 0.0)?;
    Ok(json!({
        "mode": row.try_get::<String, _>("mode")?,
        "date": row.try_get::<String, _>("date")?,
        "total_equity": total_equity,
        "spot_value": spot_value,
        "contract_value": contract_value,
        "cash_value": cash_value,
        "positions": json_object_column(&row, "positions_json")?,
        "metadata": json_object_column(&row, "metadata_json")?,
        "created_at": row.try_get::<String, _>("created_at")?
    }))
}

fn row_required_f64(row: &SqliteRow, column: &str) -> AppResult<f64> {
    let value = row.try_get::<f64, _>(column).map_err(|error| {
        AppError::Runtime(format!("读取 portfolio snapshot {column} 失败: {error}"))
    })?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(AppError::Runtime(format!(
            "portfolio snapshot {column} 不是有限数字"
        )))
    }
}

fn row_optional_f64(row: &SqliteRow, column: &str, default_value: f64) -> AppResult<f64> {
    let value = row
        .try_get::<Option<f64>, _>(column)
        .map_err(|error| {
            AppError::Runtime(format!("读取 portfolio snapshot {column} 失败: {error}"))
        })?
        .unwrap_or(default_value);
    if value.is_finite() {
        Ok(value)
    } else {
        Err(AppError::Runtime(format!(
            "portfolio snapshot {column} 不是有限数字"
        )))
    }
}

fn json_object_column(row: &SqliteRow, column: &str) -> AppResult<Value> {
    let text = row.try_get::<String, _>(column).map_err(|error| {
        AppError::Runtime(format!("读取 portfolio snapshot {column} 失败: {error}"))
    })?;
    let value = serde_json::from_str::<Value>(&text).map_err(|error| {
        AppError::Runtime(format!("解析 portfolio snapshot {column} 失败: {error}"))
    })?;
    match value {
        Value::Object(_) => Ok(value),
        _ => Err(AppError::Runtime(format!(
            "portfolio snapshot {column} 不是 JSON 对象"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

    #[tokio::test]
    async fn portfolio_snapshots_reject_dirty_required_equity_rows() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory sqlite");
        create_snapshot_table(&pool).await;

        sqlx::query(
            r#"
            INSERT INTO portfolio_snapshots
                (mode, date, total_equity, spot_value, contract_value, cash_value)
            VALUES
                ('simulated', '2026-05-29', 'bad-equity', 'bad-spot', 'bad-contract', 'bad-cash'),
                ('simulated', '2026-05-28', 1234.5, 300.0, 200.0, 734.5)
            "#,
        )
        .execute(&pool)
        .await
        .expect("insert snapshots");

        let error = portfolio_snapshots_from_pool(&pool, "simulated", 1)
            .await
            .expect_err("dirty snapshot should fail");

        assert!(error.to_string().contains("total_equity"), "{error}");
    }

    async fn create_snapshot_table(pool: &SqlitePool) {
        sqlx::query(
            r#"
            CREATE TABLE portfolio_snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                mode TEXT NOT NULL,
                date TEXT NOT NULL,
                total_equity REAL NOT NULL,
                spot_value REAL DEFAULT 0,
                contract_value REAL DEFAULT 0,
                cash_value REAL DEFAULT 0,
                positions_json TEXT DEFAULT '{}',
                metadata_json TEXT DEFAULT '{}',
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(mode, date)
            )
            "#,
        )
        .execute(pool)
        .await
        .expect("create table");
    }
}
