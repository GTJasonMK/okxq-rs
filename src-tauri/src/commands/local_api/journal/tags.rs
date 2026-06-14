use serde_json::{json, Value};
use sqlx::Row;

use crate::{app_state::AppState, commands::local_api::code_ok, error::AppResult};

pub(crate) async fn journal_tags(state: &AppState) -> AppResult<Value> {
    let rows = sqlx::query(
        "SELECT tag, color, usage_count, created_at FROM journal_tags ORDER BY usage_count DESC",
    )
    .fetch_all(&state.db)
    .await?;
    let data = rows
        .into_iter()
        .map(|row| {
            Ok(json!({
                "tag": row.try_get::<String, _>("tag")?,
                "color": row.try_get::<String, _>("color")?,
                "usage_count": row.try_get::<i64, _>("usage_count")?,
                "created_at": row.try_get::<Option<String>, _>("created_at")?
            }))
        })
        .collect::<AppResult<Vec<_>>>()?;
    Ok(code_ok(Value::Array(data)))
}
