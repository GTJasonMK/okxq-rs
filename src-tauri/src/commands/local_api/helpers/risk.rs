use serde_json::{json, Map, Value};

use crate::app_state::AppState;

pub(crate) const RISK_CONTROL_SETTINGS_KEY: &str = "risk_control_settings";
pub(crate) const DEFAULT_RISK_CONTROL_ENABLED: bool = true;
pub(crate) const DEFAULT_RISK_MAX_SINGLE_LOSS_RATIO: f64 = 0.02;
pub(crate) const DEFAULT_RISK_STOP_LOSS_RATIO: f64 = 0.03;
pub(crate) const DEFAULT_RISK_MAX_TOTAL_POSITION_RATIO: f64 = 1.0;
pub(crate) const DEFAULT_RISK_MAX_POSITION_PCT: f64 = 0.2;
pub(crate) const DEFAULT_RISK_MAX_DAILY_LOSS_PCT: f64 = 0.05;
pub(crate) const DEFAULT_RISK_MAX_ORDER_VALUE: f64 = 0.0;

pub(crate) fn default_risk_control_config(mode: &str) -> Value {
    json!({
        "mode": mode,
        "enabled": DEFAULT_RISK_CONTROL_ENABLED,
        "max_single_loss_ratio": DEFAULT_RISK_MAX_SINGLE_LOSS_RATIO,
        "default_stop_loss_ratio": DEFAULT_RISK_STOP_LOSS_RATIO,
        "max_total_position_ratio": DEFAULT_RISK_MAX_TOTAL_POSITION_RATIO,
        "max_position_pct": DEFAULT_RISK_MAX_POSITION_PCT,
        "max_daily_loss_pct": DEFAULT_RISK_MAX_DAILY_LOSS_PCT,
        "max_order_value": DEFAULT_RISK_MAX_ORDER_VALUE
    })
}

pub(crate) fn risk_control_config_with_saved(
    mode: &str,
    saved: Option<&Map<String, Value>>,
) -> Value {
    let mut config = default_risk_control_config(mode);
    if let (Some(obj), Some(saved)) = (config.as_object_mut(), saved) {
        obj.extend(saved.clone());
        obj.insert("mode".to_string(), Value::String(mode.to_string()));
    }
    config
}

pub(crate) fn live_risk_control_values(config: &Value) -> (bool, f64, f64, f64) {
    (
        config
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(DEFAULT_RISK_CONTROL_ENABLED),
        config
            .get("max_single_loss_ratio")
            .and_then(Value::as_f64)
            .unwrap_or(DEFAULT_RISK_MAX_SINGLE_LOSS_RATIO),
        config
            .get("max_position_pct")
            .and_then(Value::as_f64)
            .unwrap_or(DEFAULT_RISK_MAX_POSITION_PCT),
        config
            .get("max_order_value")
            .and_then(Value::as_f64)
            .unwrap_or(DEFAULT_RISK_MAX_ORDER_VALUE),
    )
}

/// 从偏好存储中加载风控配置用于实盘策略
pub(crate) async fn load_risk_control_for_live(
    state: &AppState,
    mode: &str,
) -> (bool, f64, f64, f64) {
    let saved = state
        .preferences
        .get(RISK_CONTROL_SETTINGS_KEY)
        .await
        .ok()
        .flatten()
        .and_then(|value| {
            value
                .as_object()
                .and_then(|all_modes| all_modes.get(mode))
                .and_then(Value::as_object)
                .cloned()
        });
    let config = risk_control_config_with_saved(mode, saved.as_ref());
    live_risk_control_values(&config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn risk_control_config_merges_saved_values_over_defaults_and_preserves_mode() {
        let saved = json!({
            "mode": "stale",
            "enabled": false,
            "max_order_value": 2500.0
        });

        let config = risk_control_config_with_saved("live", saved.as_object());

        assert_eq!(config["mode"], "live");
        assert_eq!(config["enabled"], false);
        assert_eq!(config["max_order_value"], 2500.0);
        assert_eq!(
            config["max_single_loss_ratio"],
            DEFAULT_RISK_MAX_SINGLE_LOSS_RATIO
        );
    }

    #[test]
    fn live_risk_control_values_use_same_default_config_contract() {
        let config = default_risk_control_config("simulated");

        assert_eq!(
            live_risk_control_values(&config),
            (
                DEFAULT_RISK_CONTROL_ENABLED,
                DEFAULT_RISK_MAX_SINGLE_LOSS_RATIO,
                DEFAULT_RISK_MAX_POSITION_PCT,
                DEFAULT_RISK_MAX_ORDER_VALUE,
            )
        );
    }
}
