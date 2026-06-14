use sqlx::Row;

use super::{super::*, queries::fetch_research_dataset};

pub(crate) async fn delete_research_dataset(
    state: &AppState,
    dataset_id: &str,
) -> AppResult<Value> {
    let Some(dataset) = fetch_research_dataset(state, dataset_id).await? else {
        return Err(AppError::Validation(
            "dataset manifest not found".to_string(),
        ));
    };
    let training_count =
        sqlx::query("SELECT COUNT(*) AS count FROM research_training_runs WHERE dataset_id = ?")
            .bind(dataset_id)
            .fetch_one(&state.db)
            .await?
            .try_get::<i64, _>("count")?;
    if training_count > 0 {
        return Err(AppError::Validation(format!(
            "dataset has {training_count} training run(s)"
        )));
    }
    sqlx::query("DELETE FROM research_dataset_manifests WHERE dataset_id = ?")
        .bind(dataset_id)
        .execute(&state.db)
        .await?;
    Ok(json!({ "deleted_dataset": dataset }))
}
