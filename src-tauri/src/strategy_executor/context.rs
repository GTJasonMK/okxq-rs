use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCandleRequirement {
    pub symbol: String,
    pub inst_type: String,
    pub timeframe: String,
    pub min_bars: usize,
    pub role: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeOrderbookRequirement {
    pub symbol: String,
    pub inst_type: String,
    pub depth: usize,
    pub required: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeFundingRequirement {
    pub symbol: String,
    pub inst_type: String,
    pub history_limit: usize,
    pub required: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RuntimeFeedRequirementSpec {
    symbol: String,
    inst_type: String,
    quantity: usize,
    required: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeStateRequirements {
    pub account: bool,
    pub positions: bool,
    pub orders: bool,
}

pub fn state_context_requirements(data_requirements: &Value) -> RuntimeStateRequirements {
    RuntimeStateRequirements {
        account: state_section_enabled(data_requirements.get("account")),
        positions: state_section_enabled(data_requirements.get("positions")),
        orders: orders_section_enabled(data_requirements.get("orders")),
    }
}

pub fn candle_requirements(
    data_requirements: &Value,
    default_symbol: &str,
    default_inst_type: &str,
    default_timeframe: &str,
    default_min_bars: usize,
) -> Vec<RuntimeCandleRequirement> {
    let mut requirements = Vec::new();
    if let Some(candles) = data_requirements.get("candles").and_then(Value::as_array) {
        for item in candles.iter().filter_map(Value::as_object) {
            let symbol = object_string(item, "symbol")
                .or_else(|| object_string(item, "inst_id"))
                .unwrap_or_else(|| default_symbol.to_string());
            let inst_type =
                object_string(item, "inst_type").unwrap_or_else(|| default_inst_type.to_string());
            let timeframe =
                object_string(item, "timeframe").unwrap_or_else(|| default_timeframe.to_string());
            let min_bars = object_usize(item, "min_bars").unwrap_or(default_min_bars);
            let role = object_string(item, "role").unwrap_or_else(|| "context".to_string());
            push_requirement(
                &mut requirements,
                symbol,
                inst_type,
                timeframe,
                min_bars,
                role,
            );
        }
    }

    if requirements.is_empty() {
        let symbols = string_array(data_requirements.get("symbols"));
        let timeframes = string_array(data_requirements.get("timeframes"));
        if !symbols.is_empty() || !timeframes.is_empty() {
            let symbols = if symbols.is_empty() {
                vec![default_symbol.to_string()]
            } else {
                symbols
            };
            let timeframes = if timeframes.is_empty() {
                vec![default_timeframe.to_string()]
            } else {
                timeframes
            };
            for symbol in symbols {
                for timeframe in &timeframes {
                    push_requirement(
                        &mut requirements,
                        symbol.clone(),
                        default_inst_type.to_string(),
                        timeframe.clone(),
                        default_min_bars,
                        "context".to_string(),
                    );
                }
            }
        }
    }

    push_requirement(
        &mut requirements,
        default_symbol.to_string(),
        default_inst_type.to_string(),
        default_timeframe.to_string(),
        default_min_bars,
        "primary".to_string(),
    );

    requirements
}

pub fn orderbook_requirements(
    data_requirements: &Value,
    default_symbol: &str,
    default_inst_type: &str,
) -> Vec<RuntimeOrderbookRequirement> {
    let mut requirements = Vec::new();
    for spec in runtime_feed_requirement_specs(
        data_requirements,
        "orderbook",
        default_symbol,
        default_inst_type,
        1,
        &["depth", "size", "sz"],
    ) {
        push_orderbook_requirement(
            &mut requirements,
            spec.symbol,
            spec.inst_type,
            spec.quantity,
            spec.required,
        );
    }
    requirements
}

pub fn funding_requirements(
    data_requirements: &Value,
    default_symbol: &str,
    default_inst_type: &str,
) -> Vec<RuntimeFundingRequirement> {
    let mut requirements = Vec::new();
    for spec in runtime_feed_requirement_specs(
        data_requirements,
        "funding",
        default_symbol,
        default_inst_type,
        12,
        &["history_limit", "limit", "lookback_rows"],
    ) {
        push_funding_requirement(
            &mut requirements,
            spec.symbol,
            spec.inst_type,
            spec.quantity,
            spec.required,
        );
    }
    requirements
}

pub fn candle_tree(requirements: Vec<(RuntimeCandleRequirement, Value)>) -> Value {
    let mut tree = Map::new();
    for (requirement, candles) in requirements {
        let symbol_entry = tree
            .entry(requirement.symbol)
            .or_insert_with(|| Value::Object(Map::new()));
        let Some(timeframes) = symbol_entry.as_object_mut() else {
            continue;
        };
        timeframes.insert(requirement.timeframe, candles);
    }
    Value::Object(tree)
}

pub struct StrategyContextInput<'a> {
    pub config: &'a Value,
    pub candles: Value,
    pub timestamp: i64,
    pub account: Value,
    pub positions: Value,
    pub orders: Value,
    pub funding: Value,
    pub orderbook: Value,
}

pub fn strategy_context(input: StrategyContextInput<'_>) -> Value {
    let StrategyContextInput {
        config,
        candles,
        timestamp,
        account,
        positions,
        orders,
        funding,
        orderbook,
    } = input;
    let symbol = config_string(config, "symbol");
    let inst_type = config_string(config, "inst_type");
    let timeframe = config_string(config, "timeframe");
    json!({
        "candles": candles,
        "funding": object_or_empty(funding),
        "orderbook": object_or_empty(orderbook),
        "positions": object_or_empty(positions),
        "account": object_or_empty(account),
        "orders": orders_or_empty(orders),
        "time": {
            "timestamp": timestamp,
            "timeframe": timeframe,
        },
        "runtime": {
            "strategy_id": config_string(config, "strategy_id"),
            "strategy_name": config_string(config, "strategy_name"),
            "symbol": symbol,
            "inst_type": inst_type,
            "timeframe": timeframe,
        },
    })
}

pub fn context_cache_stamp(context: &Value) -> Value {
    let mut symbols = BTreeMap::<String, BTreeMap<String, Value>>::new();
    for (symbol, timeframes) in required_context_object(context, "candles") {
        let mut timeframe_stamps = BTreeMap::new();
        let timeframes = timeframes.as_object().unwrap_or_else(|| {
            panic!("strategy context candles.{symbol} should be a timeframe object")
        });
        for (timeframe, rows) in timeframes {
            let rows = rows.as_array().unwrap_or_else(|| {
                panic!("strategy context candles.{symbol}.{timeframe} should be an array")
            });
            let first = rows.first();
            let last = rows.last();
            timeframe_stamps.insert(
                timeframe.clone(),
                json!({
                    "len": rows.len(),
                    "first_ts": first.and_then(|row| row.get("timestamp")).and_then(Value::as_i64),
                    "last_ts": last.and_then(|row| row.get("timestamp")).and_then(Value::as_i64),
                    "last_close": last.and_then(|row| row.get("close")).and_then(Value::as_f64),
                }),
            );
        }
        symbols.insert(symbol.clone(), timeframe_stamps);
    }

    let orders = required_context_object(context, "orders");
    for key in ["open", "recent_fills", "recent_rejections"] {
        if !orders.get(key).is_some_and(Value::is_array) {
            panic!("strategy context orders.{key} should be an array");
        }
    }

    json!({
        "candles": symbols,
        "positions": Value::Object(required_context_object(context, "positions").clone()),
        "orders": Value::Object(orders.clone()),
        "account": Value::Object(required_context_object(context, "account").clone()),
        "funding": funding_stamp(required_context_value(context, "funding")),
        "orderbook": orderbook_stamp(required_context_value(context, "orderbook")),
    })
}

fn required_context_value<'a>(context: &'a Value, key: &str) -> &'a Value {
    context
        .get(key)
        .unwrap_or_else(|| panic!("strategy context {key} should exist"))
}

fn required_context_object<'a>(context: &'a Value, key: &str) -> &'a Map<String, Value> {
    required_context_value(context, key)
        .as_object()
        .unwrap_or_else(|| panic!("strategy context {key} should be an object"))
}

