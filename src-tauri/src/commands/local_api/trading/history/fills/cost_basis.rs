use sqlx::{sqlite::SqliteRow, Row};

use super::super::super::values::finite_text_f64;
use super::super::super::*;

pub(crate) async fn cost_basis(state: &AppState, req: &LocalApiRequest) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let ccy = param_string(req, "ccy", "");
    let mut sql = "SELECT * FROM cost_basis WHERE mode = ?".to_string();
    if !ccy.is_empty() {
        sql.push_str(" AND ccy = ?");
    }
    sql.push_str(" ORDER BY ccy");
    let mut query = sqlx::query(&sql).bind(mode);
    if !ccy.is_empty() {
        query = query.bind(ccy);
    }
    let rows = query.fetch_all(&state.db).await?;
    let data = rows
        .into_iter()
        .map(cost_basis_row_to_json)
        .collect::<AppResult<Vec<_>>>()?;
    Ok(Value::Array(data))
}

fn cost_basis_row_to_json(row: SqliteRow) -> AppResult<Value> {
    Ok(json!({
        "ccy": row.try_get::<String, _>("ccy")?,
        "mode": row.try_get::<String, _>("mode")?,
        "avg_cost": text_f64_column(&row, "avg_cost")?,
        "total_qty": text_f64_column(&row, "total_qty")?,
        "total_cost": text_f64_column(&row, "total_cost")?,
        "total_fee": text_f64_column(&row, "total_fee")?,
        "total_buy_cost": text_f64_column(&row, "total_buy_cost")?,
        "total_sell_revenue": text_f64_column(&row, "total_sell_revenue")?,
        "last_updated": row.try_get::<Option<String>, _>("last_updated")?
    }))
}

fn text_f64_column(row: &SqliteRow, column: &str) -> AppResult<Option<f64>> {
    let text = row.try_get::<String, _>(column)?;
    Ok(finite_text_f64(&text))
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;

    #[tokio::test]
    async fn cost_basis_row_does_not_fabricate_zero_from_invalid_text_numbers() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should connect");
        let row = sqlx::query(
            r#"
            SELECT
              'BTC' AS ccy,
              'simulated' AS mode,
              'bad-average' AS avg_cost,
              'bad-quantity' AS total_qty,
              'bad-cost' AS total_cost,
              'bad-fee' AS total_fee,
              'bad-buy-cost' AS total_buy_cost,
              'bad-sell-revenue' AS total_sell_revenue,
              NULL AS last_updated
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("row should be returned");

        let cost = cost_basis_row_to_json(row).expect("cost basis row");

        assert_eq!(cost["ccy"], "BTC");
        assert!(cost["avg_cost"].is_null());
        assert!(cost["total_qty"].is_null());
        assert!(cost["total_cost"].is_null());
        assert!(cost["total_fee"].is_null());
    }

    #[tokio::test]
    async fn cost_basis_row_rejects_missing_required_columns() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should connect");
        let row = sqlx::query(
            r#"
            SELECT
              'BTC' AS ccy,
              'bad-average' AS avg_cost,
              'bad-quantity' AS total_qty,
              'bad-cost' AS total_cost,
              'bad-fee' AS total_fee,
              'bad-buy-cost' AS total_buy_cost,
              'bad-sell-revenue' AS total_sell_revenue,
              NULL AS last_updated
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("row should be returned");

        assert!(cost_basis_row_to_json(row).is_err());
    }
}
