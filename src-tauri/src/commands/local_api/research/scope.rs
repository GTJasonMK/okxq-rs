use super::*;

/// POST /api/research/factors/compute — 计算指定币种的因子并写入 factor_scores。
pub(crate) async fn compute_research_factors(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let input = research_candle_input(state, req, 120, 30, 500, None).await?;
    let factors = crate::factor_engine::compute_all_factors(
        &state.db,
        &input.inst_id,
        &input.inst_type,
        &input.timeframe,
        input.bar_count,
    )
    .await
    .map_err(AppError::Validation)?;
    Ok(Value::Array(factors))
}

/// POST /api/research/dataset/build — 从 feature_bars_1s 构建训练数据集。
pub(crate) async fn build_research_dataset(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let dataset_id = body_string(req, "dataset_id", "");
    let input = research_candle_input(state, req, 3600, 120, 10000, Some(5000)).await?;
    let config = crate::dataset_builder::DatasetBuildConfig {
        inst_id: input.inst_id,
        inst_type: input.inst_type,
        timeframe: input.timeframe,
        bar_count: input.bar_count,
        dataset_id: if dataset_id.is_empty() {
            None
        } else {
            Some(dataset_id)
        },
        ..Default::default()
    };
    let summary = crate::dataset_builder::build_dataset(&state.db, &config)
        .await
        .map_err(AppError::Validation)?;
    Ok(summary)
}

struct ResearchCandleInput {
    inst_id: String,
    inst_type: String,
    timeframe: String,
    bar_count: i64,
}

async fn research_candle_input(
    state: &AppState,
    req: &LocalApiRequest,
    default_bar_count: i64,
    min_bar_count: i64,
    max_bar_count: i64,
    ensure_bar_count_cap: Option<i64>,
) -> AppResult<ResearchCandleInput> {
    let raw_inst_id = request_string(req, "inst_id", "");
    if raw_inst_id.is_empty() {
        return Err(AppError::Validation("inst_id 是必填字段".to_string()));
    }
    let requested_inst_type = request_string(req, "inst_type", "");
    let timeframe = request_string(req, "timeframe", "1H");
    let (inst_id, inst_type) =
        resolve_research_scope(state, &raw_inst_id, &requested_inst_type).await?;
    let bar_count =
        param_i64(req, "bar_count", default_bar_count).clamp(min_bar_count, max_bar_count);
    let ensure_bar_count = ensure_bar_count_cap
        .map(|cap| bar_count.min(cap))
        .unwrap_or(bar_count);
    super::super::market_ops::ensure_local_candles_for_read(
        state,
        &inst_id,
        &inst_type,
        &timeframe,
        ensure_bar_count,
        false,
    )
    .await?;
    Ok(ResearchCandleInput {
        inst_id,
        inst_type,
        timeframe,
        bar_count,
    })
}

pub(crate) async fn resolve_research_scope(
    state: &AppState,
    inst_id: &str,
    inst_type: &str,
) -> AppResult<(String, String)> {
    let raw_inst_id = inst_id.trim().to_uppercase();
    if raw_inst_id.is_empty() {
        return Err(AppError::Validation("inst_id 是必填字段".to_string()));
    }

    let requested_inst_type = inst_type.trim();
    if !requested_inst_type.is_empty() || raw_inst_id.ends_with("-SWAP") {
        let effective_inst_type = if requested_inst_type.is_empty() {
            infer_inst_type(&raw_inst_id)
        } else {
            requested_inst_type.to_string()
        };
        return resolve_watched_market_inst(state, &raw_inst_id, &effective_inst_type).await;
    }

    let normalized_symbol = normalize_symbol(&raw_inst_id)
        .ok_or_else(|| AppError::Validation("无效交易对".to_string()))?;
    let watched = state.preferences.watched_symbols().await?;
    let Some(record) = watched
        .into_iter()
        .find(|item| item.symbol.eq_ignore_ascii_case(&normalized_symbol))
    else {
        return Err(AppError::Validation(format!(
            "{normalized_symbol} 未在关注清单中启用，已拒绝研究数据读取"
        )));
    };

    let mut enabled = Vec::new();
    if record.sync_spot {
        enabled.push((
            record.spot_inst_id.trim().to_uppercase(),
            "SPOT".to_string(),
        ));
    }
    if record.sync_swap {
        enabled.push((
            record.swap_inst_id.trim().to_uppercase(),
            "SWAP".to_string(),
        ));
    }

    match enabled.len() {
        1 => Ok(enabled.remove(0)),
        0 => Err(AppError::Validation(format!(
            "{normalized_symbol} 未启用任何研究数据市场"
        ))),
        _ => Err(AppError::Validation(format!(
            "{normalized_symbol} 同时启用了现货和合约，请指定 inst_type"
        ))),
    }
}

/// GET /api/research/dataset/splits/:dataset_id — 查询数据集切分。
pub(crate) async fn research_dataset_splits(
    state: &AppState,
    dataset_id: &str,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let split = request_string(req, "split", "");
    let limit = param_i64(req, "limit", 200);
    let rows = crate::dataset_builder::get_dataset_splits(
        &state.db,
        dataset_id,
        if split.is_empty() { None } else { Some(&split) },
        limit,
    )
    .await
    .map_err(AppError::Validation)?;
    Ok(json!({
        "dataset_id": dataset_id,
        "split": if split.is_empty() { "all".to_string() } else { split },
        "row_count": rows.len(),
        "rows": rows,
    }))
}

/// POST /api/research/model/train — 训练线性回归模型。
pub(crate) async fn train_research_model(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let dataset_id = request_string(req, "dataset_id", "");
    if dataset_id.is_empty() {
        return Err(AppError::Validation("dataset_id 是必填字段".to_string()));
    }
    let training_seed = request_i64(req, "training_seed", 7);
    let result = crate::model_trainer::train_model(&state.db, &dataset_id, training_seed)
        .await
        .map_err(AppError::Validation)?;
    Ok(result)
}
