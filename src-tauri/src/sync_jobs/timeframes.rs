use anyhow::{anyhow, Result};

use crate::timeframes::{normalize_okx_timeframe, sort_okx_timeframes};

pub(super) fn normalize_target_timeframes(
    values: Vec<String>,
    default_timeframe: &str,
) -> Result<Vec<String>> {
    let default_timeframe = normalize_required_timeframe(default_timeframe, "timeframe")?;
    let mut values = values
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .map(|value| normalize_required_timeframe(&value, "target_timeframes"))
        .collect::<Result<Vec<_>>>()?;
    if values.is_empty() {
        values.push(default_timeframe);
    }
    sort_timeframes(&mut values);
    Ok(values)
}

pub(super) fn target_timeframes_json(values: &[String]) -> Result<String> {
    let mut values = values
        .iter()
        .filter(|value| !value.trim().is_empty())
        .map(|value| normalize_required_timeframe(value, "target_timeframes"))
        .collect::<Result<Vec<_>>>()?;
    sort_timeframes(&mut values);
    Ok(serde_json::to_string(&values)?)
}

pub(super) fn normalize_required_timeframe(value: &str, field: &str) -> Result<String> {
    normalize_timeframe(value)
        .ok_or_else(|| anyhow!("invalid sync timeframe for {field}: {}", value.trim()))
}

fn normalize_timeframe(value: &str) -> Option<String> {
    normalize_okx_timeframe(value).map(ToOwned::to_owned)
}

fn sort_timeframes(values: &mut Vec<String>) {
    sort_okx_timeframes(values);
}
