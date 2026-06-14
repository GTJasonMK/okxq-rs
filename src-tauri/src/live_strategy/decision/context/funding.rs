use std::time::Instant;

use serde_json::{json, Map, Value};
use sqlx::SqlitePool;
use tokio::task::JoinSet;

use crate::{
    error::{AppError, AppResult},
    okx::OkxPublicClient,
    strategy_executor::RuntimeFundingRequirement,
};

use super::{
    events::{elapsed_ms, emit_context_log, should_log_context_milestone},
    types::ContextTaskFailure,
};

const LIVE_CONTEXT_FUNDING_FETCH_CONCURRENCY: usize = 2;

struct FundingContextLoad {
    index: usize,
    requirement: RuntimeFundingRequirement,
    value: Option<Value>,
    source: Option<&'static str>,
    history_count: usize,
    elapsed_ms: u64,
}

enum FundingContextTaskResult {
    Loaded(FundingContextLoad),
    Empty {
        index: usize,
        requirement: RuntimeFundingRequirement,
        elapsed_ms: u64,
    },
    Failed(ContextTaskFailure<RuntimeFundingRequirement>),
    OptionalFailed {
        index: usize,
        requirement: RuntimeFundingRequirement,
        error: AppError,
        elapsed_ms: u64,
    },
}

