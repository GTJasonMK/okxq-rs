use std::collections::HashSet;

use serde_json::{json, Value};

use crate::{
    app_state::AppState,
    backtest_progress::BacktestProgressReporter,
    commands::local_api::{
        backtest::{
            inputs::{load_backtest_context_candles_from_db, normalize_strategy_inst_id},
            window::BacktestWindow,
        },
        market_ops,
    },
    error::{AppError, AppResult},
    live_strategy::required_action_candle_count_for_timeframe,
    okx::OkxCandle,
    strategy_engine::{HistoricalCandleSeries, HistoricalMarketData, StrategyConfig},
    strategy_executor::{self, RuntimeCandleRequirement, RuntimeFundingRequirement},
    timeframes::okx_timeframe_millis_or,
    trading_semantics::has_periodic_funding_rate,
};

use super::funding::BacktestFundingContextData;

pub(in crate::commands::local_api::backtest::runner) fn reject_required_realtime_feeds(
    runtime_meta: &strategy_executor::types::RuntimeStrategyMeta,
    config: &StrategyConfig,
) -> AppResult<()> {
    let orderbook_requirements = strategy_executor::orderbook_requirements(
        &runtime_meta.data_requirements,
        &config.symbol,
        &config.inst_type,
    );
    if let Some(requirement) = orderbook_requirements.iter().find(|item| item.required) {
        return Err(AppError::Validation(format!(
            "{} 回测暂缺历史订单簿，不能执行声明 required orderbook 的策略",
            requirement.symbol
        )));
    }
    Ok(())
}

pub(in crate::commands::local_api::backtest::runner) struct BacktestContextData {
    series: Vec<BacktestContextSeries>,
}

struct BacktestContextSeries {
    requirement: RuntimeCandleRequirement,
    candles: Vec<OkxCandle>,
    timestamps: Vec<i64>,
    candle_jsons: Vec<Value>,
}

pub(in crate::commands::local_api::backtest::runner) struct BacktestContextWindow {
    pub(in crate::commands::local_api::backtest::runner) len: usize,
}

impl BacktestContextSeries {
    fn new(requirement: RuntimeCandleRequirement, candles: Vec<OkxCandle>) -> Self {
        let timestamps = candles.iter().map(|candle| candle.timestamp).collect();
        let candle_jsons = candles.iter().map(OkxCandle::to_json).collect();
        Self {
            requirement,
            candles,
            timestamps,
            candle_jsons,
        }
    }

    fn prefix_len_until(&self, timestamp: i64) -> usize {
        backtest_context_prefix_len_at_or_before(&self.timestamps, timestamp)
    }

    fn context_window_until(&self, timestamp: i64) -> BacktestContextWindow {
        let len = self.prefix_len_until(timestamp);
        BacktestContextWindow { len }
    }
}

