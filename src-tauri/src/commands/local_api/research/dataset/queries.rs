use super::{super::*, rows::dataset_row_to_json};

pub(crate) async fn fetch_research_dataset(
    state: &AppState,
    dataset_id: &str,
) -> AppResult<Option<Value>> {
    let row = sqlx::query("SELECT * FROM research_dataset_manifests WHERE dataset_id = ?")
        .bind(dataset_id)
        .fetch_optional(&state.db)
        .await?;
    row.map(dataset_row_to_json).transpose()
}

pub(crate) async fn research_datasets(state: &AppState, req: &LocalApiRequest) -> AppResult<Value> {
    let limit = param_i64(req, "limit", 50).clamp(1, 500);
    let rows =
        sqlx::query("SELECT * FROM research_dataset_manifests ORDER BY updated_at DESC LIMIT ?")
            .bind(limit)
            .fetch_all(&state.db)
            .await?;
    let datasets = rows
        .into_iter()
        .map(dataset_row_to_json)
        .collect::<AppResult<Vec<_>>>()?;
    Ok(Value::Array(datasets))
}

pub(crate) async fn research_dataset_detail(
    state: &AppState,
    dataset_id: &str,
) -> AppResult<Value> {
    let Some(dataset) = fetch_research_dataset(state, dataset_id).await? else {
        return Err(AppError::Validation(
            "dataset manifest not found".to_string(),
        ));
    };
    Ok(dataset)
}
