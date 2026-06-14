use std::{sync::Arc, time::Instant};

use serde_json::{json, Value};
use sqlx::SqlitePool;
use tokio::task::JoinSet;

use crate::{
    error::{AppError, AppResult},
    okx::OkxPublicClient,
    strategy_engine::StrategyConfig,
    strategy_executor::RuntimeCandleRequirement,
};

use super::{
    super::candles::fetch_candles_by_timeframe,
    candle_to_json,
    events::{elapsed_ms, emit_context_log, is_primary_requirement, should_log_context_milestone},
    types::ContextTaskFailure,
};

const LIVE_CONTEXT_CANDLE_FETCH_CONCURRENCY: usize = 2;

struct CandleContextLoad {
    index: usize,
    requirement: RuntimeCandleRequirement,
    candles_json: Vec<Value>,
    source: &'static str,
    elapsed_ms: u64,
}

enum CandleContextTaskResult {
    Loaded(CandleContextLoad),
    Failed(ContextTaskFailure<RuntimeCandleRequirement>),
}

pub(super) struct CandleContextSetRequest<'a> {
    pub(super) db: &'a SqlitePool,
    pub(super) client: &'a OkxPublicClient,
    pub(super) config: &'a StrategyConfig,
    pub(super) requirements: Vec<RuntimeCandleRequirement>,
    pub(super) primary_candles_json: &'a [Value],
    pub(super) unique_symbol_count: usize,
    pub(super) unique_timeframe_count: usize,
}

pub(super) async fn fetch_candle_context_sets(
    request: CandleContextSetRequest<'_>,
    on_context_event: &mut (dyn FnMut(&Value) + Send),
) -> AppResult<Vec<(RuntimeCandleRequirement, Value)>> {
    let total = request.requirements.len();
    let mut normalized_requirements = Vec::with_capacity(total);
    for (index, requirement) in request.requirements.into_iter().enumerate() {
        normalized_requirements.push((
            index,
            crate::strategy_executor::normalize_candle_requirement(requirement)?,
        ));
    }

    let primary_candles_json = Arc::new(request.primary_candles_json.to_vec());
    let concurrency_limit = LIVE_CONTEXT_CANDLE_FETCH_CONCURRENCY.max(1);
    let mut tasks = JoinSet::new();
    let mut next_index = 0;
    let mut loaded = 0usize;
    let mut results = (0..total)
        .map(|_| None)
        .collect::<Vec<Option<CandleContextLoad>>>();
    let mut first_error = None::<(usize, AppError)>;

    while next_index < normalized_requirements.len() || !tasks.is_empty() {
        while next_index < normalized_requirements.len() && tasks.len() < concurrency_limit {
            let (index, normalized) = normalized_requirements[next_index].clone();
            emit_context_log(
                on_context_event,
                "context_candles",
                "info",
                format!(
                    "读取上下文 K 线组 {}/{}: {} {} 至少 {} 根",
                    index + 1,
                    total,
                    normalized.symbol,
                    normalized.timeframe,
                    normalized.min_bars
                ),
                json!({
                    "index": index + 1,
                    "total": total,
                    "total_groups": total,
                    "unique_symbol_count": request.unique_symbol_count,
                    "unique_timeframe_count": request.unique_timeframe_count,
                    "symbol": normalized.symbol.clone(),
                    "inst_type": normalized.inst_type.clone(),
                    "timeframe": normalized.timeframe.clone(),
                    "min_bars": normalized.min_bars,
                    "role": normalized.role.clone(),
                    "concurrency_limit": concurrency_limit,
                }),
            );
            let db = request.db.clone();
            let client = request.client.clone();
            let primary_candles_json = Arc::clone(&primary_candles_json);
            let is_primary = is_primary_requirement(&normalized, request.config);
            tasks.spawn(async move {
                load_candle_context_group(
                    db,
                    client,
                    index,
                    normalized,
                    primary_candles_json,
                    is_primary,
                )
                .await
            });
            next_index += 1;
        }

        let Some(joined) = tasks.join_next().await else {
            continue;
        };
        match joined {
            Ok(CandleContextTaskResult::Loaded(result)) => {
                loaded += 1;
                if should_log_context_milestone(result.index, total) || loaded == total {
                    emit_context_log(
                        on_context_event,
                        "context_candles",
                        "success",
                        format!(
                            "上下文 K 线组完成 {}/{}: {} {} 已读取 {} 根",
                            loaded,
                            total,
                            result.requirement.symbol,
                            result.requirement.timeframe,
                            result.candles_json.len()
                        ),
                        json!({
                            "index": result.index + 1,
                            "completed": loaded,
                            "total": total,
                            "total_groups": total,
                            "symbol": result.requirement.symbol.clone(),
                            "inst_type": result.requirement.inst_type.clone(),
                            "timeframe": result.requirement.timeframe.clone(),
                            "bar_count": result.candles_json.len(),
                            "min_bars": result.requirement.min_bars,
                            "source": result.source,
                            "elapsed_ms": result.elapsed_ms,
                            "concurrency_limit": concurrency_limit,
                        }),
                    );
                }
                let index = result.index;
                results[index] = Some(result);
            }
            Ok(CandleContextTaskResult::Failed(failure)) => {
                emit_context_log(
                    on_context_event,
                    "context_candles",
                    "error",
                    format!(
                        "读取上下文 K 线失败: {} {} {}",
                        failure.requirement.symbol,
                        failure.requirement.inst_type,
                        failure.requirement.timeframe
                    ),
                    json!({
                        "index": failure.index + 1,
                        "total": total,
                        "total_groups": total,
                        "symbol": failure.requirement.symbol.clone(),
                        "inst_type": failure.requirement.inst_type.clone(),
                        "timeframe": failure.requirement.timeframe.clone(),
                        "available_bars": failure.available_count,
                        "min_bars": failure.requirement.min_bars,
                        "elapsed_ms": failure.elapsed_ms,
                        "error": failure.error.to_string(),
                        "concurrency_limit": concurrency_limit,
                    }),
                );
                let should_replace_error = match first_error.as_ref() {
                    Some((index, _)) => failure.index < *index,
                    None => true,
                };
                if should_replace_error {
                    first_error = Some((failure.index, failure.error));
                }
            }
            Err(error) => {
                let error = AppError::Runtime(format!("上下文 K 线读取任务失败: {error}"));
                emit_context_log(
                    on_context_event,
                    "context_candles",
                    "error",
                    error.to_string(),
                    json!({ "concurrency_limit": concurrency_limit }),
                );
                if first_error.is_none() {
                    first_error = Some((usize::MAX, error));
                }
            }
        }
    }

    if let Some((_, error)) = first_error {
        return Err(error);
    }

    results
        .into_iter()
        .enumerate()
        .map(|(index, result)| {
            let result = result.ok_or_else(|| {
                AppError::Runtime(format!("上下文 K 线读取缺少第 {} 组结果", index + 1))
            })?;
            Ok((result.requirement, Value::Array(result.candles_json)))
        })
        .collect()
}

