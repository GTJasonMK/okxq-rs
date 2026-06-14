use serde_json::{json, Value};

use super::bars::FactorBar;

/// 构建单个因子的 payload JSON。
pub(super) fn factor_payload(
    inst_id: &str,
    factor_name: &str,
    value: f64,
    bars: &[FactorBar],
) -> Value {
    let last_close = bars.last().map(|bar| bar.3).unwrap_or(0.0);
    let last_volume = bars.last().map(|bar| bar.4).unwrap_or(0.0);
    json!({
        "inst_id": inst_id,
        "factor_name": factor_name,
        "value": round6(value),
        "last_close": round6(last_close),
        "last_volume": round6(last_volume),
        "bar_count": bars.len(),
        "computed_at": chrono::Utc::now().timestamp_millis() as f64 / 1000.0,
    })
}

fn round6(value: f64) -> f64 {
    if value.is_finite() {
        (value * 1_000_000.0).round() / 1_000_000.0
    } else {
        0.0
    }
}
