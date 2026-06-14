use serde_json::Value;

use crate::{app_state::AppState, error::AppResult};

use super::rows::{factor_score_row_to_json, feature_bar_row_to_json, inference_row_to_json};

pub(super) async fn trend_inference_rows(state: &AppState, limit: i64) -> AppResult<Vec<Value>> {
    let rows = sqlx::query("SELECT * FROM inference_snapshots ORDER BY created_at DESC LIMIT ?")
        .bind(limit.clamp(1, 500))
        .fetch_all(&state.db)
        .await?;
    rows.into_iter()
        .map(inference_row_to_json)
        .collect::<AppResult<Vec<_>>>()
}

pub(super) async fn trend_feature_rows(
    state: &AppState,
    inst_id: &str,
    limit: i64,
) -> AppResult<Vec<Value>> {
    let rows =
        sqlx::query("SELECT * FROM feature_bars_1s WHERE inst_id = ? ORDER BY ts DESC LIMIT ?")
            .bind(inst_id)
            .bind(limit.clamp(1, 2000))
            .fetch_all(&state.db)
            .await?;
    let mut rows = rows
        .into_iter()
        .map(feature_bar_row_to_json)
        .collect::<AppResult<Vec<_>>>()?;
    rows.reverse();
    Ok(rows)
}

pub(super) async fn trend_factor_rows(
    state: &AppState,
    inst_id: &str,
    limit: i64,
) -> AppResult<Vec<Value>> {
    let rows = sqlx::query(
        "SELECT * FROM factor_scores WHERE inst_id = ? ORDER BY created_at DESC LIMIT ?",
    )
    .bind(inst_id)
    .bind(limit.clamp(1, 200))
    .fetch_all(&state.db)
    .await?;
    rows.into_iter()
        .map(factor_score_row_to_json)
        .collect::<AppResult<Vec<_>>>()
}
