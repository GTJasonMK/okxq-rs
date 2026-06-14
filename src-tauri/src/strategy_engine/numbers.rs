pub(super) fn finite_or(value: f64, default_value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        default_value
    }
}

pub(super) fn round6(value: f64) -> f64 {
    if value.is_finite() {
        (value * 1_000_000.0).round() / 1_000_000.0
    } else {
        0.0
    }
}

pub(super) fn round8(value: f64) -> f64 {
    if value.is_finite() {
        (value * 100_000_000.0).round() / 100_000_000.0
    } else {
        0.0
    }
}
