use super::{
    super::*,
    payload::trend_meta_payload,
    queries::{trend_feature_rows, trend_inference_rows},
};

pub(crate) async fn trend_research_inference(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let rows = trend_inference_rows(state, param_i64(req, "limit", 100)).await?;
    let mut payload = trend_meta_payload(state, rows.len()).await;
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("rows".to_string(), Value::Array(rows));
    }
    Ok(payload)
}

pub(crate) async fn trend_research_process(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let bar_limit = param_i64(req, "bar_limit", 20).clamp(1, 120);
    let cfg = state.config.read().await.trend_research.clone();
    let inference_rows = trend_inference_rows(state, 100).await?;
    let mut instruments = Vec::new();
    let mut feature_ready_count = 0;
    let mut inference_ready_count = 0;
    for inst_id in &cfg.whitelist {
        let feature_bars = trend_feature_rows(state, inst_id, bar_limit).await?;
        let latest_feature_bar = feature_bars.last().cloned().unwrap_or(Value::Null);
        let latest_inference = inference_rows
            .iter()
            .find(|row| value_string_at(row, "inst_id", "") == *inst_id)
            .cloned()
            .unwrap_or(Value::Null);
        let has_feature = !latest_feature_bar.is_null();
        let has_inference = !latest_inference.is_null();
        if has_feature {
            feature_ready_count += 1;
        }
        if has_inference {
            inference_ready_count += 1;
        }
        let pipeline_state = if has_inference {
            "inference_ready"
        } else if has_feature {
            "feature_ready"
        } else {
            "collecting"
        };
        instruments.push(json!({
            "inst_id": inst_id,
            "pipeline_state": pipeline_state,
            "stages": {
                "trade": {"ready": has_feature, "label": "trade"},
                "book": {"ready": has_feature, "label": "book"},
                "state": {"ready": has_feature, "label": "state"},
                "feature": {"ready": has_feature, "label": "feature"},
                "inference": {"ready": has_inference, "label": "inference"}
            },
            "runtime": {
                "pending_trade_count": 0,
                "last_trade_price": latest_feature_bar.get("close").cloned().unwrap_or(Value::Null),
                "last_trade_side": ""
            },
            "latest_feature_bar": latest_feature_bar,
            "latest_inference": latest_inference,
            "recent_feature_bars": feature_bars
        }));
    }
    let summary = json!({
        "whitelist_count": cfg.whitelist.len(),
        "trade_ready_count": feature_ready_count,
        "book_ready_count": feature_ready_count,
        "state_ready_count": feature_ready_count,
        "feature_ready_count": feature_ready_count,
        "inference_ready_count": inference_ready_count
    });
    let mut payload = trend_meta_payload(state, inference_rows.len()).await;
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("summary".to_string(), summary);
        obj.insert("instruments".to_string(), Value::Array(instruments));
        obj.insert("bar_limit".to_string(), Value::from(bar_limit));
    }
    Ok(payload)
}

pub(crate) async fn trend_research_diagnostics(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let cfg = state.config.read().await.trend_research.clone();
    let selected_inst_id = request_string(req, "inst_id", "").trim().to_uppercase();
    let selected_inst_id = if selected_inst_id.is_empty() {
        cfg.whitelist.first().cloned().unwrap_or_default()
    } else {
        selected_inst_id
    };
    let process = trend_research_process(state, req).await?;
    let instruments = process
        .get("instruments")
        .cloned()
        .unwrap_or_else(|| Value::Array(vec![]));
    let summary = process.get("summary").cloned().unwrap_or_else(|| json!({}));
    Ok(json!({
        "event_type": "snapshot",
        "selected_inst_id": selected_inst_id,
        "instruments": instruments,
        "global_health": {
            "whitelist_count": cfg.whitelist.len(),
            "status": process.get("status").cloned().unwrap_or_else(|| Value::String("disabled".to_string()))
        },
        "instrument_health": {
            "inst_id": selected_inst_id,
            "pipeline_stage": "collecting",
            "is_error": false,
            "is_stale": false,
            "last_inference_at": null
        },
        "timeline": [],
        "details": {
            "summary": summary,
            "last_inference_bucket": null,
            "runtime_error": ""
        },
        "emitted_at": now_unix_seconds()
    }))
}
