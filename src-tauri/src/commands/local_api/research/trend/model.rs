use serde_json::{json, Map, Value};

use super::{
    super::*,
    payload::{trend_meta_payload, trend_model_status},
};

pub(crate) async fn trend_research_model(state: &AppState) -> AppResult<Value> {
    let meta = trend_meta_payload(state, 0).await;
    Ok(json!({
        "model": trend_model_status(),
        "enabled": meta.get("enabled").cloned().unwrap_or(Value::Bool(false)),
        "status": meta.get("status").cloned().unwrap_or(Value::String("disabled".to_string())),
        "whitelist": meta.get("whitelist").cloned().unwrap_or_else(|| Value::Array(vec![])),
        "runtime_error": "",
        "model_status": trend_model_status()
    }))
}

pub(crate) async fn trend_research_training_run(state: &AppState) -> AppResult<Value> {
    let row = sqlx::query("SELECT * FROM research_training_runs ORDER BY updated_at DESC LIMIT 1")
        .fetch_optional(&state.db)
        .await?;
    let training_run = row
        .map(training_run_row_to_json)
        .transpose()?
        .unwrap_or_else(|| json!({}));
    let mut meta = trend_meta_payload(state, 0).await;
    if let Some(obj) = meta.as_object_mut() {
        obj.insert("training_run".to_string(), training_run);
    }
    Ok(meta)
}

pub(crate) async fn retrain_trend_research_model(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let latest_dataset =
        sqlx::query("SELECT * FROM research_dataset_manifests ORDER BY updated_at DESC LIMIT 1")
            .fetch_optional(&state.db)
            .await?
            .map(dataset_row_to_json)
            .transpose()?;
    let training_run = if let Some(dataset) = latest_dataset {
        let mut request = LocalApiRequest {
            method: "POST".to_string(),
            path: "/api/research-platform/training-runs".to_string(),
            params: Map::new(),
            body: json!({
                "dataset_id": value_string_at(&dataset, "dataset_id", ""),
                "candidate_set_ref": "candidate://trend-research/retrain",
                "model_family": "joint_density_model_v1",
                "model_spec_ref": "model://joint_density_model_v1/retrain",
                "training_seed": request_i64(req, "training_seed", 7)
            }),
        };
        request.params.insert(
            "lookback".to_string(),
            Value::from(request_i64(req, "lookback", 3600)),
        );
        create_research_training_run(state, &request).await?
    } else {
        json!({
            "run_id": generated_id("tr"),
            "status": "skipped",
            "progress_stage": "no_dataset",
            "created_at": now_unix_seconds(),
            "updated_at": now_unix_seconds()
        })
    };
    let mut meta = trend_meta_payload(state, 0).await;
    if let Some(obj) = meta.as_object_mut() {
        obj.insert("model".to_string(), trend_model_status());
        obj.insert("training_run".to_string(), training_run);
    }
    Ok(meta)
}
