use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Map, Value};

use crate::{
    app_state::AppState,
    commands::local_api::backtest::inputs::normalize_strategy_inst_id,
    error::{AppError, AppResult},
    strategy_engine::StrategyConfig,
    strategy_executor,
    trading_semantics::{is_contract_inst_type, symbol_currencies},
};

const INSTRUMENT_RULES_BY_SYMBOL_KEY: &str = "_backtest_instrument_rules_by_symbol";

pub(super) async fn backtest_config_with_instrument_rules(
    state: &AppState,
    runtime_meta: &strategy_executor::types::RuntimeStrategyMeta,
    config: &StrategyConfig,
) -> AppResult<StrategyConfig> {
    let declared_instruments =
        backtest_required_instruments(&runtime_meta.data_requirements, config)?;
    let strict_instruments =
        backtest_strict_instrument_rules(&runtime_meta.data_requirements, config)?;
    let source = backtest_instrument_rules_source(config)?;
    match source {
        BacktestInstrumentRulesSource::Simulated => {
            return Ok(config_with_simulated_instrument_rules_for(
                config,
                &declared_instruments,
            ));
        }
        BacktestInstrumentRulesSource::Params => {
            return Ok(config_with_resolved_instrument_rules_source(
                config, "params",
            ));
        }
        BacktestInstrumentRulesSource::Okx => {}
    }
    let required_inst_types = required_okx_instrument_types(&declared_instruments);
    if required_inst_types.is_empty() {
        return Ok(config_with_resolved_instrument_rules_source(config, "okx"));
    }
    let client = crate::commands::local_api::okx_client(state).await?;
    let mut instruments_by_inst_type = BTreeMap::new();
    for inst_type in required_inst_types {
        let instruments = client.get_instruments(&inst_type).await.map_err(|error| {
            AppError::Runtime(format!(
                "加载 OKX {inst_type} instruments 规则失败，无法按交易所规格执行回测: {error}"
            ))
        })?;
        instruments_by_inst_type.insert(inst_type, instruments);
    }
    config_with_okx_instrument_rules_from_instruments(
        config,
        &strict_instruments,
        &instruments_by_inst_type,
    )
}

pub(in crate::commands::local_api::backtest) fn config_with_okx_instrument_rules_from_instruments(
    config: &StrategyConfig,
    required_instruments: &[(String, String)],
    instruments_by_inst_type: &BTreeMap<String, Vec<Value>>,
) -> AppResult<StrategyConfig> {
    let primary_scope = normalized_instrument_scope(&config.symbol, &config.inst_type)?;
    let mut rules_by_symbol = Map::new();
    let mut found_required = BTreeMap::<(String, String), Value>::new();
    let required_set = required_instruments
        .iter()
        .filter(|(_, inst_type)| uses_okx_instrument_rules(inst_type))
        .map(|(symbol, inst_type)| (symbol.clone(), inst_type.clone()))
        .collect::<BTreeSet<_>>();
    for (inst_type, instruments) in instruments_by_inst_type {
        let inst_type = normalize_inst_type(inst_type);
        for item in instruments {
            let inst_id = json_text(item, "instId").to_ascii_uppercase();
            if inst_id.is_empty() {
                continue;
            }
            let scope = (inst_id.clone(), inst_type.clone());
            if required_set.contains(&scope) {
                found_required.insert(scope, item.clone());
            }
            let state_text = json_text(item, "state");
            if state_text.trim().is_empty() || state_text.eq_ignore_ascii_case("live") {
                rules_by_symbol.insert(inst_id, instrument_rules_value(item));
            }
        }
    }
    for (symbol, inst_type) in &required_set {
        let Some(instrument) = found_required.get(&(symbol.clone(), inst_type.clone())) else {
            return Err(AppError::Validation(format!(
                "OKX {inst_type} instruments 未返回 {symbol}，无法按交易所规格执行回测"
            )));
        };
        let state_text = json_text(instrument, "state");
        if !state_text.trim().is_empty() && !state_text.eq_ignore_ascii_case("live") {
            return Err(AppError::Validation(format!(
                "OKX instrument {symbol} 当前状态为 {state_text}，已拒绝回测执行"
            )));
        }
    }
    let mut params = config.params.as_object().cloned().unwrap_or_default();
    if let Some(instrument) =
        found_required.get(&(primary_scope.0.clone(), primary_scope.1.clone()))
    {
        for key in [
            "instId",
            "instType",
            "state",
            "minSz",
            "lotSz",
            "tickSz",
            "ctVal",
            "ctValCcy",
            "maxMktSz",
            "maxLmtSz",
            "maxStopSz",
        ] {
            if let Some(value) = instrument.get(key) {
                params.insert(key.to_string(), value.clone());
            }
        }
    }
    params.insert(
        INSTRUMENT_RULES_BY_SYMBOL_KEY.to_string(),
        Value::Object(rules_by_symbol),
    );
    params.insert(
        "_backtest_instrument_rules_source_resolved".to_string(),
        Value::String("okx".to_string()),
    );
    Ok(StrategyConfig {
        params: Value::Object(params),
        ..config.clone()
    })
}

