use serde::Serialize;
use serde_json::{json, Map, Value};

/// 运行策略元数据（从 .py 文件发现）
#[derive(Clone, Debug)]
pub struct RuntimeStrategyMeta {
    pub strategy_id: String,
    pub strategy_name: String,
    pub description: String,
    pub strategy_type: String,
    pub data_requirements: Value,
    pub runtime_config: Value,
    pub visualization: Value,
    pub decision_contract: Value,
    pub file_name: String,
}

/// 运行策略动作/交易意图（Python evaluate() 返回的标准化 action）
#[derive(Clone, Debug)]
pub struct RuntimeStrategyAction {
    pub action: String,
    pub symbol: String,
    pub side: String,
    pub order_type: String,
    pub price: Option<f64>,
    pub reference_price: Option<f64>,
    pub reason: String,
    pub strength: f64,
    pub timestamp: i64,
    pub position_size: Option<f64>,
    pub exchange_size: Option<String>,
    pub raw: Value,
}

#[derive(Clone, Debug, Serialize)]
pub struct RuntimeStrategyExecutionLog {
    pub stage: String,
    pub level: String,
    pub message: String,
    pub details: Value,
}

impl RuntimeStrategyAction {
    #[cfg(test)]
    pub fn from_value(value: &Value) -> Self {
        Self {
            action: string_field(value, &["action"], ""),
            symbol: string_field(value, &["symbol"], ""),
            side: string_field(value, &["side"], ""),
            order_type: string_field(value, &["order_type"], "market"),
            price: f64_field(value, &["price"]),
            reference_price: f64_field(value, &["reference_price"]),
            reason: string_field(value, &["reason"], ""),
            strength: f64_field(value, &["strength"]).unwrap_or(0.0),
            timestamp: i64_field(value, &["timestamp"]).unwrap_or(0),
            position_size: f64_field(value, &["position_size"]),
            exchange_size: text_field(value, &["exchange_size"]),
            raw: value.clone(),
        }
    }

    pub fn to_value(&self) -> Value {
        let mut item = self.raw.as_object().cloned().unwrap_or_else(Map::new);
        item.insert("action".to_string(), json!(self.action));
        item.insert("symbol".to_string(), json!(self.symbol));
        item.insert("side".to_string(), json!(self.side));
        item.insert("order_type".to_string(), json!(self.order_type));
        item.insert("reason".to_string(), json!(self.reason));
        item.insert("strength".to_string(), json!(self.strength));
        item.insert("timestamp".to_string(), json!(self.timestamp));
        if let Some(price) = self.price {
            item.insert("price".to_string(), json!(price));
        }
        if let Some(reference_price) = self.reference_price {
            item.insert("reference_price".to_string(), json!(reference_price));
        }
        if let Some(position_size) = self.position_size {
            item.insert("position_size".to_string(), json!(position_size));
        }
        if let Some(exchange_size) = self.exchange_size.as_deref() {
            item.insert("exchange_size".to_string(), json!(exchange_size));
        }
        Value::Object(item)
    }
}

#[cfg(test)]
fn string_field(value: &Value, keys: &[&str], default: &str) -> String {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .filter(|item| !item.trim().is_empty())
        .unwrap_or(default)
        .to_string()
}

#[cfg(test)]
fn text_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value.get(*key).and_then(|item| match item {
            Value::String(value) => {
                let value = value.trim();
                (!value.is_empty()).then(|| value.to_string())
            }
            _ => None,
        })
    })
}

#[cfg(test)]
fn f64_field(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(|item| {
                item.as_f64()
                    .or_else(|| item.as_i64().map(|value| value as f64))
                    .or_else(|| item.as_u64().map(|value| value as f64))
            })
            .filter(|item| item.is_finite())
    })
}

#[cfg(test)]
fn i64_field(value: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|key| {
        value.get(*key).and_then(|item| {
            item.as_i64()
                .or_else(|| item.as_u64().and_then(|value| i64::try_from(value).ok()))
        })
    })
}

/// 运行策略决策：新执行层消费 actions。
#[derive(Clone, Debug)]
pub struct RuntimeStrategyDecision {
    pub actions: Vec<RuntimeStrategyAction>,
    pub execution_logs: Vec<RuntimeStrategyExecutionLog>,
    pub indicators: Value,
    pub diagnostics: Value,
}
