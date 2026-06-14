use std::collections::HashSet;

use serde_json::{json, Map, Value};

use crate::{
    app_state::AppState,
    commands::local_api::backtest::window::BacktestWindow,
    error::{AppError, AppResult},
    risk_controls,
    strategy_engine::{HistoricalFundingPoint, HistoricalFundingSeries, StrategyConfig},
    strategy_executor::{self, RuntimeFundingRequirement},
    trading_semantics::has_periodic_funding_rate,
};

use super::candles::backtest_context_prefix_len_at_or_before;

pub(in crate::commands::local_api::backtest::runner) struct BacktestFundingContextData {
    series: Vec<BacktestFundingSeries>,
    execution_series: Vec<BacktestFundingSeries>,
}

struct BacktestFundingSeries {
    requirement: RuntimeFundingRequirement,
    funding_times: Vec<i64>,
    history: Vec<Value>,
}

impl BacktestFundingContextData {
    pub(in crate::commands::local_api::backtest::runner) async fn load(
        state: &AppState,
        data_requirements: &Value,
        config: &StrategyConfig,
        window: BacktestWindow,
        execution_requirements: Vec<RuntimeFundingRequirement>,
    ) -> AppResult<Self> {
        let requirements = strategy_executor::funding_requirements(
            data_requirements,
            &config.symbol,
            &config.inst_type,
        );
        let execution_requirements =
            execution_funding_requirements(execution_requirements, &requirements, &config.params);
        if requirements.is_empty() && execution_requirements.is_empty() {
            return Ok(Self {
                series: Vec::new(),
                execution_series: Vec::new(),
            });
        }
        let table_exists = strategy_executor::local_funding_table_exists(&state.db).await?;
        if !table_exists {
            if let Some(requirement) = requirements.iter().find(|item| item.required) {
                return Err(AppError::Validation(format!(
                    "{} 回测缺少 okx_funding_rates 表，不能执行声明 DATA_REQUIREMENTS.funding 的策略",
                    requirement.symbol
                )));
            }
            if let Some(requirement) = execution_requirements.iter().find(|item| item.required) {
                return Err(AppError::Validation(format!(
                    "{} 回测缺少 okx_funding_rates 表，不能结算 SWAP 历史资金费率；如需显式关闭，请设置 historical_funding_required=false",
                    requirement.symbol
                )));
            }
            return Ok(Self {
                series: Vec::new(),
                execution_series: Vec::new(),
            });
        }

        let mut series = Vec::new();
        for requirement in requirements {
            let normalized = strategy_executor::normalize_funding_requirement(requirement)?;
            let history = strategy_executor::load_local_funding_history_until_checked(
                &state.db,
                &normalized,
                window.end_ts,
                table_exists,
            )
            .await?;
            let funding_times = history
                .iter()
                .map(funding_history_timestamp)
                .collect::<Vec<_>>();
            series.push(BacktestFundingSeries {
                requirement: normalized,
                funding_times,
                history,
            });
        }
        let mut execution_series = Vec::new();
        for requirement in execution_requirements {
            let normalized = strategy_executor::normalize_funding_requirement(requirement)?;
            let history = strategy_executor::load_local_funding_history_until_checked(
                &state.db,
                &normalized,
                window.end_ts,
                table_exists,
            )
            .await?;
            if history.is_empty() && normalized.required {
                return Err(AppError::Validation(format!(
                    "{} 回测缺少截至 {} 的历史资金费率，不能结算 SWAP funding 成本；如需显式关闭，请设置 historical_funding_required=false",
                    normalized.symbol, window.end_ts
                )));
            }
            let funding_times = history
                .iter()
                .map(funding_history_timestamp)
                .collect::<Vec<_>>();
            execution_series.push(BacktestFundingSeries {
                requirement: normalized,
                funding_times,
                history,
            });
        }
        Ok(Self {
            series,
            execution_series,
        })
    }

    pub(in crate::commands::local_api::backtest::runner) fn ensure_available_at(
        &self,
        timestamp: i64,
    ) -> AppResult<()> {
        for item in &self.series {
            let end = backtest_context_prefix_len_at_or_before(&item.funding_times, timestamp);
            if end == 0 && item.requirement.required {
                return Err(AppError::Validation(format!(
                    "{} 截至 {} 缺少资金费率上下文，不能执行声明 DATA_REQUIREMENTS.funding 的策略",
                    item.requirement.symbol, timestamp
                )));
            }
        }
        Ok(())
    }

    pub(super) fn full_context(&self, window: BacktestWindow) -> Value {
        let mut funding = Map::new();
        for item in &self.series {
            let mut value = strategy_executor::funding_context_value_from_history(
                "okx_funding_rates",
                item.history.clone(),
            );
            let object = value
                .as_object_mut()
                .expect("funding context value should be an object");
            object.insert(
                "_history_limit".to_string(),
                json!(backtest_funding_history_limit(
                    item.requirement.history_limit,
                    window
                )),
            );
            funding.insert(item.requirement.symbol.clone(), value);
        }
        Value::Object(funding)
    }

    pub(in crate::commands::local_api::backtest::runner) fn execution_funding_series(
        &self,
    ) -> Vec<HistoricalFundingSeries> {
        self.execution_series
            .iter()
            .filter_map(|item| {
                let rates = item
                    .history
                    .iter()
                    .map(historical_funding_point)
                    .collect::<Vec<_>>();
                (!rates.is_empty()).then(|| HistoricalFundingSeries {
                    symbol: item.requirement.symbol.clone(),
                    inst_type: item.requirement.inst_type.clone(),
                    rates,
                })
            })
            .collect()
    }