pub(in crate::commands::local_api::backtest) fn backtest_required_instruments(
    data_requirements: &Value,
    config: &StrategyConfig,
) -> AppResult<Vec<(String, String)>> {
    let mut instruments = BTreeSet::<(String, String)>::new();
    push_required_instrument(&mut instruments, &config.symbol, &config.inst_type)?;
    for requirement in strategy_executor::candle_requirements(
        data_requirements,
        &config.symbol,
        &config.inst_type,
        &config.timeframe,
        3,
    ) {
        push_required_instrument(
            &mut instruments,
            &requirement.symbol,
            &requirement.inst_type,
        )?;
    }
    for requirement in strategy_executor::orderbook_requirements(
        data_requirements,
        &config.symbol,
        &config.inst_type,
    ) {
        push_required_instrument(
            &mut instruments,
            &requirement.symbol,
            &requirement.inst_type,
        )?;
    }
    for requirement in strategy_executor::funding_requirements(
        data_requirements,
        &config.symbol,
        &config.inst_type,
    ) {
        push_required_instrument(
            &mut instruments,
            &requirement.symbol,
            &requirement.inst_type,
        )?;
    }
    Ok(instruments.into_iter().collect())
}

pub(in crate::commands::local_api::backtest) fn backtest_strict_instrument_rules(
    data_requirements: &Value,
    config: &StrategyConfig,
) -> AppResult<Vec<(String, String)>> {
    let mut instruments = BTreeSet::<(String, String)>::new();
    push_required_instrument(&mut instruments, &config.symbol, &config.inst_type)?;
    for requirement in strategy_executor::candle_requirements(
        data_requirements,
        &config.symbol,
        &config.inst_type,
        &config.timeframe,
        3,
    ) {
        push_required_instrument(
            &mut instruments,
            &requirement.symbol,
            &requirement.inst_type,
        )?;
    }
    for requirement in strategy_executor::orderbook_requirements(
        data_requirements,
        &config.symbol,
        &config.inst_type,
    ) {
        if requirement.required {
            push_required_instrument(
                &mut instruments,
                &requirement.symbol,
                &requirement.inst_type,
            )?;
        }
    }
    for requirement in strategy_executor::funding_requirements(
        data_requirements,
        &config.symbol,
        &config.inst_type,
    ) {
        if requirement.required {
            push_required_instrument(
                &mut instruments,
                &requirement.symbol,
                &requirement.inst_type,
            )?;
        }
    }
    Ok(instruments.into_iter().collect())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::commands::local_api::backtest) enum BacktestInstrumentRulesSource {
    Simulated,
    Params,
    Okx,
}

