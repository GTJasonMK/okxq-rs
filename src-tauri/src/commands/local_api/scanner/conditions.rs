use serde_json::{json, Map, Value};

use super::super::{round2, round4, simple_ma, simple_rsi, AppError, AppResult};

pub(super) fn evaluate_scan_condition(
    indicator: &str,
    operator: &str,
    target: f64,
    params: Option<&Map<String, Value>>,
    closes: &[f64],
) -> AppResult<(bool, String, Option<f64>)> {
    match indicator {
        "rsi" => {
            let period = params
                .and_then(|params| params.get("period"))
                .and_then(Value::as_u64)
                .unwrap_or(14) as usize;
            let value = simple_rsi(closes, period);
            Ok((
                compare(value, operator, target),
                format!("rsi({period})"),
                Some(round2(value)),
            ))
        }
        "sma_cross" => {
            let fast = params
                .and_then(|params| params.get("fast_period"))
                .and_then(Value::as_u64)
                .unwrap_or(5) as usize;
            let slow = params
                .and_then(|params| params.get("slow_period"))
                .and_then(Value::as_u64)
                .unwrap_or(20) as usize;
            let fast_ma = simple_ma(closes, fast);
            let slow_ma = simple_ma(closes, slow);
            let diff = fast_ma - slow_ma;
            Ok((
                compare(diff, operator, target),
                format!("sma({fast}/{slow})"),
                Some(round4(diff)),
            ))
        }
        "price" => {
            let value = closes[closes.len() - 1];
            Ok((
                compare(value, operator, target),
                "price".to_string(),
                Some(value),
            ))
        }
        _ => Err(AppError::Validation(format!(
            "不支持的扫描指标: {indicator}"
        ))),
    }
}

fn compare(value: f64, operator: &str, target: f64) -> bool {
    match operator {
        "gt" => value > target,
        "lt" => value < target,
        "gte" => value >= target,
        "lte" => value <= target,
        _ => false,
    }
}

pub(in crate::commands::local_api) fn scanner_conditions() -> Value {
    json!([
        {
            "indicator": "rsi",
            "label": "RSI",
            "operators": ["gt", "lt", "gte", "lte"],
            "params": {"period": {"type": "int", "default": 14, "min": 2, "max": 50}},
            "value_hint": "如 30（超卖）或 70（超买）"
        },
        {
            "indicator": "sma_cross",
            "label": "均线交叉",
            "operators": ["gt", "lt", "gte", "lte"],
            "params": {
                "fast_period": {"type": "int", "default": 5, "min": 2, "max": 100},
                "slow_period": {"type": "int", "default": 20, "min": 5, "max": 200}
            },
            "value_hint": "快慢均线差值阈值"
        },
        {
            "indicator": "price",
            "label": "价格",
            "operators": ["gt", "lt", "gte", "lte"],
            "params": {},
            "value_hint": "价格阈值"
        }
    ])
}