fn state_section_enabled(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(enabled)) => *enabled,
        Some(Value::Object(object)) => object_bool(object, "required").unwrap_or(false),
        _ => false,
    }
}

fn orders_section_enabled(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(enabled)) => *enabled,
        Some(Value::Object(object)) => {
            object_bool(object, "required").unwrap_or(false)
                || object_bool(object, "open").unwrap_or(false)
                || object_bool(object, "recent_fills").unwrap_or(false)
                || object_bool(object, "recent_rejections").unwrap_or(false)
        }
        _ => false,
    }
}

fn push_requirement(
    requirements: &mut Vec<RuntimeCandleRequirement>,
    symbol: String,
    inst_type: String,
    timeframe: String,
    min_bars: usize,
    role: String,
) {
    let symbol = symbol.trim().to_uppercase();
    let inst_type = inst_type.trim().to_uppercase();
    let timeframe = timeframe.trim().to_string();
    if symbol.is_empty() || timeframe.is_empty() {
        return;
    }
    let min_bars = min_bars.max(3);
    if let Some(existing) = requirements.iter_mut().find(|item| {
        item.symbol == symbol && item.inst_type == inst_type && item.timeframe == timeframe
    }) {
        existing.min_bars = existing.min_bars.max(min_bars);
        if existing.role != "primary" && role == "primary" {
            existing.role = role;
        }
        return;
    }
    requirements.push(RuntimeCandleRequirement {
        symbol,
        inst_type,
        timeframe,
        min_bars,
        role,
    });
}