impl BacktestContextData {
    pub(in crate::commands::local_api::backtest::runner) async fn load(
        state: &AppState,
        runtime_meta: &strategy_executor::types::RuntimeStrategyMeta,
        config: &StrategyConfig,
        window: BacktestWindow,
        progress: &BacktestProgressReporter,
    ) -> AppResult<Self> {
        let default_min_bars = default_backtest_primary_min_bars(config);
        let requirements = strategy_executor::candle_requirements(
            &runtime_meta.data_requirements,
            &config.symbol,
            &config.inst_type,
            &config.timeframe,
            default_min_bars,
        );
        let total_requirements = requirements.len().max(1);
        let mut series = Vec::new();
        for (index, requirement) in requirements.into_iter().enumerate() {
            let normalized = normalize_requirement(requirement)?;
            progress.report_detail(
                26,
                "context",
                &format!(
                    "加载上下文序列 {}/{} {} {}",
                    index + 1,
                    total_requirements,
                    normalized.symbol,
                    normalized.timeframe
                ),
                index,
                total_requirements,
                json!({
                    "step": "context_series_load",
                    "context_series_index": index + 1,
                    "context_series_count": total_requirements,
                    "symbol": normalized.symbol,
                    "inst_type": normalized.inst_type,
                    "timeframe": normalized.timeframe,
                    "min_bars": normalized.min_bars,
                    "role": normalized.role,
                }),
            );
            let req_config = StrategyConfig {
                symbol: normalized.symbol.clone(),
                inst_type: normalized.inst_type.clone(),
                timeframe: normalized.timeframe.clone(),
                ..config.clone()
            };
            let candles = load_backtest_requirement_context_candles(
                state,
                &req_config,
                window,
                normalized.min_bars,
            )
            .await?;
            progress.report_detail(
                28,
                "context",
                &format!(
                    "上下文序列已加载 {}/{} {} {}",
                    index + 1,
                    total_requirements,
                    normalized.symbol,
                    normalized.timeframe
                ),
                index + 1,
                total_requirements,
                json!({
                    "step": "context_series_ready",
                    "context_series_index": index + 1,
                    "context_series_count": total_requirements,
                    "symbol": normalized.symbol,
                    "inst_type": normalized.inst_type,
                    "timeframe": normalized.timeframe,
                    "min_bars": normalized.min_bars,
                    "role": normalized.role,
                    "loaded_candles": candles.len(),
                }),
            );
            series.push(BacktestContextSeries::new(normalized, candles));
        }
        Ok(Self { series })
    }

    pub(in crate::commands::local_api::backtest::runner) fn market_data(
        &self,
    ) -> HistoricalMarketData {
        HistoricalMarketData::new(
            self.series
                .iter()
                .map(|item| HistoricalCandleSeries {
                    symbol: item.requirement.symbol.clone(),
                    inst_type: item.requirement.inst_type.clone(),
                    timeframe: item.requirement.timeframe.clone(),
                    candles: item.candles.clone(),
                })
                .collect(),
        )
    }

    pub(in crate::commands::local_api::backtest::runner) fn execution_funding_requirements(
        &self,
    ) -> Vec<RuntimeFundingRequirement> {
        let mut seen = HashSet::new();
        let mut requirements = Vec::new();
        for item in &self.series {
            if !has_periodic_funding_rate(&item.requirement.inst_type) {
                continue;
            }
            let key = (
                item.requirement.symbol.trim().to_ascii_uppercase(),
                item.requirement.inst_type.trim().to_ascii_uppercase(),
            );
            if !seen.insert(key) {
                continue;
            }
            requirements.push(RuntimeFundingRequirement {
                symbol: item.requirement.symbol.clone(),
                inst_type: item.requirement.inst_type.clone(),
                history_limit: 0,
                required: true,
            });
        }
        requirements
    }

    pub(in crate::commands::local_api::backtest::runner) fn static_context(
        &self,
        config_json: &Value,
        funding_context_data: &BacktestFundingContextData,
        window: BacktestWindow,
        primary_window_candles: &[OkxCandle],
    ) -> Value {
        let candle_sets = self
            .series
            .iter()
            .map(|item| {
                (
                    item.requirement.clone(),
                    Value::Array(item.candle_jsons.clone()),
                )
            })
            .collect::<Vec<_>>();
        let mut context =
            strategy_executor::strategy_context(strategy_executor::StrategyContextInput {
                config: config_json,
                candles: strategy_executor::candle_tree(candle_sets),
                timestamp: 0,
                account: json!({}),
                positions: json!({}),
                orders: json!({}),
                funding: funding_context_data.full_context(window),
                orderbook: json!({}),
            });
        let object = context
            .as_object_mut()
            .expect("backtest static strategy context should be an object");
        object.insert(
            "_backtest_plan".to_string(),
            json!({
                "evaluation_timestamps": primary_window_candles
                    .iter()
                    .map(|candle| candle.timestamp)
                    .collect::<Vec<_>>(),
                "window_start": window.start_ts,
                "window_end": window.end_ts,
                "primary_symbol": config_json
                    .get("symbol")
                    .expect("backtest runtime config symbol should exist"),
                "primary_timeframe": config_json
                    .get("timeframe")
                    .expect("backtest runtime config timeframe should exist"),
            }),
        );
        context
    }

