use serde_json::{json, Value};

pub(super) fn vec_to_values(values: &[f64]) -> Value {
    Value::Array(
        values
            .iter()
            .map(|value| {
                if value.is_finite() {
                    json!(round6(*value))
                } else {
                    Value::Null
                }
            })
            .collect(),
    )
}

fn round6(value: f64) -> f64 {
    if value.is_finite() {
        (value * 1_000_000.0).round() / 1_000_000.0
    } else {
        0.0
    }
}
