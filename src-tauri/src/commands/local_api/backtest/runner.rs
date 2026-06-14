use std::time::Instant;

use serde_json::{json, Value};

use crate::{
    app_state::AppState,
    backtest_progress::BacktestProgressReporter,
    error::{AppError, AppResult},
    live_strategy::decision::plan_runtime_actions_for_execution,
    okx::OkxCandle,
    risk_controls,
    strategy_engine::{BacktestReport, HistoricalLiveBacktest, StrategyConfig},
    strategy_executor,
};

use super::window::BacktestWindow;

mod context_data;
mod diagnostics;
mod instrument_rules;
mod runtime_progress;

#[cfg(test)]
pub(super) use context_data::{
    backtest_context_prefix_len_at_or_before, backtest_context_required_end_ts,
    backtest_context_required_start_ts, context_candles_cover_window_warmup,
    default_backtest_primary_min_bars,
};
use context_data::{
    context_with_backtest_progress, dynamic_backtest_context, no_evaluable_window_message,
    reject_required_realtime_feeds, BacktestContextData, BacktestFundingContextData,
};
#[cfg(test)]
pub(super) use diagnostics::{compact_backtest_diagnostics_enabled, compact_strategy_diagnostics};
use instrument_rules::{backtest_config_with_instrument_rules, resolved_instrument_rules_source};
#[cfg(test)]
pub(super) use instrument_rules::{
    backtest_instrument_rules_source, backtest_required_instruments,
    backtest_strict_instrument_rules, config_with_okx_instrument_rules_from_instruments,
    config_with_resolved_instrument_rules_source, config_with_simulated_instrument_rules,
    config_with_simulated_instrument_rules_for, BacktestInstrumentRulesSource,
};
use runtime_progress::{
    elapsed_ms, should_report_backtest_step, strategy_loop_percent, BacktestRuntimeProfile,
};

