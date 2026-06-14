use super::{
    super::*,
    payload::trend_meta_payload,
    queries::{trend_factor_rows, trend_feature_rows},
};

pub(crate) async fn trend_research_feature_bars(
    state: &AppState,
    inst_id: &str,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let requested_inst_type = request_string(req, "inst_type", "");
    let (inst_id, inst_type) = resolve_research_scope(state, inst_id, &requested_inst_type).await?;
    let rows = trend_feature_rows(state, &inst_id, param_i64(req, "limit", 120)).await?;
    let mut payload = trend_meta_payload(state, rows.len()).await;
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("inst_id".to_string(), Value::String(inst_id));
        obj.insert("inst_type".to_string(), Value::String(inst_type));
        obj.insert("rows".to_string(), Value::Array(rows));
    }
    Ok(payload)
}

pub(crate) async fn trend_research_factors(
    state: &AppState,
    inst_id: &str,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let requested_inst_type = request_string(req, "inst_type", "");
    let timeframe = request_string(req, "timeframe", "1H");
    let (inst_id, inst_type) = resolve_research_scope(state, inst_id, &requested_inst_type).await?;
    let rows = trend_factor_rows(state, &inst_id, param_i64(req, "limit", 20)).await?;
    let mut payload = trend_meta_payload(state, rows.len()).await;
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("inst_id".to_string(), Value::String(inst_id));
        obj.insert("inst_type".to_string(), Value::String(inst_type));
        obj.insert("timeframe".to_string(), Value::String(timeframe));
        obj.insert(
            "lookback".to_string(),
            Value::from(param_i64(req, "lookback", 3600)),
        );
        obj.insert("rows".to_string(), Value::Array(rows));
    }
    Ok(payload)
}

pub(crate) async fn trend_research_factor_series(
    state: &AppState,
    inst_id: &str,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let lookback = param_i64(req, "lookback", 3600);
    let requested_inst_type = request_string(req, "inst_type", "");
    let (inst_id, inst_type) = resolve_research_scope(state, inst_id, &requested_inst_type).await?;
    let score_meta = trend_factor_rows(state, &inst_id, 100).await?;
    let feature_rows = trend_feature_rows(state, &inst_id, lookback).await?;
    let second_buckets = feature_rows
        .iter()
        .map(|row| {
            row.get("second_bucket")
                .and_then(Value::as_i64)
                .map(Value::from)
                .ok_or_else(|| {
                    AppError::Runtime("trend feature row 缺少 second_bucket".to_string())
                })
        })
        .collect::<AppResult<Vec<_>>>()?;
    Ok(json!({
        "inst_id": inst_id,
        "inst_type": inst_type,
        "lookback": lookback,
        "second_buckets": second_buckets,
        "score_meta": score_meta,
        "series": []
    }))
}