async fn load_candle_context_group(
    db: SqlitePool,
    client: OkxPublicClient,
    index: usize,
    requirement: RuntimeCandleRequirement,
    primary_candles_json: Arc<Vec<Value>>,
    is_primary: bool,
) -> CandleContextTaskResult {
    let started = Instant::now();
    let mut source = "runtime_fetch";
    let candles_json = if is_primary && primary_candles_json.len() >= requirement.min_bars {
        source = "primary_candles";
        primary_candles_json.as_ref().clone()
    } else {
        match fetch_candles_by_timeframe(
            &db,
            &client,
            &requirement.symbol,
            &requirement.inst_type,
            &requirement.timeframe,
            requirement.min_bars,
        )
        .await
        {
            Ok(candles) => candles.iter().map(candle_to_json).collect::<Vec<_>>(),
            Err(error) => {
                return CandleContextTaskResult::Failed(ContextTaskFailure {
                    index,
                    requirement,
                    error,
                    elapsed_ms: elapsed_ms(started),
                    available_count: None,
                });
            }
        }
    };
    if candles_json.len() < requirement.min_bars {
        let error = AppError::Validation(format!(
            "{} {} {} 可用 K 线数量 {} 小于策略 DATA_REQUIREMENTS.min_bars {}",
            requirement.symbol,
            requirement.inst_type,
            requirement.timeframe,
            candles_json.len(),
            requirement.min_bars
        ));
        return CandleContextTaskResult::Failed(ContextTaskFailure {
            index,
            requirement,
            error,
            elapsed_ms: elapsed_ms(started),
            available_count: Some(candles_json.len()),
        });
    }
    CandleContextTaskResult::Loaded(CandleContextLoad {
        index,
        requirement,
        candles_json,
        source,
        elapsed_ms: elapsed_ms(started),
    })
}
