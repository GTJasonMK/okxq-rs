use serde_json::{json, Value};

#[derive(Clone, Debug)]
pub(in crate::alerts) struct PriceAlert {
    pub(in crate::alerts) id: String,
    pub(in crate::alerts) inst_id: String,
    pub(in crate::alerts) symbol: String,
    pub(in crate::alerts) inst_type: String,
    pub(in crate::alerts) alert_type: String,
    pub(in crate::alerts) direction: String,
    pub(in crate::alerts) target_price: Option<f64>,
    pub(in crate::alerts) change_percent: Option<f64>,
    pub(in crate::alerts) note: String,
    pub(in crate::alerts) enabled: bool,
    pub(in crate::alerts) trigger_once: bool,
    pub(in crate::alerts) cooldown_seconds: i64,
    pub(in crate::alerts) created_at: String,
    pub(in crate::alerts) updated_at: String,
    pub(in crate::alerts) triggered_at: Option<String>,
    pub(in crate::alerts) last_value: Option<f64>,
    pub(in crate::alerts) last_trigger_value: Option<f64>,
    pub(in crate::alerts) last_trigger_ts: i64,
}

#[derive(Clone, Debug)]
pub struct TickerSnapshot {
    pub inst_id: String,
    pub inst_type: String,
    pub last_price: f64,
    pub change_24h: Option<f64>,
    pub ticker_ts: i64,
}

impl PriceAlert {
    pub(in crate::alerts) fn to_value(&self) -> Value {
        json!({
            "id": self.id,
            "inst_id": self.inst_id,
            "symbol": self.symbol,
            "inst_type": self.inst_type,
            "alert_type": self.alert_type,
            "direction": self.direction,
            "target_price": self.target_price,
            "change_percent": self.change_percent,
            "note": self.note,
            "enabled": self.enabled,
            "trigger_once": self.trigger_once,
            "cooldown_seconds": self.cooldown_seconds,
            "created_at": self.created_at,
            "updated_at": self.updated_at,
            "triggered_at": self.triggered_at,
            "last_value": self.last_value,
            "last_trigger_value": self.last_trigger_value,
            "last_trigger_ts": self.last_trigger_ts
        })
    }
}