fn runtime_feed_requirement_specs(
    data_requirements: &Value,
    section_key: &str,
    default_symbol: &str,
    default_inst_type: &str,
    default_quantity: usize,
    quantity_keys: &[&str],
) -> Vec<RuntimeFeedRequirementSpec> {
    let mut specs = Vec::new();
    let Some(section) = data_requirements.get(section_key) else {
        return specs;
    };

    match section {
        Value::Bool(true) => {
            for (symbol, inst_type) in
                symbol_universe(data_requirements, default_symbol, default_inst_type, None)
            {
                specs.push(RuntimeFeedRequirementSpec {
                    symbol,
                    inst_type,
                    quantity: default_quantity,
                    required: true,
                });
            }
        }
        Value::Array(items) => {
            for item in items {
                match item {
                    Value::String(symbol) => specs.push(RuntimeFeedRequirementSpec {
                        symbol: symbol.clone(),
                        inst_type: default_inst_type.to_string(),
                        quantity: default_quantity,
                        required: true,
                    }),
                    Value::Object(object) => {
                        let symbol =
                            object_symbol(object).unwrap_or_else(|| default_symbol.to_string());
                        let inst_type = object_string(object, "inst_type")
                            .unwrap_or_else(|| default_inst_type.to_string());
                        specs.push(RuntimeFeedRequirementSpec {
                            symbol,
                            inst_type,
                            quantity: object_usize_any(object, quantity_keys)
                                .unwrap_or(default_quantity),
                            required: object_bool(object, "required").unwrap_or(true),
                        });
                    }
                    _ => {}
                }
            }
        }
        Value::Object(object) => {
            let required = object_bool(object, "required").unwrap_or(true);
            let quantity = object_usize_any(object, quantity_keys).unwrap_or(default_quantity);
            for (symbol, inst_type) in runtime_feed_object_symbols(
                data_requirements,
                object,
                default_symbol,
                default_inst_type,
            ) {
                specs.push(RuntimeFeedRequirementSpec {
                    symbol,
                    inst_type,
                    quantity,
                    required,
                });
            }
        }
        _ => {}
    }

    specs
}

