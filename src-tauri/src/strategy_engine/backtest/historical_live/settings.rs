use std::collections::HashMap;

use serde_json::Value;

use crate::{
    error::{AppError, AppResult},
    risk_controls,
    strategy_engine::StrategyConfig,
    trading_semantics::{
        contract_mode_param_or_default, explicit_configured_leverage_value, is_contract_inst_type,
        symbol_currencies, InstrumentTradeRules,
    },
};

use super::super::super::numbers::finite_or;
use super::values::norm_symbol;

const INSTRUMENT_RULES_BY_SYMBOL_KEY: &str = "_backtest_instrument_rules_by_symbol";

#[derive(Clone, Debug)]
pub(super) struct SimInstrumentRules {
    pub(super) symbol: String,
    pub(super) min_size: Option<f64>,
    pub(super) lot_size: Option<f64>,
    pub(super) tick_size: Option<f64>,
    pub(super) contract_value: Option<f64>,
    pub(super) contract_value_ccy: String,
}

#[derive(Clone, Debug)]
pub(super) struct SimSettings {
    pub(super) default_symbol: String,
    pub(super) initial_capital: f64,
    pub(super) contract_mode: bool,
    pub(super) leverage: f64,
    pub(super) fee_rate: f64,
    pub(super) slippage_rate: f64,
    pub(super) spread_rate: f64,
    pub(super) participation_rate: f64,
    pub(super) min_size: Option<f64>,
    pub(super) lot_size: Option<f64>,
    pub(super) tick_size: Option<f64>,
    pub(super) contract_value: Option<f64>,
    pub(super) contract_value_ccy: String,
    pub(super) instrument_rules_source: String,
    pub(super) min_notional: Option<f64>,
    pub(super) instrument_rules_by_symbol: HashMap<String, SimInstrumentRules>,
}

impl SimSettings {
    pub(super) fn try_from_config(config: &StrategyConfig) -> AppResult<Self> {
        let contract_mode =
            contract_mode_param_or_default(&config.params, &config.inst_type, "回测")?;
        if is_contract_inst_type(&config.inst_type) && !contract_mode {
            return Err(AppError::Validation(format!(
                "回测 inst_type={} 是合约交易，但 contract_mode=false；已拒绝运行以避免跳过合约杠杆和保证金语义",
                config.inst_type
            )));
        }
        let leverage = if contract_mode || is_contract_inst_type(&config.inst_type) {
            explicit_configured_leverage_value(&config.params, "回测")?.unwrap_or(1.0)
        } else {
            1.0
        };
        let default_symbol = norm_symbol(&config.symbol);
        let default_rules = SimInstrumentRules::from_params(&default_symbol, &config.params);
        let mut instrument_rules_by_symbol = instrument_rules_by_symbol(&config.params);
        instrument_rules_by_symbol
            .entry(default_symbol.clone())
            .or_insert_with(|| default_rules.clone());
        let fee_rate = bounded_number_any(
            &config.params,
            &["historical_fee_rate", "fee_rate", "commission_rate"],
            0.0005,
            0.0,
            0.02,
        )?;
        let slippage_rate = bounded_rate_or_bps_any(
            &config.params,
            &[
                "historical_slippage_rate",
                "simulation_slippage_rate",
                "slippage_rate",
            ],
            &[
                "historical_slippage_bps",
                "simulation_slippage_bps",
                "slippage_bps",
            ],
            5.0,
            0.05,
        )?;
        let spread_rate = bounded_rate_or_bps_any(
            &config.params,
            &[
                "historical_spread_rate",
                "simulation_spread_rate",
                "spread_rate",
            ],
            &[
                "historical_spread_bps",
                "simulation_spread_bps",
                "spread_bps",
            ],
            2.0,
            0.05,
        )?;
        let participation_rate = bounded_number_any(
            &config.params,
            &[
                "historical_participation_rate",
                "simulation_participation_rate",
                "max_volume_participation",
            ],
            0.02,
            0.000001,
            1.0,
        )?;
        Ok(Self {
            default_symbol,
            initial_capital: finite_or(config.initial_capital, 10_000.0).max(1.0),
            contract_mode,
            leverage,
            fee_rate,
            slippage_rate,
            spread_rate,
            participation_rate,
            min_size: default_rules.min_size,
            lot_size: default_rules.lot_size,
            tick_size: default_rules.tick_size,
            contract_value: default_rules.contract_value,
            contract_value_ccy: default_rules.contract_value_ccy.clone(),
            instrument_rules_source: string_opt_any(
                &config.params,
                &[
                    "_backtest_instrument_rules_source_resolved",
                    "backtest_instrument_rules_source",
                    "instrument_rules_source",
                    "historical_instrument_rules_source",
                ],
            )
            .unwrap_or_else(|| "simulated".to_string()),
            min_notional: number_opt_any(&config.params, &["min_notional", "min_order_notional"]),
            instrument_rules_by_symbol,
        })
    }

