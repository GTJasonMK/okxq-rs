pub(in crate::commands::local_api::trading) fn finite_text_f64(value: &str) -> Option<f64> {
    let parsed = value.parse::<f64>().ok()?;
    parsed.is_finite().then_some(parsed)
}
