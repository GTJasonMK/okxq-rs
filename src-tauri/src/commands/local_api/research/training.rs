use sqlx::{sqlite::SqliteRow, Row};

use super::*;

pub(crate) fn training_run_row_to_json(row: SqliteRow) -> AppResult<Value> {
    let payload_text = row.try_get::<String, _>("payload_json")?;
    let mut run = json_object_from_text(&payload_text, "training run payload_json")?;
    run.insert(
        "run_id".to_string(),
        Value::String(row.try_get::<String, _>("run_id")?),
    );
    run.insert(
        "dataset_id".to_string(),
        Value::String(row.try_get::<String, _>("dataset_id")?),
    );
    run.insert(
        "status".to_string(),
        Value::String(row.try_get::<String, _>("status")?),
    );
    run.insert(
        "progress_stage".to_string(),
        Value::String(row.try_get::<String, _>("progress_stage")?),
    );
    run.insert(
        "created_at".to_string(),
        Value::from(row.try_get::<f64, _>("created_at")?),
    );
    run.insert(
        "updated_at".to_string(),
        Value::from(row.try_get::<f64, _>("updated_at")?),
    );
    Ok(Value::Object(run))
}

async fn fetch_training_run(state: &AppState, run_id: &str) -> AppResult<Option<Value>> {
    let row = sqlx::query("SELECT * FROM research_training_runs WHERE run_id = ?")
        .bind(run_id)
        .fetch_optional(&state.db)
        .await?;
    row.map(training_run_row_to_json).transpose()
}

pub(crate) async fn research_training_runs(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let dataset_id = request_string(req, "dataset_id", "");
    let limit = param_i64(req, "limit", 50).clamp(1, 500);
    let rows = if dataset_id.trim().is_empty() {
        sqlx::query("SELECT * FROM research_training_runs ORDER BY updated_at DESC LIMIT ?")
            .bind(limit)
            .fetch_all(&state.db)
            .await?
    } else {
        sqlx::query(
            "SELECT * FROM research_training_runs WHERE dataset_id = ? ORDER BY updated_at DESC LIMIT ?",
        )
        .bind(dataset_id)
        .bind(limit)
        .fetch_all(&state.db)
        .await?
    };
    let runs = rows
        .into_iter()
        .map(training_run_row_to_json)
        .collect::<AppResult<Vec<_>>>()?;
    Ok(Value::Array(runs))
}

pub(crate) async fn research_training_run_detail(
    state: &AppState,
    run_id: &str,
) -> AppResult<Value> {
    let Some(training_run) = fetch_training_run(state, run_id).await? else {
        return Err(AppError::Validation("training run not found".to_string()));
    };
    Ok(training_run)
}

pub(crate) async fn create_research_training_run(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let dataset_id = request_string(req, "dataset_id", "");
    if dataset_id.trim().is_empty() {
        return Err(AppError::Validation("dataset_id is required".to_string()));
    }
    let Some(dataset) = fetch_research_dataset(state, &dataset_id).await? else {
        return Err(AppError::Validation(
            "dataset manifest not found".to_string(),
        ));
    };
    let now = now_unix_seconds();
    let run_id = body_string(req, "run_id", "");
    let run_id = if run_id.trim().is_empty() {
        generated_id("tr")
    } else {
        run_id
    };
    let qualified_sample_count = value_i64_at(&dataset, "target_census_count", 0);
    let model_family = request_string(req, "model_family", "joint_density_model_v1");
    let mut run = Map::new();
    run.insert("run_id".to_string(), Value::String(run_id.clone()));
    run.insert("dataset_id".to_string(), Value::String(dataset_id.clone()));
    run.insert("status".to_string(), Value::String("completed".to_string()));
    run.insert(
        "progress_stage".to_string(),
        Value::String("completed".to_string()),
    );
    run.insert("created_at".to_string(), Value::from(now));
    run.insert("updated_at".to_string(), Value::from(now));
    run.insert("model_family".to_string(), Value::String(model_family));
    run.insert(
        "model_spec_ref".to_string(),
        Value::String(request_string(
            req,
            "model_spec_ref",
            "model://joint_density_model_v1/default-v1",
        )),
    );
    run.insert(
        "candidate_set_ref".to_string(),
        Value::String(request_string(
            req,
            "candidate_set_ref",
            "candidate://locked/default-v1",
        )),
    );
    run.insert(
        "training_seed".to_string(),
        Value::from(request_i64(req, "training_seed", 7).max(0)),
    );
    for key in [
        "split_definition_version",
        "evaluation_protocol_version",
        "refit_policy_version",
        "outer_origin_selection_policy",
        "weighting_version",
        "score_definition_version",
        "policy_parameter_ref",
        "policy_definition_version",
        "decision_utility_version",
        "utility_parameter_ref",
        "execution_assumption_version",
        "multiple_comparison_version",
    ] {
        run.insert(
            key.to_string(),
            Value::String(value_string_at(&dataset, key, "")),
        );
    }
    run.insert(
        "artifacts".to_string(),
        json!({
            "split_artifact": {
                "qualified_sample_count": qualified_sample_count,
                "outer_origin_selection_policy": value_string_at(&dataset, "outer_origin_selection_policy", "all_completed_sessions_v1"),
                "refit_policy_version": value_string_at(&dataset, "refit_policy_version", ""),
                "origins": []
            },
            "policy_parameter_bundle": {"by_origin": []},
            "utility_parameter_bundle": {"by_origin": []},
            "execution_assumption_bundle": {
                "execution_assumption_version": value_string_at(&dataset, "execution_assumption_version", "")
            },
            "comparison_result": {
                "status": "completed",
                "candidate_count": 1
            },
            "baseline_result": {
                "status": "completed",
                "baseline": "holdout_zero_return"
            },
            "weighted_diagnostics": {"by_origin": []},
            "unweighted_diagnostics": {"by_origin": []}
        }),
    );

    sqlx::query(
        "INSERT INTO research_training_runs (run_id, dataset_id, status, progress_stage, payload_json, created_at, updated_at) VALUES (?, ?, 'completed', 'completed', ?, ?, ?)",
    )
    .bind(&run_id)
    .bind(&dataset_id)
    .bind(serde_json::to_string(&Value::Object(run))?)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await?;

    research_training_run_detail(state, &run_id).await
}

fn json_object_from_text(text: &str, context: &str) -> AppResult<Map<String, Value>> {
    let value = serde_json::from_str::<Value>(text)?;
    match value {
        Value::Object(object) => Ok(object),
        _ => Err(AppError::Runtime(format!("{context} 不是 JSON 对象"))),
    }
}

#[cfg(test)]
mod tests {
    use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

    use super::*;

    async fn test_pool() -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory sqlite")
    }

    #[tokio::test]
    async fn training_run_row_rejects_invalid_payload_json() {
        let pool = test_pool().await;
        let row = sqlx::query(
            r#"
            SELECT
              'run_a' AS run_id,
              'dataset_a' AS dataset_id,
              'completed' AS status,
              'completed' AS progress_stage,
              'not-json' AS payload_json,
              1.0 AS created_at,
              1.0 AS updated_at
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("training row");

        assert!(training_run_row_to_json(row).is_err());
    }
}
