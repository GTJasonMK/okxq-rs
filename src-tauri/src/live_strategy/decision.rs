use std::{path::Path, time::Instant};

use sqlx::SqlitePool;

use serde_json::{json, Value};

use crate::{
    error::{AppError, AppResult},
    okx::{OkxCandle, OkxPrivateClient, OkxPublicClient},
    realtime::RealtimeManager,
    strategy_engine::StrategyConfig,
    strategy_executor::{
        types::{RuntimeStrategyDecision, RuntimeStrategyMeta},
        PythonRunnerSession,
    },
};

use super::{
    runtime_helpers::strategy_config_json_for_evaluate,
    types::{LiveStrategyConfig, LiveStrategyStatus},
};

pub(crate) use self::actions::{
    plan_runtime_actions_for_execution, StrategyCancelOrderIntent, StrategyExecutionIntent,
    StrategyExecutionPlan, StrategyIntentAction, StrategyModifyOrderIntent,
    StrategyOrderTargetKind, StrategyPlannedExitIntent, StrategyRiskOrderIntent,
};

use self::context::{build_live_runtime_context, candle_to_json, LiveRuntimeContextRequest};

mod actions;
mod candles;
mod context;

pub(in crate::live_strategy) use self::candles::fetch_strategy_candles;

pub(super) struct LatestStrategyPlanRequest<'a> {
    pub(super) db: &'a SqlitePool,
    pub(super) client: &'a OkxPublicClient,
    pub(super) project_root: &'a Path,
    pub(super) live_config: &'a LiveStrategyConfig,
    pub(super) runtime_status: &'a LiveStrategyStatus,
    pub(super) config: &'a StrategyConfig,
    pub(super) candles: &'a [OkxCandle],
    pub(super) private_client: Option<&'a OkxPrivateClient>,
    pub(super) realtime: Option<&'a RealtimeManager>,
    pub(super) strategy_session: Option<&'a mut PythonRunnerSession>,
}

pub(super) async fn latest_strategy_plan(
    request: LatestStrategyPlanRequest<'_>,
    on_strategy_event: &mut (dyn FnMut(&Value) + Send),
) -> AppResult<StrategyExecutionPlan> {
    let LatestStrategyPlanRequest {
        db,
        client,
        project_root,
        live_config,
        runtime_status,
        config,
        candles,
        private_client,
        realtime,
        strategy_session,
    } = request;
    if candles.len() < 3 {
        return Ok(StrategyExecutionPlan {
            intents: Vec::new(),
            risk_actions: Vec::new(),
            skipped_actions: Vec::new(),
            idle_actions: Vec::new(),
            execution_logs: Vec::new(),
            diagnostics: json!({}),
        });
    }
    let registry_started = Instant::now();
    let (runtime_meta, registered_at_runtime) =
        match runtime_meta_for_live_plan(project_root, &config.strategy_id) {
            Ok(value) => value,
            Err(error) => {
                emit_runtime_strategy_log(
                    on_strategy_event,
                    "strategy_registry",
                    "error",
                    format!("实时策略元数据不可用: {error}"),
                    json!({
                        "elapsed_ms": elapsed_ms(registry_started),
                        "strategy_id": config.strategy_id,
                    }),
                );
                return Err(error);
            }
        };
    if registered_at_runtime {
        emit_runtime_strategy_log(
            on_strategy_event,
            "strategy_registry",
            "success",
            "实时策略元数据已在运行期重新注册",
            json!({
                "elapsed_ms": elapsed_ms(registry_started),
                "strategy_id": &runtime_meta.strategy_id,
                "strategy_file": &runtime_meta.file_name,
            }),
        );
    };
    let config_json = strategy_config_json_for_evaluate(config);
    let candles_json = candles.iter().map(candle_to_json).collect::<Vec<_>>();
    let latest = candles
        .last()
        .expect("latest strategy plan requires at least three candles");
    emit_runtime_strategy_log(
        on_strategy_event,
        "context",
        "info",
        "开始构建策略运行上下文",
        json!({
            "strategy_id": config.strategy_id,
            "strategy_name": config.strategy_name,
            "symbol": config.symbol,
            "timeframe": config.timeframe,
            "primary_candle_count": candles.len(),
        }),
    );
    let context_started = Instant::now();
    let context = match build_live_runtime_context(
        LiveRuntimeContextRequest {
            db,
            client,
            data_requirements: &runtime_meta.data_requirements,
            live_config,
            config,
            config_json: &config_json,
            primary_candles: candles,
            primary_candles_json: &candles_json,
            status: runtime_status,
            private_client,
            realtime,
        },
        on_strategy_event,
    )
    .await
    {
        Ok(context) => {
            emit_runtime_strategy_log(
                on_strategy_event,
                "context",
                "success",
                "策略运行上下文构建完成",
                json!({
                    "elapsed_ms": elapsed_ms(context_started),
                    "context_time": context
                        .get("time")
                        .and_then(|time| time.get("timestamp"))
                        .and_then(Value::as_i64),
                }),
            );
            context
        }
        Err(error) => {
            emit_runtime_strategy_log(
                on_strategy_event,
                "context",
                "error",
                format!("策略运行上下文构建失败: {error}"),
                json!({
                    "elapsed_ms": elapsed_ms(context_started),
                }),
            );
            return Err(error);
        }
    };
    emit_runtime_strategy_log(
        on_strategy_event,
        "strategy_call",
        "info",
        "开始调用 Python 策略 evaluate",
        json!({
            "strategy_file": runtime_meta.file_name,
            "strategy_id": config.strategy_id,
            "primary_candle_count": candles_json.len(),
        }),
    );
    let strategy_call_started = Instant::now();
    let decision_result = if let Some(session) = strategy_session {
        session.compute_runtime_decision_with_context_and_events(
            &runtime_meta.file_name,
            &config_json,
            &candles_json,
            &context,
            |event| on_strategy_event(event),
        )
    } else {
        crate::strategy_executor::compute_runtime_decision_with_context_and_events(
            project_root,
            &runtime_meta.file_name,
            &config_json,
            &candles_json,
            &context,
            |event| on_strategy_event(event),
        )
    };
    let decision = match decision_result {
        Ok(decision) => {
            emit_runtime_strategy_log(
                on_strategy_event,
                "strategy_call",
                "success",
                "Python 策略决策返回",
                json!({
                    "elapsed_ms": elapsed_ms(strategy_call_started),
                    "action_count": decision.actions.len(),
                    "strategy_log_count": decision.execution_logs.len(),
                }),
            );
            decision
        }
        Err(error) => {
            emit_runtime_strategy_log(
                on_strategy_event,
                "strategy_call",
                "error",
                format!("Python 策略决策失败: {error}"),
                json!({
                    "elapsed_ms": elapsed_ms(strategy_call_started),
                }),
            );
            return Err(AppError::Runtime(format!("执行运行策略决策失败: {error}")));
        }
    };

    Ok(strategy_execution_plan_from_decision(
        decision, latest, config,
    ))
}

