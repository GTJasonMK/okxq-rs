use std::time::Instant;

use serde_json::{json, Map, Value};
use tokio::task::JoinSet;

use crate::{
    error::{AppError, AppResult},
    okx::OkxPublicClient,
    strategy_executor::RuntimeOrderbookRequirement,
};

use super::{
    events::{elapsed_ms, emit_context_log, should_log_context_milestone},
    types::ContextTaskFailure,
};

const LIVE_CONTEXT_ORDERBOOK_FETCH_CONCURRENCY: usize = 2;

struct OrderbookContextLoad {
    index: usize,
    requirement: RuntimeOrderbookRequirement,
    book: Option<Value>,
    elapsed_ms: u64,
}

enum OrderbookContextTaskResult {
    Loaded(OrderbookContextLoad),
    Empty {
        index: usize,
        requirement: RuntimeOrderbookRequirement,
        elapsed_ms: u64,
    },
    Failed(ContextTaskFailure<RuntimeOrderbookRequirement>),
    OptionalFailed {
        index: usize,
        requirement: RuntimeOrderbookRequirement,
        error: AppError,
        elapsed_ms: u64,
    },
}

pub(super) async fn fetch_orderbook_context(
    client: &OkxPublicClient,
    requirements: Vec<RuntimeOrderbookRequirement>,
    on_context_event: &mut (dyn FnMut(&Value) + Send),
) -> AppResult<Value> {
    if requirements.is_empty() {
        return Ok(json!({}));
    }
    let total = requirements.len();
    let started = Instant::now();
    emit_context_log(
        on_context_event,
        "context_orderbook",
        "info",
        format!("开始读取盘口上下文，共 {total} 组"),
        json!({
            "requirement_count": total,
        }),
    );
    let concurrency_limit = LIVE_CONTEXT_ORDERBOOK_FETCH_CONCURRENCY.max(1);
    let mut normalized_requirements = Vec::with_capacity(total);
    for (index, requirement) in requirements.into_iter().enumerate() {
        normalized_requirements.push((
            index,
            crate::strategy_executor::normalize_orderbook_requirement(requirement)?,
        ));
    }
    let mut tasks = JoinSet::new();
    let mut next_index = 0;
    let mut loaded = 0usize;
    let mut results = (0..total)
        .map(|_| None)
        .collect::<Vec<Option<OrderbookContextLoad>>>();
    let mut first_error = None::<(usize, AppError)>;

    while next_index < normalized_requirements.len() || !tasks.is_empty() {
        while next_index < normalized_requirements.len() && tasks.len() < concurrency_limit {
            let (index, normalized) = normalized_requirements[next_index].clone();
            emit_context_log(
                on_context_event,
                "context_orderbook",
                "info",
                format!(
                    "读取盘口 {}/{}: {} depth {}",
                    index + 1,
                    total,
                    normalized.symbol,
                    normalized.depth
                ),
                json!({
                    "index": index + 1,
                    "total": total,
                    "symbol": normalized.symbol.clone(),
                    "inst_type": normalized.inst_type.clone(),
                    "depth": normalized.depth,
                    "required": normalized.required,
                    "concurrency_limit": concurrency_limit,
                }),
            );
            let client = client.clone();
            tasks.spawn(
                async move { load_orderbook_context_group(client, index, normalized).await },
            );
            next_index += 1;
        }

        let Some(joined) = tasks.join_next().await else {
            continue;
        };
        match joined {
            Ok(OrderbookContextTaskResult::Loaded(result)) => {
                loaded += 1;
                if should_log_context_milestone(result.index, total) || loaded == total {
                    emit_context_log(
                        on_context_event,
                        "context_orderbook",
                        "success",
                        format!(
                            "盘口完成 {}/{}: {}",
                            loaded, total, result.requirement.symbol
                        ),
                        json!({
                            "index": result.index + 1,
                            "completed": loaded,
                            "total": total,
                            "symbol": result.requirement.symbol.clone(),
                            "inst_type": result.requirement.inst_type.clone(),
                            "depth": result.requirement.depth,
                            "elapsed_ms": result.elapsed_ms,
                            "concurrency_limit": concurrency_limit,
                        }),
                    );
                }
                let index = result.index;
                results[index] = Some(result);
            }
            Ok(OrderbookContextTaskResult::Empty {
                index,
                requirement,
                elapsed_ms,
            }) => {
                emit_context_log(
                    on_context_event,
                    "context_orderbook",
                    "warn",
                    format!("{} 可选盘口上下文为空", requirement.symbol),
                    json!({
                        "index": index + 1,
                        "total": total,
                        "symbol": requirement.symbol.clone(),
                        "inst_type": requirement.inst_type.clone(),
                        "depth": requirement.depth,
                        "required": false,
                        "elapsed_ms": elapsed_ms,
                        "concurrency_limit": concurrency_limit,
                    }),
                );
            }
            Ok(OrderbookContextTaskResult::Failed(failure)) => {
                emit_context_log(
                    on_context_event,
                    "context_orderbook",
                    "error",
                    format!("获取 {} 盘口上下文失败", failure.requirement.symbol),
                    json!({
                        "index": failure.index + 1,
                        "total": total,
                        "symbol": failure.requirement.symbol.clone(),
                        "inst_type": failure.requirement.inst_type.clone(),
                        "depth": failure.requirement.depth,
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
            Ok(OrderbookContextTaskResult::OptionalFailed {
                index,
                requirement,
                error,
                elapsed_ms,
            }) => {
                emit_context_log(
                    on_context_event,
                    "context_orderbook",
                    "warn",
                    format!("可选盘口读取失败: {}", requirement.symbol),
                    json!({
                        "index": index + 1,
                        "total": total,
                        "symbol": requirement.symbol,
                        "inst_type": requirement.inst_type,
                        "depth": requirement.depth,
                        "required": false,
                        "elapsed_ms": elapsed_ms,
                        "error": error.to_string(),
                        "concurrency_limit": concurrency_limit,
                    }),
                );
                tracing::debug!(
                    symbol = requirement.symbol.as_str(),
                    error = %error,
                    "optional strategy orderbook context fetch failed"
                );
            }
            Err(error) => {
                let error = AppError::Runtime(format!("盘口上下文读取任务失败: {error}"));
                emit_context_log(
                    on_context_event,
                    "context_orderbook",
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

    let mut books = Map::new();
    for result in results.into_iter().flatten() {
        if let Some(book) = result.book {
            books.insert(result.requirement.symbol, book);
        }
    }
    emit_context_log(
        on_context_event,
        "context_orderbook",
        "success",
        "盘口上下文读取完成",
        json!({
            "requirement_count": total,
            "loaded_count": books.len(),
            "elapsed_ms": elapsed_ms(started),
        }),
    );
    Ok(Value::Object(books))
}

async fn load_orderbook_context_group(
    client: OkxPublicClient,
    index: usize,
    requirement: RuntimeOrderbookRequirement,
) -> OrderbookContextTaskResult {
    let started = Instant::now();
    match client
        .get_orderbook(&requirement.symbol, requirement.depth as u32)
        .await
    {
        Ok(book) if book.as_object().is_some_and(|item| !item.is_empty()) => {
            OrderbookContextTaskResult::Loaded(OrderbookContextLoad {
                index,
                requirement,
                book: Some(book),
                elapsed_ms: elapsed_ms(started),
            })
        }
        Ok(_) if requirement.required => {
            let error = AppError::Runtime(format!(
                "{} 订单簿上下文为空，策略声明 DATA_REQUIREMENTS.orderbook 后不能用空盘口执行",
                requirement.symbol
            ));
            OrderbookContextTaskResult::Failed(ContextTaskFailure {
                index,
                requirement,
                error,
                elapsed_ms: elapsed_ms(started),
                available_count: None,
            })
        }
        Ok(_) => OrderbookContextTaskResult::Empty {
            index,
            requirement,
            elapsed_ms: elapsed_ms(started),
        },
        Err(error) if requirement.required => {
            let error = AppError::Runtime(format!(
                "获取 {} 订单簿上下文失败: {error}",
                requirement.symbol
            ));
            OrderbookContextTaskResult::Failed(ContextTaskFailure {
                index,
                requirement,
                error,
                elapsed_ms: elapsed_ms(started),
                available_count: None,
            })
        }
        Err(error) => OrderbookContextTaskResult::OptionalFailed {
            index,
            requirement,
            error,
            elapsed_ms: elapsed_ms(started),
        },
    }
}