    pub(super) fn instrument_rules_for(&self, symbol: &str) -> AppResult<SimInstrumentRules> {
        let symbol = norm_symbol(symbol);
        if let Some(rules) = self.instrument_rules_by_symbol.get(&symbol) {
            return Ok(rules.clone());
        }
        if self
            .instrument_rules_source
            .eq_ignore_ascii_case("simulated")
        {
            return Ok(self.simulated_rules_for_symbol(&symbol));
        }
        Err(AppError::Validation(format!(
            "{} 缺少回测交易规格，不能复用 {} 的 ctVal/lotSz；请使用 backtest_instrument_rules_source=okx 或在 {} 中提供该 symbol",
            symbol, self.default_symbol, INSTRUMENT_RULES_BY_SYMBOL_KEY
        )))
    }

    fn simulated_rules_for_symbol(&self, symbol: &str) -> SimInstrumentRules {
        let (base_ccy, _) = symbol_currencies(symbol);
        SimInstrumentRules {
            symbol: symbol.to_string(),
            min_size: self.min_size,
            lot_size: self.lot_size,
            tick_size: self.tick_size,
            contract_value: self.contract_value,
            contract_value_ccy: if self.contract_mode {
                base_ccy
            } else {
                self.contract_value_ccy.clone()
            },
        }
    }
}

impl SimInstrumentRules {
    pub(super) fn to_trade_rules(&self) -> InstrumentTradeRules {
        InstrumentTradeRules {
            inst_id: self.symbol.clone(),
            min_sz: self.min_size,
            lot_sz: self.lot_size,
            tick_sz: self.tick_size,
            ct_val: self.contract_value,
            ct_val_ccy: self.contract_value_ccy.clone(),
        }
    }

    fn from_params(symbol: &str, params: &Value) -> Self {
        Self {
            symbol: norm_symbol(symbol),
            min_size: number_opt_any(params, &["min_size", "min_sz", "minSz"]),
            lot_size: number_opt_any(params, &["lot_size", "lot_sz", "lotSz"]),
            tick_size: number_opt_any(params, &["tick_size", "tick_sz", "tickSz"]),
            contract_value: number_opt_any(
                params,
                &["ctVal", "ct_val", "contract_value", "contract_size"],
            ),
            contract_value_ccy: string_opt_any(
                params,
                &[
                    "ctValCcy",
                    "ct_val_ccy",
                    "contract_value_ccy",
                    "contract_size_ccy",
                ],
            )
            .unwrap_or_default()
            .to_ascii_uppercase(),
        }
    }
}

fn instrument_rules_by_symbol(params: &Value) -> HashMap<String, SimInstrumentRules> {
    params
        .get(INSTRUMENT_RULES_BY_SYMBOL_KEY)
        .and_then(Value::as_object)
        .map(|items| {
            items
                .iter()
                .map(|(symbol, value)| {
                    let symbol = norm_symbol(
                        value
                            .get("instId")
                            .and_then(Value::as_str)
                            .unwrap_or(symbol),
                    );
                    let rules = SimInstrumentRules::from_params(&symbol, value);
                    (symbol, rules)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn number_opt_any(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|key| risk_controls::numeric_param(value, key))
}

fn bounded_number_any(
    value: &Value,
    keys: &[&str],
    fallback: f64,
    min: f64,
    max: f64,
) -> AppResult<f64> {
    let Some((key, number)) = explicit_number_any(value, keys)? else {
        return Ok(fallback);
    };
    validate_bounded_number(&key, number, min, max)?;
    Ok(number)
}

fn bounded_rate_or_bps_any(
    value: &Value,
    rate_keys: &[&str],
    bps_keys: &[&str],
    fallback_bps: f64,
    max_rate: f64,
) -> AppResult<f64> {
    if let Some((key, number)) = explicit_number_any(value, rate_keys)? {
        validate_bounded_number(&key, number, 0.0, max_rate)?;
        return Ok(number);
    }
    if let Some((key, bps)) = explicit_number_any(value, bps_keys)? {
        let max_bps = max_rate * 10_000.0;
        validate_bounded_number(&key, bps, 0.0, max_bps)?;
        return Ok(bps / 10_000.0);
    }
    Ok(fallback_bps / 10_000.0)
}

fn explicit_number_any(value: &Value, keys: &[&str]) -> AppResult<Option<(String, f64)>> {
    for key in keys {
        let Some(raw) = value.get(*key) else {
            continue;
        };
        if raw.is_null() {
            continue;
        }
        let Some(number) = parse_explicit_number(raw) else {
            return Err(AppError::Validation(format!(
                "回测 {key} 必须是有效数字；已拒绝运行以避免静默使用默认成本模型"
            )));
        };
        return Ok(Some(((*key).to_string(), number)));
    }
    Ok(None)
}

fn parse_explicit_number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    }
    .filter(|number| number.is_finite())
}

fn validate_bounded_number(key: &str, value: f64, min: f64, max: f64) -> AppResult<()> {
    if value < min || value > max {
        return Err(AppError::Validation(format!(
            "回测 {key}={} 超出允许范围 [{}, {}]；已拒绝运行以避免静默钳制成本模型参数",
            value, min, max
        )));
    }
    Ok(())
}

fn string_opt_any(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
    })
}