fn runtime_meta_for_live_plan(
    project_root: &Path,
    strategy_id: &str,
) -> AppResult<(RuntimeStrategyMeta, bool)> {
    let strategy_id = strategy_id.trim();
    if strategy_id.is_empty() {
        return Err(AppError::Validation("实时策略 id 不能为空".to_string()));
    }
    if let Some(meta) = crate::strategy_executor::get_meta(strategy_id) {
        return Ok((meta, false));
    }
    crate::strategy_executor::ensure_registered(project_root, strategy_id)
        .map(|meta| (meta, true))
        .map_err(|error| {
            AppError::Runtime(format!(
                "实时策略运行期注册策略失败，不能继续执行决策: {error}"
            ))
        })
}

fn strategy_execution_plan_from_decision(
    decision: RuntimeStrategyDecision,
    latest: &OkxCandle,
    config: &StrategyConfig,
) -> StrategyExecutionPlan {
    let (intents, risk_actions, skipped_actions, idle_actions) =
        plan_runtime_actions_for_execution(&decision.actions, latest, config);
    StrategyExecutionPlan {
        intents,
        risk_actions,
        skipped_actions,
        idle_actions,
        execution_logs: decision.execution_logs,
        diagnostics: decision.diagnostics,
    }
}

fn emit_runtime_strategy_log(
    on_strategy_event: &mut (dyn FnMut(&Value) + Send),
    stage: &str,
    level: &str,
    message: impl Into<String>,
    details: Value,
) {
    let event = json!({
        "event": "strategy_log",
        "stage": stage,
        "level": level,
        "message": message.into(),
        "details": {
            "source": "rust_runtime",
            "data": details,
        },
    });
    on_strategy_event(&event);
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy_executor::{register, types::RuntimeStrategyAction};

    #[test]
    fn runtime_meta_for_live_plan_reuses_registered_strategy_metadata() {
        let strategy_id = "live_decision_registered_meta_fixture";
        register(RuntimeStrategyMeta {
            strategy_id: strategy_id.to_string(),
            strategy_name: "Live Decision Registered Meta Fixture".to_string(),
            description: String::new(),
            strategy_type: "single_symbol_strategy".to_string(),
            data_requirements: json!({}),
            runtime_config: json!({}),
            visualization: json!({}),
            decision_contract: json!({}),
            file_name: "runtime/live_decision_registered_meta_fixture.py".to_string(),
        });

        let (meta, registered_at_runtime) =
            runtime_meta_for_live_plan(Path::new("."), strategy_id).unwrap();

        assert_eq!(meta.strategy_id, strategy_id);
        assert!(
            !registered_at_runtime,
            "already registered strategies should not force runtime rediscovery"
        );
    }

    #[test]
    fn runtime_meta_for_live_plan_fails_when_strategy_cannot_be_registered() {
        let error = runtime_meta_for_live_plan(
            Path::new("/tmp/okxq_missing_runtime_strategy_root"),
            "missing_live_decision_strategy_fixture",
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("实时策略运行期注册策略失败，不能继续执行决策"),
            "missing runtime metadata must be a visible decision error, got: {error}"
        );
    }

    #[test]
    fn execution_plan_blocks_all_intents_when_any_action_is_skipped() {
        let decision = RuntimeStrategyDecision {
            actions: vec![
                RuntimeStrategyAction::from_value(&json!({
                    "action": "open_position",
                    "symbol": "BTC-USDT-SWAP",
                    "side": "long",
                    "price": 100.0,
                    "position_size": 0.2,
                    "timestamp": 1_700_000_000_000_i64,
                    "reason": "valid_entry"
                })),
                RuntimeStrategyAction::from_value(&json!({
                    "action": "modify_order",
                    "symbol": "BTC-USDT-SWAP",
                    "side": "hold",
                    "order_id": "order-1",
                    "new_px": "101.2",
                    "timestamp": 1_700_000_000_000_i64,
                    "reason": "legacy_alias_should_block_batch"
                })),
            ],
            execution_logs: Vec::new(),
            indicators: json!({}),
            diagnostics: json!({}),
        };

        let plan =
            strategy_execution_plan_from_decision(decision, &test_candle(100.0), &test_config());

        assert!(
            plan.intents.is_empty(),
            "live runtime must not partially execute a decision batch with invalid actions"
        );
        assert!(
            plan.risk_actions.is_empty(),
            "live runtime must not expose executable risk actions for an invalid decision batch"
        );
        assert_eq!(plan.skipped_actions.len(), 1);
        assert!(plan.skipped_actions[0]["_execution_skip_reason"]
            .as_str()
            .unwrap_or_default()
            .contains("已删除字段别名 new_px"));
    }

    #[test]
    fn execution_plan_blocks_standalone_risk_intents_when_any_action_is_skipped() {
        let decision = RuntimeStrategyDecision {
            actions: vec![
                RuntimeStrategyAction::from_value(&json!({
                    "action": "place_risk_order",
                    "symbol": "BTC-USDT-SWAP",
                    "side": "sell",
                    "order_type": "stop_market",
                    "trigger_price": 95.0,
                    "stop_loss_bps": 500,
                    "timestamp": 1_700_000_000_000_i64,
                    "reason": "valid_standalone_risk"
                })),
                RuntimeStrategyAction::from_value(&json!({
                    "action": "modify_order",
                    "symbol": "BTC-USDT-SWAP",
                    "side": "hold",
                    "order_id": "order-1",
                    "new_px": "101.2",
                    "timestamp": 1_700_000_000_000_i64,
                    "reason": "legacy_alias_should_block_risk_batch"
                })),
            ],
            execution_logs: Vec::new(),
            indicators: json!({}),
            diagnostics: json!({}),
        };

        let plan =
            strategy_execution_plan_from_decision(decision, &test_candle(100.0), &test_config());

        assert!(
            plan.intents.is_empty(),
            "standalone risk orders are executable intents and must be blocked with the batch"
        );
        assert!(plan.risk_actions.is_empty());
        assert_eq!(plan.skipped_actions.len(), 1);
    }

    #[test]
    fn execution_plan_keeps_intents_when_all_actions_are_valid() {
        let decision = RuntimeStrategyDecision {
            actions: vec![RuntimeStrategyAction::from_value(&json!({
                "action": "open_position",
                "symbol": "BTC-USDT-SWAP",
                "side": "long",
                "price": 100.0,
                "position_size": 0.2,
                "timestamp": 1_700_000_000_000_i64,
                "reason": "valid_entry"
            }))],
            execution_logs: Vec::new(),
            indicators: json!({}),
            diagnostics: json!({}),
        };

        let plan =
            strategy_execution_plan_from_decision(decision, &test_candle(100.0), &test_config());

        assert_eq!(plan.intents.len(), 1);
        assert!(plan.skipped_actions.is_empty());
    }

    fn test_config() -> StrategyConfig {
        StrategyConfig {
            strategy_id: "runtime_action_test".to_string(),
            strategy_name: "Runtime Action Test".to_string(),
            symbol: "BTC-USDT-SWAP".to_string(),
            inst_type: "SWAP".to_string(),
            timeframe: "15m".to_string(),
            initial_capital: 1_000.0,
            position_size: 0.2,
            stop_loss: 0.0,
            take_profit: 0.0,
            params: json!({}),
        }
    }

    fn test_candle(close: f64) -> OkxCandle {
        OkxCandle {
            timestamp: 1_700_000_000_000,
            open: close,
            high: close,
            low: close,
            close,
            volume: 1.0,
            volume_ccy: 1.0,
            volume_quote: 1.0,
            confirm: "1".to_string(),
        }
    }
}
