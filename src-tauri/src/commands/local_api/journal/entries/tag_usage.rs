use serde_json::Value;

use crate::{app_state::AppState, error::AppResult};

pub(super) async fn increment_journal_tag_usage(state: &AppState, tags: &Value) -> AppResult<()> {
    if let Some(items) = tags.as_array() {
        for tag in items
            .iter()
            .filter_map(Value::as_str)
            .filter(|tag| !tag.trim().is_empty())
        {
            sqlx::query(
                r#"
                INSERT INTO journal_tags (tag, usage_count, created_at)
                VALUES (?, 1, CURRENT_TIMESTAMP)
                ON CONFLICT(tag) DO UPDATE SET usage_count = journal_tags.usage_count + 1
                "#,
            )
            .bind(tag)
            .execute(&state.db)
            .await?;
        }
    }
    Ok(())
}
