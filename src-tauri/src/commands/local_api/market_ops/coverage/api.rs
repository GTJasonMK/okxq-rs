use crate::sync_jobs::SyncJobRequest;

use super::super::*;

pub(in crate::commands::local_api) fn sync_request_from_api(
    req: &LocalApiRequest,
) -> AppResult<SyncJobRequest> {
    let raw_inst_id = request_string(req, "inst_id", "").trim().to_uppercase();
    if raw_inst_id.is_empty() {
        return Err(AppError::Validation("inst_id 不能为空".to_string()));
    }

    let inferred_inst_type = infer_inst_type(&raw_inst_id);
    let inst_type = request_string(req, "inst_type", &inferred_inst_type)
        .trim()
        .to_uppercase();
    if !matches!(inst_type.as_str(), "SPOT" | "SWAP") {
        return Err(AppError::Validation(format!(
            "当前仅支持 SPOT/SWAP，收到 inst_type={inst_type}"
        )));
    }
    let inst_id = normalize_market_inst_id(&raw_inst_id, &inst_type);
    let timeframe = normalize_timeframe_name(&request_string(req, "timeframe", "1H"));
    if timeframe.is_empty() {
        return Err(AppError::Validation(
            "不支持的 K 线周期，当前仅支持 1m/3m/5m/15m/30m/1H/2H/4H/6H/12H/1D/1W/1M".to_string(),
        ));
    }
    let days = request_i64(req, "days", 30).clamp(1, 3650);
    let mode = normalize_sync_mode(&request_string(req, "mode", "window"));
    let mut target_timeframes = request_string_array(req, "target_timeframes");
    if target_timeframes.is_empty() {
        target_timeframes.push(timeframe.clone());
    }
    sync_request(inst_id, inst_type, timeframe, mode, days, target_timeframes)
}

pub(in crate::commands::local_api) async fn ensure_sync_request_allowed(
    state: &AppState,
    request: &SyncJobRequest,
) -> AppResult<()> {
    let scopes = watched_instrument_scopes(state).await?;
    if !watched_request_allowed(&scopes, &request.inst_id, &request.inst_type) {
        return Err(AppError::Validation(format!(
            "{} {} 未在关注清单中启用，已拒绝同步任务",
            request.inst_id, request.inst_type
        )));
    }
    let targets = normalize_target_timeframes(request.target_timeframes.clone());
    ensure_timeframes_allowed_for_scope(state, &request.inst_id, &request.inst_type, &targets).await
}