pub(super) async fn fetch_funding_context(
    db: &SqlitePool,
    client: &OkxPublicClient,
    requirements: Vec<RuntimeFundingRequirement>,
    timestamp: i64,
    on_context_event: &mut (dyn FnMut(&Value) + Send),
) -> AppResult<Value> {
    if requirements.is_empty() {
        return Ok(json!({}));
    }
    let total = requirements.len();
    let started = Instant::now();
    emit_context_log(
        on_context_event,
        "context_funding",
        "info",
        format!("开始读取资金费率上下文，共 {total} 组"),
        json!({
            "requirement_count": total,
            "timestamp": timestamp,
        }),
    );
    let funding_table_exists = crate::strategy_executor::local_funding_table_exists(db).await?;
    let concurrency_limit = LIVE_CONTEXT_FUNDING_FETCH_CONCURRENCY.max(1);
    let mut normalized_requirements = Vec::with_capacity(total);
    for (index, requirement) in requirements.into_iter().enumerate() {
        normalized_requirements.push((
            index,
            crate::strategy_executor::normalize_funding_requirement(requirement)?,
        ));
    }
    let mut tasks = JoinSet::new();
    let mut next_index = 0;
    let mut loaded = 0usize;
    let mut results = (0..total)
        .map(|_| None)
        .collect::<Vec<Option<FundingContextLoad>>>();
    let mut first_error = None::<(usize, AppError)>;

    while next_index < normalized_requirements.len() || !tasks.is_empty() {
        while next_index < normalized_requirements.len() && tasks.len() < concurrency_limit {
            let (index, normalized) = normalized_requirements[next_index].clone();
            emit_context_log(
                on_context_event,
                "context_funding",
                "info",
                format!(
                    "读取资金费率 {}/{}: {}",
                    index + 1,
                    total,
                    normalized.symbol
                ),
                json!({
                    "index": index + 1,
                    "total": total,
                    "symbol": normalized.symbol.clone(),
                    "inst_type": normalized.inst_type.clone(),
                    "history_limit": normalized.history_limit,
                    "required": normalized.required,
                    "local_table_exists": funding_table_exists,
                    "concurrency_limit": concurrency_limit,
                }),
            );
            let db = db.clone();
            let client = client.clone();
            tasks.spawn(async move {
                load_funding_context_group(
                    db,
                    client,
                    index,
                    normalized,
                    timestamp,
                    funding_table_exists,
                )
                .await
            });
            next_index += 1;
        }

        let Some(joined) = tasks.join_next().await else {
            continue;
        };
        match joined {
            Ok(FundingContextTaskResult::Loaded(result)) => {
                loaded += 1;
                if should_log_context_milestone(result.index, total) || loaded == total {
                    let source = result.source.unwrap_or("none");
                    emit_context_log(
                        on_context_event,
                        "context_funding",
                        "success",
                        format!(
                            "资金费率完成 {}/{}: {}",
                            loaded, total, result.requirement.symbol
                        ),
                        json!({
                            "index": result.index + 1,
                            "completed": loaded,
                            "total": total,
                            "symbol": result.requirement.symbol.clone(),
                            "inst_type": result.requirement.inst_type.clone(),
                            "source": source,
                            "history_count": result.history_count,
                            "elapsed_ms": result.elapsed_ms,
                            "concurrency_limit": concurrency_limit,
                        }),
                    );
                }
                let index = result.index;
                results[index] = Some(result);
            }
            Ok(FundingContextTaskResult::Empty {
                index,
                requirement,
                elapsed_ms,
            }) => {
                emit_context_log(
                    on_context_event,
                    "context_funding",
                    "warn",
                    format!("{} 可选资金费率上下文为空", requirement.symbol),
                    json!({
                        "index": index + 1,
                        "total": total,
                        "symbol": requirement.symbol,
                        "inst_type": requirement.inst_type,
                        "required": false,
                        "elapsed_ms": elapsed_ms,
                        "concurrency_limit": concurrency_limit,
                    }),
                );
            }
            Ok(FundingContextTaskResult::Failed(failure)) => {
                emit_context_log(
                    on_context_event,
                    "context_funding",
                    "error",
                    format!("获取 {} 资金费率上下文失败", failure.requirement.symbol),
                    json!({
                        "index": failure.index + 1,
                        "total": total,
                        "symbol": failure.requirement.symbol.clone(),
                        "inst_type": failure.requirement.inst_type.clone(),
                        "required": failure.requirement.required,
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
            Ok(FundingContextTaskResult::OptionalFailed {
                index,
                requirement,
                error,
                elapsed_ms,
            }) => {
                emit_context_log(
                    on_context_event,
                    "context_funding",
                    "warn",
                    format!("可选资金费率读取失败: {}", requirement.symbol),
                    json!({
                        "index": index + 1,
                        "total": total,
                        "symbol": requirement.symbol.clone(),
                        "inst_type": requirement.inst_type.clone(),
                        "required": false,
                        "elapsed_ms": elapsed_ms,
                        "error": error.to_string(),
                        "concurrency_limit": concurrency_limit,
                    }),
                );
                tracing::debug!(
                    symbol = requirement.symbol.as_str(),
                    error = %error,
                    "optional strategy funding context fetch failed"
                );
            }
            Err(error) => {
                let error = AppError::Runtime(format!("资金费率上下文读取任务失败: {error}"));
                emit_context_log(
                    on_context_event,
                    "context_funding",
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

    let mut funding = Map::new();
    for result in results.into_iter().flatten() {
        if let Some(value) = result.value {
            funding.insert(result.requirement.symbol, value);
        }
    }
    emit_context_log(
        on_context_event,
        "context_funding",
        "success",
        "资金费率上下文读取完成",
        json!({
            "requirement_count": total,
            "loaded_count": funding.len(),
            "elapsed_ms": elapsed_ms(started),
        }),
    );
    Ok(Value::Object(funding))
}

async fn load_funding_context_group(
    db: SqlitePool,
    client: OkxPublicClient,
    index: usize,
    requirement: RuntimeFundingRequirement,
    timestamp: i64,
    funding_table_exists: bool,
) -> FundingContextTaskResult {
    let started = Instant::now();
    let history = match crate::strategy_executor::load_local_funding_history_checked(
        &db,
        &requirement,
        timestamp,
        requirement.history_limit,
        funding_table_exists,
    )
    .await
    {
        Ok(history) => history,
        Err(error) => {
            return FundingContextTaskResult::Failed(ContextTaskFailure {
                index,
                requirement,
                error,
                elapsed_ms: elapsed_ms(started),
                available_count: None,
            });
        }
    };
    if !history.is_empty() {
        let history_count = history.len();
        return FundingContextTaskResult::Loaded(FundingContextLoad {
            index,
            value: Some(
                crate::strategy_executor::funding_context_value_from_history(
                    "okx_funding_rates",
                    history,
                ),
            ),
            requirement,
            source: Some("okx_funding_rates"),
            history_count,
            elapsed_ms: elapsed_ms(started),
        });
    }

    match client.get_funding_rate(&requirement.symbol).await {
        Ok(latest) if latest.as_object().is_some_and(|item| !item.is_empty()) => {
            FundingContextTaskResult::Loaded(FundingContextLoad {
                index,
                value: Some(
                    crate::strategy_executor::funding_context_value_from_history(
                        "okx_public_current",
                        vec![latest],
                    ),
                ),
                requirement,
                source: Some("okx_public_current"),
                history_count: 1,
                elapsed_ms: elapsed_ms(started),
            })
        }
        Ok(_) if requirement.required => {
            let error = AppError::Runtime(format!(
                "{} 资金费率上下文为空，策略声明 DATA_REQUIREMENTS.funding 后不能用空 funding 执行",
                requirement.symbol
            ));
            FundingContextTaskResult::Failed(ContextTaskFailure {
                index,
                requirement,
                error,
                elapsed_ms: elapsed_ms(started),
                available_count: None,
            })
        }
        Ok(_) => FundingContextTaskResult::Empty {
            index,
            requirement,
            elapsed_ms: elapsed_ms(started),
        },
        Err(error) if requirement.required => {
            let error = AppError::Runtime(format!(
                "获取 {} 资金费率上下文失败: {error}",
                requirement.symbol
            ));
            FundingContextTaskResult::Failed(ContextTaskFailure {
                index,
                requirement,
                error,
                elapsed_ms: elapsed_ms(started),
                available_count: None,
            })
        }
        Err(error) => FundingContextTaskResult::OptionalFailed {
            index,
            requirement,
            error,
            elapsed_ms: elapsed_ms(started),
        },
    }
}