fn push_orderbook_requirement(
    requirements: &mut Vec<RuntimeOrderbookRequirement>,
    symbol: String,
    inst_type: String,
    depth: usize,
    required: bool,
) {
    let symbol = symbol.trim().to_uppercase();
    let inst_type = inst_type.trim().to_uppercase();
    if symbol.is_empty() {
        return;
    }
    let depth = depth.clamp(1, 400);
    if let Some(existing) = requirements
        .iter_mut()
        .find(|item| item.symbol == symbol && item.inst_type == inst_type)
    {
        existing.depth = existing.depth.max(depth);
        existing.required = existing.required || required;
        return;
    }
    requirements.push(RuntimeOrderbookRequirement {
        symbol,
        inst_type,
        depth,
        required,
    });
}

fn push_funding_requirement(
    requirements: &mut Vec<RuntimeFundingRequirement>,
    symbol: String,
    inst_type: String,
    history_limit: usize,
    required: bool,
) {
    let symbol = symbol.trim().to_uppercase();
    let inst_type = inst_type.trim().to_uppercase();
    if symbol.is_empty() {
        return;
    }
    let history_limit = history_limit.clamp(1, 2_000);
    if let Some(existing) = requirements
        .iter_mut()
        .find(|item| item.symbol == symbol && item.inst_type == inst_type)
    {
        existing.history_limit = existing.history_limit.max(history_limit);
        existing.required = existing.required || required;
        return;
    }
    requirements.push(RuntimeFundingRequirement {
        symbol,
        inst_type,
        history_limit,
        required,
    });
}

fn symbol_universe(
    data_requirements: &Value,
    default_symbol: &str,
    default_inst_type: &str,
    requirement_object: Option<&Map<String, Value>>,
) -> Vec<(String, String)> {
    let mut symbols = Vec::<(String, String)>::new();
    let inst_type = requirement_object
        .and_then(|object| object_string(object, "inst_type"))
        .unwrap_or_else(|| default_inst_type.to_string());
    for symbol in string_array(data_requirements.get("symbols")) {
        push_symbol(&mut symbols, symbol, inst_type.clone());
    }
    if let Some(candles) = data_requirements.get("candles").and_then(Value::as_array) {
        for item in candles.iter().filter_map(Value::as_object) {
            let symbol = object_string(item, "symbol")
                .or_else(|| object_string(item, "inst_id"))
                .unwrap_or_else(|| default_symbol.to_string());
            let inst_type =
                object_string(item, "inst_type").unwrap_or_else(|| default_inst_type.to_string());
            push_symbol(&mut symbols, symbol, inst_type);
        }
    }
    if symbols.is_empty() {
        push_symbol(
            &mut symbols,
            default_symbol.to_string(),
            default_inst_type.to_string(),
        );
    }
    symbols
}

fn push_symbol(symbols: &mut Vec<(String, String)>, symbol: String, inst_type: String) {
    let symbol = symbol.trim().to_uppercase();
    let inst_type = inst_type.trim().to_uppercase();
    if symbol.is_empty() {
        return;
    }
    if symbols.iter().any(|(existing_symbol, existing_inst_type)| {
        existing_symbol == &symbol && existing_inst_type == &inst_type
    }) {
        return;
    }
    symbols.push((symbol, inst_type));
}