    pub(in crate::commands::local_api::backtest::runner) fn primary_context_until(
        &self,
        config: &StrategyConfig,
        timestamp: i64,
    ) -> AppResult<Option<BacktestContextWindow>> {
        let Some(series) = self.series.iter().find(|item| {
            item.requirement.symbol.eq_ignore_ascii_case(&config.symbol)
                && item
                    .requirement
                    .timeframe
                    .eq_ignore_ascii_case(&config.timeframe)
        }) else {
            return Err(AppError::Validation(format!(
                "回测上下文缺少主序列 {} {}",
                config.symbol, config.timeframe
            )));
        };
        let context = series.context_window_until(timestamp);
        if context.len < series.requirement.min_bars.max(3) {
            return Ok(None);
        }
        Ok(Some(context))
    }

    pub(in crate::commands::local_api::backtest::runner) fn context_ready_at(
        &self,
        timestamp: i64,
    ) -> bool {
        self.series
            .iter()
            .all(|item| item.prefix_len_until(timestamp) >= item.requirement.min_bars)
    }

    pub(in crate::commands::local_api::backtest::runner) fn series_count(&self) -> usize {
        self.series.len()
    }

    pub(in crate::commands::local_api::backtest::runner) fn requirement_summary(&self) -> Value {
        Value::Array(
            self.series
                .iter()
                .map(|item| {
                    json!({
                        "symbol": item.requirement.symbol,
                        "inst_type": item.requirement.inst_type,
                        "timeframe": item.requirement.timeframe,
                        "min_bars": item.requirement.min_bars,
                        "role": item.requirement.role,
                        "loaded_candles": item.candles.len(),
                    })
                })
                .collect(),
        )
    }

    pub(in crate::commands::local_api::backtest::runner) fn no_evaluable_window_detail(
        &self,
        primary_window_candles: &[OkxCandle],
        window: BacktestWindow,
    ) -> Option<Value> {
        let last_window_timestamp = primary_window_candles.last().map(|item| item.timestamp)?;
        let first_evaluable_timestamp = self.first_evaluable_timestamp();
        if first_evaluable_timestamp.is_some_and(|timestamp| timestamp <= last_window_timestamp) {
            return None;
        }
        let blockers = self
            .series
            .iter()
            .filter_map(|item| {
                let required = item.requirement.min_bars.max(3);
                let available_until_window_end = item.prefix_len_until(last_window_timestamp);
                if available_until_window_end >= required {
                    return None;
                }
                Some(json!({
                    "symbol": item.requirement.symbol,
                    "inst_type": item.requirement.inst_type,
                    "timeframe": item.requirement.timeframe,
                    "role": item.requirement.role,
                    "min_bars": required,
                    "available_until_window_end": available_until_window_end,
                    "missing_bars": required.saturating_sub(available_until_window_end),
                    "loaded_candles": item.candles.len(),
                    "first_loaded_timestamp": item.candles.first().map(|candle| candle.timestamp),
                    "last_loaded_timestamp": item.candles.last().map(|candle| candle.timestamp),
                    "required_start_timestamp": backtest_context_required_start_ts(
                        window,
                        &item.requirement.timeframe,
                        required,
                    ),
                    "required_end_timestamp": backtest_context_required_end_ts(
                        window,
                        &item.requirement.timeframe,
                    ),
                }))
            })
            .take(12)
            .collect::<Vec<_>>();
        Some(json!({
            "step": "warmup_blocked",
            "window_start": window.start_ts,
            "window_end": window.end_ts,
            "primary_window_candles": primary_window_candles.len(),
            "last_window_timestamp": last_window_timestamp,
            "first_evaluable_timestamp": first_evaluable_timestamp,
            "first_evaluable_time": first_evaluable_timestamp
                .map(timestamp_text)
                .unwrap_or_else(|| "不可用".to_string()),
            "last_window_time": timestamp_text(last_window_timestamp),
            "context_series_count": self.series.len(),
            "blocked_requirements": blockers,
            "context_requirements": self.requirement_summary(),
        }))
    }