    pub(in crate::commands::local_api::backtest::runner) fn series_count(&self) -> usize {
        self.series.len()
    }

    pub(in crate::commands::local_api::backtest::runner) fn total_rows(&self) -> usize {
        self.series.iter().map(|item| item.history.len()).sum()
    }

    pub(in crate::commands::local_api::backtest::runner) fn execution_series_count(&self) -> usize {
        self.execution_series.len()
    }

    pub(in crate::commands::local_api::backtest::runner) fn execution_total_rows(&self) -> usize {
        self.execution_series
            .iter()
            .map(|item| item.history.len())
            .sum()
    }
}

fn backtest_funding_history_limit(history_limit: usize, window: BacktestWindow) -> usize {
    let window_funding_rows = window.days.saturating_mul(3).max(0) as usize;
    history_limit
        .saturating_add(window_funding_rows)
        .max(history_limit)
        .clamp(1, 20_000)
}

fn funding_history_timestamp(item: &Value) -> i64 {
    item.get("funding_time")
        .and_then(Value::as_i64)
        .filter(|value| *value > 0)
        .expect("local funding history row funding_time should be a positive integer")
}

fn historical_funding_point(item: &Value) -> HistoricalFundingPoint {
    let funding_time = funding_history_timestamp(item);
    let funding_rate = item
        .get("funding_rate")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .expect("local funding history row funding_rate should be a finite number");
    let symbol = item
        .get("inst_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .expect("local funding history row inst_id should be a non-empty string")
        .to_string();
    let inst_type = item
        .get("inst_type")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .expect("local funding history row inst_type should be a non-empty string")
        .to_string();
    HistoricalFundingPoint {
        symbol,
        inst_type,
        funding_time,
        funding_rate,
    }
}

fn backtest_execution_funding_required(params: &Value) -> bool {
    risk_controls::bool_param_any(
        params,
        &["historical_funding_required", "backtest_funding_required"],
    )
    .unwrap_or(true)
}

fn execution_funding_requirements(
    market_requirements: Vec<RuntimeFundingRequirement>,
    context_requirements: &[RuntimeFundingRequirement],
    params: &Value,
) -> Vec<RuntimeFundingRequirement> {
    let required = backtest_execution_funding_required(params);
    let mut seen = HashSet::new();
    let mut output = Vec::new();
    for mut requirement in market_requirements
        .into_iter()
        .chain(context_requirements.iter().cloned())
    {
        if !has_periodic_funding_rate(&requirement.inst_type) {
            continue;
        }
        let key = (
            requirement.symbol.trim().to_ascii_uppercase(),
            requirement.inst_type.trim().to_ascii_uppercase(),
        );
        if !seen.insert(key) {
            continue;
        }
        requirement.required = required;
        output.push(requirement);
    }
    output
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{
        commands::local_api::backtest::window::BacktestWindow,
        strategy_executor::RuntimeFundingRequirement,
    };

    use super::{BacktestFundingContextData, BacktestFundingSeries};

    #[test]
    fn full_context_attaches_backtest_history_limit_to_funding_context() {
        let context = BacktestFundingContextData {
            series: vec![BacktestFundingSeries {
                requirement: RuntimeFundingRequirement {
                    symbol: "BTC-USDT-SWAP".to_string(),
                    inst_type: "SWAP".to_string(),
                    history_limit: 12,
                    required: true,
                },
                funding_times: vec![1_000],
                history: vec![json!({
                    "inst_id": "BTC-USDT-SWAP",
                    "inst_type": "SWAP",
                    "funding_time": 1_000,
                    "funding_rate": 0.0001
                })],
            }],
            execution_series: Vec::new(),
        };

        let value = context.full_context(BacktestWindow {
            start_ts: 0,
            end_ts: 86_400_000,
            days: 1,
        });

        assert_eq!(value["BTC-USDT-SWAP"]["source"], json!("okx_funding_rates"));
        assert_eq!(value["BTC-USDT-SWAP"]["_history_limit"], json!(15));
        assert_eq!(
            value["BTC-USDT-SWAP"]["latest"]["funding_time"],
            json!(1_000)
        );
    }

    #[test]
    fn execution_funding_series_uses_canonical_local_funding_rows() {
        let context = BacktestFundingContextData {
            series: Vec::new(),
            execution_series: vec![BacktestFundingSeries {
                requirement: RuntimeFundingRequirement {
                    symbol: "BTC-USDT-SWAP".to_string(),
                    inst_type: "SWAP".to_string(),
                    history_limit: 12,
                    required: true,
                },
                funding_times: vec![1_000],
                history: vec![json!({
                    "inst_id": "BTC-USDT-SWAP",
                    "inst_type": "SWAP",
                    "funding_time": 1_000,
                    "funding_rate": 0.0001
                })],
            }],
        };

        let series = context.execution_funding_series();

        assert_eq!(series.len(), 1);
        assert_eq!(series[0].symbol, "BTC-USDT-SWAP");
        assert_eq!(series[0].inst_type, "SWAP");
        assert_eq!(series[0].rates.len(), 1);
        assert_eq!(series[0].rates[0].symbol, "BTC-USDT-SWAP");
        assert_eq!(series[0].rates[0].inst_type, "SWAP");
        assert_eq!(series[0].rates[0].funding_time, 1_000);
        assert_eq!(series[0].rates[0].funding_rate, 0.0001);
    }
}