fn runtime_feed_object_symbols(
    data_requirements: &Value,
    object: &Map<String, Value>,
    default_symbol: &str,
    default_inst_type: &str,
) -> Vec<(String, String)> {
    let inst_type =
        object_string(object, "inst_type").unwrap_or_else(|| default_inst_type.to_string());
    let explicit_symbols = string_array(object.get("symbols"))
        .into_iter()
        .map(|symbol| (symbol, inst_type.clone()))
        .collect::<Vec<_>>();
    if !explicit_symbols.is_empty() {
        return explicit_symbols;
    }
    if let Some(symbol) = object_symbol(object) {
        return vec![(symbol, inst_type)];
    }
    symbol_universe(
        data_requirements,
        default_symbol,
        default_inst_type,
        Some(object),
    )
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn object_symbol(object: &Map<String, Value>) -> Option<String> {
    object_string(object, "symbol").or_else(|| object_string(object, "inst_id"))
}

fn object_bool(object: &Map<String, Value>, key: &str) -> Option<bool> {
    object.get(key).and_then(Value::as_bool)
}

fn object_string(object: &Map<String, Value>, key: &str) -> Option<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
}

fn object_usize(object: &Map<String, Value>, key: &str) -> Option<usize> {
    let value = object.get(key)?;
    value
        .as_u64()
        .and_then(|item| usize::try_from(item).ok())
        .or_else(|| {
            value
                .as_i64()
                .filter(|item| *item >= 0)
                .and_then(|item| usize::try_from(item).ok())
        })
        .or_else(|| {
            value
                .as_f64()
                .filter(|item| item.is_finite() && *item >= 0.0)
                .map(|item| item.round() as usize)
        })
}

fn object_usize_any(object: &Map<String, Value>, keys: &[&str]) -> Option<usize> {
    keys.iter().find_map(|key| object_usize(object, key))
}

fn config_string(config: &Value, key: &str) -> String {
    config
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("strategy config {key} should be a string"))
        .to_string()
}

fn object_or_empty(value: Value) -> Value {
    if value.is_object() {
        value
    } else {
        json!({})
    }
}

fn orders_or_empty(value: Value) -> Value {
    let mut object = value.as_object().cloned().unwrap_or_else(Map::new);
    for key in ["open", "recent_fills", "recent_rejections"] {
        if !object.get(key).is_some_and(Value::is_array) {
            object.insert(key.to_string(), json!([]));
        }
    }
    Value::Object(object)
}

fn funding_stamp(value: &Value) -> Value {
    let items = value
        .as_object()
        .expect("strategy context funding should be an object");
    let mut stamp = Map::new();
    for (symbol, funding) in items {
        let Some(funding) = funding.as_object() else {
            continue;
        };
        let latest = funding.get("latest").and_then(Value::as_object);
        let history = funding
            .get("history")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        stamp.insert(
            symbol.clone(),
            json!({
                "source": funding.get("source").and_then(Value::as_str).unwrap_or(""),
                "history_len": history.len(),
                "first_funding_time": history
                    .first()
                    .and_then(|row| row.get("funding_time").or_else(|| row.get("timestamp")))
                    .and_then(Value::as_i64),
                "last_funding_time": history
                    .last()
                    .and_then(|row| row.get("funding_time").or_else(|| row.get("timestamp")))
                    .and_then(Value::as_i64)
                    .or_else(|| latest
                        .and_then(|row| row.get("funding_time").or_else(|| row.get("timestamp")))
                        .and_then(Value::as_i64)),
                "latest_rate": latest
                    .and_then(|row| row.get("funding_rate").or_else(|| row.get("rate")))
                    .and_then(Value::as_f64),
            }),
        );
    }
    Value::Object(stamp)
}

fn orderbook_stamp(value: &Value) -> Value {
    let items = value
        .as_object()
        .expect("strategy context orderbook should be an object");
    let mut stamp = Map::new();
    for (symbol, book) in items {
        let Some(book) = book.as_object() else {
            continue;
        };
        stamp.insert(
            symbol.clone(),
            json!({
                "best_bid": book.get("best_bid").and_then(Value::as_f64),
                "best_ask": book.get("best_ask").and_then(Value::as_f64),
                "mid_price": book.get("mid_price").and_then(Value::as_f64),
                "spread": book.get("spread").and_then(Value::as_f64),
                "ts": book.get("ts").and_then(Value::as_i64),
            }),
        );
    }
    Value::Object(stamp)
}