    fn first_evaluable_timestamp(&self) -> Option<i64> {
        self.series
            .iter()
            .map(|item| {
                let required_index = item.requirement.min_bars.max(3).saturating_sub(1);
                item.candles
                    .get(required_index)
                    .map(|candle| candle.timestamp)
            })
            .collect::<Option<Vec<_>>>()
            .and_then(|timestamps| timestamps.into_iter().max())
    }
}

pub(in crate::commands::local_api::backtest) fn backtest_context_prefix_len_at_or_before(
    timestamps: &[i64],
    timestamp: i64,
) -> usize {
    timestamps.partition_point(|value| *value <= timestamp)
}

pub(in crate::commands::local_api::backtest) fn default_backtest_primary_min_bars(
    config: &StrategyConfig,
) -> usize {
    required_action_candle_count_for_timeframe(&config.params, &config.timeframe).clamp(3, 20_000)
}

async fn load_backtest_requirement_context_candles(
    state: &AppState,
    config: &StrategyConfig,
    window: BacktestWindow,
    min_bars: usize,
) -> AppResult<Vec<OkxCandle>> {
    let required_start_ts = backtest_context_required_start_ts(window, &config.timeframe, min_bars);
    let required_end_ts = backtest_context_required_end_ts(window, &config.timeframe);
    let local = load_backtest_context_candles_from_db(&state.db, config, window, min_bars).await?;
    if context_candles_cover_window_warmup(&local, min_bars, required_start_ts, required_end_ts) {
        return Ok(local);
    }
    market_ops::ensure_local_candles_range_coverage(
        state,
        &config.symbol,
        &config.inst_type,
        &config.timeframe,
        required_start_ts,
        required_end_ts,
    )
    .await?;
    load_backtest_context_candles_from_db(&state.db, config, window, min_bars).await
}

pub(in crate::commands::local_api::backtest) fn context_candles_cover_window_warmup(
    candles: &[OkxCandle],
    min_bars: usize,
    required_start_ts: i64,
    required_end_ts: i64,
) -> bool {
    candles.len() >= min_bars
        && candles
            .first()
            .is_some_and(|candle| candle.timestamp <= required_start_ts)
        && candles
            .last()
            .is_some_and(|candle| candle.timestamp >= required_end_ts)
}

pub(in crate::commands::local_api::backtest) fn backtest_context_required_start_ts(
    window: BacktestWindow,
    timeframe: &str,
    min_bars: usize,
) -> i64 {
    let timeframe_ms = okx_timeframe_millis_or(timeframe, 60_000).max(1);
    let warmup_bars = (min_bars as i64).saturating_sub(1).max(0);
    window
        .start_ts
        .saturating_sub(warmup_bars.saturating_mul(timeframe_ms))
}

pub(in crate::commands::local_api::backtest) fn backtest_context_required_end_ts(
    window: BacktestWindow,
    timeframe: &str,
) -> i64 {
    let timeframe_ms = okx_timeframe_millis_or(timeframe, 60_000).max(1);
    window
        .end_ts
        .saturating_sub(window.end_ts.rem_euclid(timeframe_ms))
}

fn timestamp_text(timestamp: i64) -> String {
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(timestamp)
        .map(|item| item.to_rfc3339())
        .unwrap_or_else(|| timestamp.to_string())
}

fn normalize_requirement(
    requirement: RuntimeCandleRequirement,
) -> AppResult<RuntimeCandleRequirement> {
    let symbol = normalize_strategy_inst_id(&requirement.symbol, &requirement.inst_type)?;
    Ok(RuntimeCandleRequirement {
        symbol,
        inst_type: requirement.inst_type,
        timeframe: requirement.timeframe,
        min_bars: requirement.min_bars.max(3),
        role: requirement.role,
    })
}