pub(in crate::commands::local_api::backtest) fn backtest_instrument_rules_source(
    config: &StrategyConfig,
) -> AppResult<BacktestInstrumentRulesSource> {
    let raw = config
        .params
        .get("backtest_instrument_rules_source")
        .or_else(|| config.params.get("instrument_rules_source"))
        .or_else(|| config.params.get("historical_instrument_rules_source"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("simulated")
        .to_ascii_lowercase();
    match raw.as_str() {
        "simulated" | "simulation" | "mock" | "local" => {
            Ok(BacktestInstrumentRulesSource::Simulated)
        }
        "params" | "manual" | "custom" => Ok(BacktestInstrumentRulesSource::Params),
        "okx" | "exchange" | "public" | "live" => Ok(BacktestInstrumentRulesSource::Okx),
        _ => Err(AppError::Validation(format!(
            "回测参数 backtest_instrument_rules_source={} 无效，只支持 simulated、params、okx",
            raw
        ))),
    }
}

pub(in crate::commands::local_api::backtest) fn config_with_resolved_instrument_rules_source(
    config: &StrategyConfig,
    source: &str,
) -> StrategyConfig {
    let mut params = config.params.as_object().cloned().unwrap_or_default();
    params.insert(
        "_backtest_instrument_rules_source_resolved".to_string(),
        Value::String(source.to_string()),
    );
    StrategyConfig {
        params: Value::Object(params),
        ..config.clone()
    }
}

#[cfg(test)]
pub(in crate::commands::local_api::backtest) fn config_with_simulated_instrument_rules(
    config: &StrategyConfig,
) -> StrategyConfig {
    let required_instruments = vec![
        normalized_instrument_scope(&config.symbol, &config.inst_type).unwrap_or_else(|_| {
            (
                config.symbol.trim().to_ascii_uppercase(),
                normalize_inst_type(&config.inst_type),
            )
        }),
    ];
    config_with_simulated_instrument_rules_for(config, &required_instruments)
}

pub(in crate::commands::local_api::backtest) fn config_with_simulated_instrument_rules_for(
    config: &StrategyConfig,
    required_instruments: &[(String, String)],
) -> StrategyConfig {
    let mut params = config.params.as_object().cloned().unwrap_or_default();
    insert_string_if_missing(&mut params, "backtest_instrument_rules_source", "simulated");
    params.insert(
        "_backtest_instrument_rules_source_resolved".to_string(),
        Value::String("simulated".to_string()),
    );
    insert_string_if_missing(&mut params, "instId", config.symbol.trim());
    insert_string_if_missing(&mut params, "instType", config.inst_type.trim());
    insert_string_if_missing(&mut params, "state", "simulated");
    insert_number_if_missing(&mut params, "tickSz", 0.00000001);
    if is_contract_inst_type(&config.inst_type) {
        let (base_ccy, _) = symbol_currencies(&config.symbol);
        insert_number_if_missing(&mut params, "ctVal", 1.0);
        insert_string_if_missing(&mut params, "ctValCcy", &base_ccy);
        insert_number_if_missing(&mut params, "lotSz", 1.0);
        insert_number_if_missing(&mut params, "minSz", 1.0);
    } else {
        insert_number_if_missing(&mut params, "lotSz", 0.00000001);
        insert_number_if_missing(&mut params, "minSz", 0.00001);
    }
    let primary_scope = normalized_instrument_scope(&config.symbol, &config.inst_type)
        .unwrap_or_else(|_| {
            (
                config.symbol.trim().to_ascii_uppercase(),
                normalize_inst_type(&config.inst_type),
            )
        });
    let mut rules_by_symbol = params
        .get(INSTRUMENT_RULES_BY_SYMBOL_KEY)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for (symbol, inst_type) in required_instruments {
        let symbol = symbol.trim().to_ascii_uppercase();
        let inst_type = normalize_inst_type(inst_type);
        if symbol.is_empty() || inst_type.is_empty() || rules_by_symbol.contains_key(&symbol) {
            continue;
        }
        let use_top_level_params = symbol == primary_scope.0 && inst_type == primary_scope.1;
        rules_by_symbol.insert(
            symbol.clone(),
            simulated_instrument_rules_value(&symbol, &inst_type, &params, use_top_level_params),
        );
    }
    if rules_by_symbol.is_empty() {
        let symbol = primary_scope.0.clone();
        rules_by_symbol.insert(
            symbol.clone(),
            simulated_instrument_rules_value(&symbol, &primary_scope.1, &params, true),
        );
    }
    params.insert(
        INSTRUMENT_RULES_BY_SYMBOL_KEY.to_string(),
        Value::Object(rules_by_symbol),
    );
    StrategyConfig {
        params: Value::Object(params),
        ..config.clone()
    }
}

pub(super) fn resolved_instrument_rules_source(config: &StrategyConfig) -> String {
    for key in [
        "_backtest_instrument_rules_source_resolved",
        "backtest_instrument_rules_source",
        "instrument_rules_source",
        "historical_instrument_rules_source",
    ] {
        if let Some(value) = config
            .params
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return value.to_string();
        }
    }
    "simulated".to_string()
}

fn insert_string_if_missing(params: &mut Map<String, Value>, key: &str, value: &str) {
    if params.get(key).is_some() || value.trim().is_empty() {
        return;
    }
    params.insert(key.to_string(), Value::String(value.trim().to_string()));
}

fn insert_number_if_missing(params: &mut Map<String, Value>, key: &str, value: f64) {
    if params.get(key).is_some() || !value.is_finite() || value <= 0.0 {
        return;
    }
    params.insert(key.to_string(), json!(value));
}

fn uses_okx_instrument_rules(inst_type: &str) -> bool {
    matches!(
        inst_type.trim().to_ascii_uppercase().as_str(),
        "SPOT" | "SWAP" | "FUTURES"
    )
}

fn required_okx_instrument_types(required_instruments: &[(String, String)]) -> Vec<String> {
    required_instruments
        .iter()
        .map(|(_, inst_type)| normalize_inst_type(inst_type))
        .filter(|inst_type| uses_okx_instrument_rules(inst_type))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn push_required_instrument(
    instruments: &mut BTreeSet<(String, String)>,
    symbol: &str,
    inst_type: &str,
) -> AppResult<()> {
    let (symbol, inst_type) = normalized_instrument_scope(symbol, inst_type)?;
    if !symbol.is_empty() && !inst_type.is_empty() {
        instruments.insert((symbol, inst_type));
    }
    Ok(())
}

fn normalized_instrument_scope(symbol: &str, inst_type: &str) -> AppResult<(String, String)> {
    let inst_type = normalize_inst_type(inst_type);
    let symbol = normalize_strategy_inst_id(symbol, &inst_type)?;
    Ok((symbol, inst_type))
}

fn normalize_inst_type(inst_type: &str) -> String {
    inst_type.trim().to_ascii_uppercase()
}

fn simulated_instrument_rules_value(
    symbol: &str,
    inst_type: &str,
    params: &Map<String, Value>,
    use_top_level_params: bool,
) -> Value {
    let mut rules = Map::new();
    rules.insert(
        "instId".to_string(),
        Value::String(symbol.trim().to_ascii_uppercase()),
    );
    rules.insert(
        "instType".to_string(),
        Value::String(inst_type.trim().to_ascii_uppercase()),
    );
    rules.insert("state".to_string(), Value::String("simulated".to_string()));
    let is_contract = is_contract_inst_type(inst_type);
    insert_simulated_number_rule(
        &mut rules,
        params,
        "tickSz",
        0.00000001,
        use_top_level_params,
    );
    if is_contract {
        let (base_ccy, _) = symbol_currencies(symbol);
        insert_simulated_number_rule(&mut rules, params, "ctVal", 1.0, use_top_level_params);
        insert_simulated_string_rule(
            &mut rules,
            params,
            "ctValCcy",
            &base_ccy,
            use_top_level_params,
        );
        insert_simulated_number_rule(&mut rules, params, "lotSz", 1.0, use_top_level_params);
        insert_simulated_number_rule(&mut rules, params, "minSz", 1.0, use_top_level_params);
    } else {
        insert_simulated_number_rule(
            &mut rules,
            params,
            "lotSz",
            0.00000001,
            use_top_level_params,
        );
        insert_simulated_number_rule(&mut rules, params, "minSz", 0.00001, use_top_level_params);
    }
    Value::Object(rules)
}

fn insert_simulated_number_rule(
    rules: &mut Map<String, Value>,
    params: &Map<String, Value>,
    key: &str,
    fallback: f64,
    use_top_level_params: bool,
) {
    if use_top_level_params {
        if let Some(value) = params.get(key) {
            rules.insert(key.to_string(), value.clone());
            return;
        }
    }
    rules.insert(key.to_string(), json!(fallback));
}

fn insert_simulated_string_rule(
    rules: &mut Map<String, Value>,
    params: &Map<String, Value>,
    key: &str,
    fallback: &str,
    use_top_level_params: bool,
) {
    if use_top_level_params {
        if let Some(value) = params.get(key) {
            rules.insert(key.to_string(), value.clone());
            return;
        }
    }
    rules.insert(key.to_string(), Value::String(fallback.to_string()));
}

fn instrument_rules_value(item: &Value) -> Value {
    let mut rules = Map::new();
    for key in [
        "instId",
        "instType",
        "state",
        "minSz",
        "lotSz",
        "tickSz",
        "ctVal",
        "ctValCcy",
        "maxMktSz",
        "maxLmtSz",
        "maxStopSz",
    ] {
        if let Some(value) = item.get(key) {
            rules.insert(key.to_string(), value.clone());
        }
    }
    Value::Object(rules)
}

fn json_text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}