pub(super) async fn run_runtime_backtest(
    state: &AppState,
    runtime_meta: &strategy_executor::types::RuntimeStrategyMeta,
    config: &StrategyConfig,
    candles: &[OkxCandle],
    window: BacktestWindow,
    progress: &BacktestProgressReporter,
) -> AppResult<BacktestReport> {
    progress.report_detail(
        23,
        "instrument_rules",
        "解析交易规格来源",
        0,
        candles.len(),
        json!({
            "step": "instrument_rules_prepare",
            "symbol": config.symbol,
            "inst_type": config.inst_type,
        }),
    );
    let config = backtest_config_with_instrument_rules(state, runtime_meta, config).await?;
    let instrument_rules_source = resolved_instrument_rules_source(&config);
    progress.report_detail(
        24,
        "instrument_rules",
        &format!("交易规格来源: {instrument_rules_source}"),
        0,
        candles.len(),
        json!({
            "step": "instrument_rules_ready",
            "instrument_rules_source": instrument_rules_source,
            "symbol": config.symbol,
            "inst_type": config.inst_type,
            "ctVal": config.params.get("ctVal").cloned().unwrap_or(Value::Null),
            "ctValCcy": config.params.get("ctValCcy").cloned().unwrap_or(Value::Null),
            "lotSz": config.params.get("lotSz").cloned().unwrap_or(Value::Null),
            "minSz": config.params.get("minSz").cloned().unwrap_or(Value::Null),
            "tickSz": config.params.get("tickSz").cloned().unwrap_or(Value::Null),
        }),
    );
    reject_required_realtime_feeds(runtime_meta, &config)?;
    progress.report_detail(
        25,
        "context",
        "加载历史上下文数据",
        0,
        candles.len(),
        json!({
            "step": "context_load_start",
            "primary_candles": candles.len(),
            "window_days": window.days,
        }),
    );
    let context_data =
        BacktestContextData::load(state, runtime_meta, &config, window, progress).await?;
    if let Some(blocked) = context_data.no_evaluable_window_detail(candles, window) {
        progress.report_detail(
            34,
            "warmup_blocked",
            "回测窗口没有满足 warmup 的历史决策点",
            0,
            candles.len(),
            blocked.clone(),
        );
        return Err(AppError::Validation(no_evaluable_window_message(&blocked)));
    }
    let funding_context_data = BacktestFundingContextData::load(
        state,
        &runtime_meta.data_requirements,
        &config,
        window,
        context_data.execution_funding_requirements(),
    )
    .await?;
    progress.report_detail(
        32,
        "context",
        "历史上下文数据加载完成",
        0,
        candles.len(),
        json!({
            "step": "context_load_ready",
            "context_series_count": context_data.series_count(),
            "context_requirements": context_data.requirement_summary(),
            "funding_series_count": funding_context_data.series_count(),
            "funding_rows": funding_context_data.total_rows(),
            "execution_funding_series_count": funding_context_data.execution_series_count(),
            "execution_funding_rows": funding_context_data.execution_total_rows(),
        }),
    );
    let market = context_data
        .market_data()
        .with_funding(funding_context_data.execution_funding_series());
    let mut backtest = HistoricalLiveBacktest::try_new(&config, candles, window.days)?;
    let config_json = runtime_config_json(&config);
    let runtime_execution_stamp =
        strategy_executor::runtime_execution_stamp(&state.paths.root, &runtime_meta.file_name);
    let mut python_runner = strategy_executor::PythonRunnerSession::new(&state.paths.root)?;
    let context_cache_id = format!(
        "backtest:{}:{}:{}:{}",
        config.strategy_id, config.symbol, window.start_ts, window.end_ts
    );
    let cache_started = Instant::now();
    let cache_result = python_runner.cache_runtime_context(
        &context_cache_id,
        &context_data.static_context(&config_json, &funding_context_data, window, candles),
    )?;
    let cache_ms = cache_started.elapsed().as_millis() as u64;
    progress.report_detail(
        33,
        "context_cache",
        "Python 策略上下文缓存完成",
        0,
        candles.len(),
        json!({
            "step": "context_cache_ready",
            "context_cache_id": context_cache_id,
            "cache_ms": cache_ms,
            "python_cache": cache_result,
        }),
    );

    progress.report_detail(
        35,
        "strategy",
        "按历史时间执行策略 evaluate",
        0,
        candles.len(),
        json!({
            "step": "strategy_loop_start",
            "total_steps": candles.len(),
            "instrument_rules_source": instrument_rules_source,
        }),
    );
    let total = candles.len();
    let mut evaluated_steps = 0usize;
    let mut warmup_skipped = 0usize;
    let mut context_skipped = 0usize;
    let mut submitted_intents_total = 0usize;
    let mut skipped_actions_total = 0usize;
    let mut risk_actions_total = 0usize;
    let mut profile = BacktestRuntimeProfile::default();
    let compact_diagnostics = diagnostics::compact_backtest_diagnostics_enabled(&config);
    let state_requirements =
        strategy_executor::state_context_requirements(&runtime_meta.data_requirements);
    let mut strategy_logs_total = 0usize;
    let mut stored_strategy_logs_total = 0usize;
    let execution_delay_ms = backtest_execution_delay_ms(&config);
    for (index, candle) in candles.iter().enumerate() {
        let step_started = Instant::now();
        let timestamp = candle.timestamp;
        let market_started = Instant::now();
        backtest.process_market_until(timestamp, &market);
        profile.market_ms += elapsed_ms(market_started);
        let context_started = Instant::now();
        let Some(primary_context) = context_data.primary_context_until(&config, timestamp)? else {
            profile.context_ms += elapsed_ms(context_started);
            warmup_skipped += 1;
            if should_report_backtest_step(index, total) {
                progress.report_detail(
                    strategy_loop_percent(index + 1, total),
                    "warmup",
                    &format!("等待 warmup {}/{} 根K线", index + 1, total),
                    index + 1,
                    total,
                    json!({
                        "step": "warmup_wait",
                        "warmup_skipped": warmup_skipped,
                        "evaluated_steps": evaluated_steps,
                        "context_skipped": context_skipped,
                        "current_timestamp": timestamp,
                    }),
                );
            }
            backtest.record_equity(candle, &market);
            continue;
        };
        if !context_data.context_ready_at(timestamp) {
            profile.context_ms += elapsed_ms(context_started);
            context_skipped += 1;
            if should_report_backtest_step(index, total) {
                progress.report_detail(
                    strategy_loop_percent(index + 1, total),
                    "context_wait",
                    &format!("等待上下文 {}/{} 根K线", index + 1, total),
                    index + 1,
                    total,
                    json!({
                        "step": "context_wait",
                        "warmup_skipped": warmup_skipped,
                        "evaluated_steps": evaluated_steps,
                        "context_skipped": context_skipped,
                        "primary_context_candles": primary_context.len,
                        "current_timestamp": timestamp,
                    }),
                );
            }
            backtest.record_equity(candle, &market);
            continue;
        }
        funding_context_data.ensure_available_at(timestamp)?;
        let context = dynamic_backtest_context(
            &config_json,
            state_requirements,
            timestamp,
            &backtest,
            &market,
        );
        let progress_context = context_with_backtest_progress(&context, index + 1, total);
        profile.context_ms += elapsed_ms(context_started);
        if should_report_backtest_step(index, total) {
            progress.report_detail(
                strategy_loop_percent(index, total),
                "strategy_evaluate",
                &format!("调用策略 evaluate {}/{} 根K线", index + 1, total),
                index,
                total,
                json!({
                    "step": "strategy_evaluate_start",
                    "evaluated_steps": evaluated_steps,
                    "warmup_skipped": warmup_skipped,
                    "context_skipped": context_skipped,
                    "primary_context_candles": primary_context.len,
                    "current_timestamp": timestamp,
                }),
            );
        }
        let python_started = Instant::now();
        let current_candle_json = vec![candle.to_json()];
        let decision = python_runner.compute_runtime_decision_with_context_ref_and_events(
            &runtime_meta.file_name,
            &config_json,
            &current_candle_json,
            &context_cache_id,
            &progress_context,
            |event| progress.report_python_progress(event),
        )?;
        profile.python_ms += elapsed_ms(python_started);
        evaluated_steps += 1;
        let execution_started = Instant::now();
        let (intents, risk_actions, skipped_actions, _idle_actions) =
            plan_runtime_actions_for_execution(&decision.actions, candle, &config);
        let submit_timestamp = backtest_execution_submit_timestamp(timestamp, &config);
        let risk_actions_count = risk_actions.len();
        submitted_intents_total += intents.len();
        skipped_actions_total += skipped_actions.len();
        risk_actions_total += risk_actions_count;
        let diagnostics_for_storage = if compact_diagnostics {
            diagnostics::compact_strategy_diagnostics(&decision.diagnostics)
        } else {
            decision.diagnostics.clone()
        };
        progress.report_strategy_step_with_detail(
            index + 1,
            total,
            &diagnostics_for_storage,
            json!({
                "step": "strategy_evaluate_done",
                "evaluated_steps": evaluated_steps,
                "warmup_skipped": warmup_skipped,
                "context_skipped": context_skipped,
                "actions": decision.actions.len(),
                "intents": intents.len(),
                "risk_actions": risk_actions_count,
                "skipped_actions": skipped_actions.len(),
                "submitted_intents_total": submitted_intents_total,
                "risk_actions_total": risk_actions_total,
                "skipped_actions_total": skipped_actions_total,
                "historical_execution_delay_ms": execution_delay_ms,
                "submit_timestamp": submit_timestamp,
                "compact_backtest_diagnostics": compact_diagnostics,
                "primary_context_candles": primary_context.len,
                "current_timestamp": timestamp,
            }),
        );
        let actions_json = decision
            .actions
            .iter()
            .map(|action| action.to_value())
            .collect::<Vec<_>>();
        let execution_logs = decision
            .execution_logs
            .iter()
            .filter_map(|entry| serde_json::to_value(entry).ok())
            .collect::<Vec<_>>();
        strategy_logs_total += execution_logs.len();
        let stored_execution_logs = if compact_diagnostics {
            Vec::new()
        } else {
            execution_logs
        };
        stored_strategy_logs_total += stored_execution_logs.len();
        backtest.record_strategy_step(
            &actions_json,
            &skipped_actions,
            &stored_execution_logs,
            diagnostics_for_storage,
            decision.indicators,
        );
        backtest.submit_intents(&intents, submit_timestamp, &market);
        backtest.record_equity(candle, &market);
        profile.execution_ms += elapsed_ms(execution_started);
        profile.total_ms += elapsed_ms(step_started);
    }

    if evaluated_steps == 0 {
        return Err(AppError::Validation(
            "回测窗口内没有任何满足 DATA_REQUIREMENTS warmup 的历史决策点".to_string(),
        ));
    }
    progress.report_detail(
        90,
        "simulate",
        "历史订单执行完成，生成报告",
        total,
        total,
        json!({
            "step": "simulation_done",
            "evaluated_steps": evaluated_steps,
            "warmup_skipped": warmup_skipped,
            "context_skipped": context_skipped,
            "submitted_intents_total": submitted_intents_total,
            "risk_actions_total": risk_actions_total,
            "skipped_actions_total": skipped_actions_total,
            "strategy_logs_total": strategy_logs_total,
            "stored_strategy_logs_total": stored_strategy_logs_total,
            "compact_backtest_diagnostics": compact_diagnostics,
            "historical_execution_delay_ms": execution_delay_ms,
            "profile": profile.to_json(evaluated_steps),
        }),
    );
    Ok(backtest.finish(
        &market,
        json!({
            "runtime_execution_stamp": runtime_execution_stamp,
            "context_requirements": context_data.requirement_summary(),
            "evaluated_steps": evaluated_steps,
            "total_steps": total,
            "strategy_logs_total": strategy_logs_total,
            "stored_strategy_logs_total": stored_strategy_logs_total,
            "compact_backtest_diagnostics": compact_diagnostics,
            "historical_execution_delay_ms": execution_delay_ms,
            "backtest_profile": profile.to_json(evaluated_steps),
        }),
    ))
}

pub(super) fn backtest_execution_submit_timestamp(timestamp: i64, config: &StrategyConfig) -> i64 {
    timestamp.saturating_add(backtest_execution_delay_ms(config))
}

pub(super) fn backtest_execution_delay_ms(config: &StrategyConfig) -> i64 {
    risk_controls::numeric_param_any(
        &config.params,
        &[
            "historical_execution_delay_ms",
            "backtest_execution_delay_ms",
        ],
    )
    .filter(|value| value.is_finite() && *value > 0.0)
    .map(|value| value.round().min(i64::MAX as f64) as i64)
    .unwrap_or(0)
}

fn runtime_config_json(config: &StrategyConfig) -> Value {
    json!({
        "strategy_id": config.strategy_id,
        "strategy_name": config.strategy_name,
        "symbol": config.symbol,
        "inst_type": config.inst_type,
        "timeframe": config.timeframe,
        "mode": "backtest",
        "execution_mode": "historical_sim",
        "initial_capital": config.initial_capital,
        "position_size": config.position_size,
        "stop_loss": config.stop_loss,
        "take_profit": config.take_profit,
        "params": config.params
    })
}
