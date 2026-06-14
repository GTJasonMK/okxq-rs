use serde_json::{json, Value};

use super::values::{parse_f64, parse_i64};

#[derive(Clone, Debug)]
pub struct OkxCandle {
    pub timestamp: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub volume_ccy: f64,
    pub volume_quote: f64,
    pub confirm: String,
}

impl OkxCandle {
    pub fn is_valid_market_candle(&self) -> bool {
        self.timestamp > 0
            && self.open.is_finite()
            && self.high.is_finite()
            && self.low.is_finite()
            && self.close.is_finite()
            && self.volume.is_finite()
            && self.volume_ccy.is_finite()
            && self.volume_quote.is_finite()
            && self.open > 0.0
            && self.high > 0.0
            && self.low > 0.0
            && self.close > 0.0
            && self.volume >= 0.0
            && self.volume_ccy >= 0.0
            && self.volume_quote >= 0.0
    }

    pub fn to_json(&self) -> Value {
        json!({
            "timestamp": self.timestamp,
            "open": self.open,
            "high": self.high,
            "low": self.low,
            "close": self.close,
            "volume": self.volume,
            "volume_ccy": self.volume_ccy,
            "volume_quote": self.volume_quote,
        })
    }

    pub fn to_json_with_confirm(&self) -> Value {
        let mut value = self.to_json();
        value["confirm"] = json!(self.confirm);
        value
    }
}

pub fn parse_okx_candle(value: Value) -> Option<OkxCandle> {
    let items = value.as_array()?;
    let candle = OkxCandle {
        timestamp: parse_i64(items.first()?)?,
        open: parse_f64(items.get(1)?)?,
        high: parse_f64(items.get(2)?)?,
        low: parse_f64(items.get(3)?)?,
        close: parse_f64(items.get(4)?)?,
        volume: parse_f64(items.get(5)?)?,
        volume_ccy: parse_f64(items.get(6)?)?,
        volume_quote: parse_f64(items.get(7)?)?,
        confirm: candle_confirm(items.get(8)?)?.to_string(),
    };
    candle.is_valid_market_candle().then_some(candle)
}

fn candle_confirm(value: &Value) -> Option<&'static str> {
    match value.as_str()? {
        "0" => Some("0"),
        "1" => Some("1"),
        _ => None,
    }
}

pub fn okx_bar(timeframe: &str) -> String {
    let trimmed = timeframe.trim();
    match trimmed {
        "1d" => "1D".to_string(),
        "1w" => "1W".to_string(),
        "1h" => "1H".to_string(),
        "2h" => "2H".to_string(),
        "4h" => "4H".to_string(),
        "6h" => "6H".to_string(),
        "12h" => "12H".to_string(),
        "1M" => "1M".to_string(),
        "1W" => "1W".to_string(),
        "1D" => "1D".to_string(),
        "1H" => "1H".to_string(),
        "2H" => "2H".to_string(),
        "4H" => "4H".to_string(),
        "6H" => "6H".to_string(),
        "12H" => "12H".to_string(),
        other if !other.is_empty() => other.to_string(),
        _ => "1H".to_string(),
    }
}
