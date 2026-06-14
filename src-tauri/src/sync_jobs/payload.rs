use anyhow::{anyhow, Result};
use serde_json::Value;

pub(super) fn required_string(value: &Value, key: &str) -> Result<String> {
    match field(value, key)? {
        Value::String(text) if !text.is_empty() => Ok(text.clone()),
        Value::String(_) => Err(anyhow!(
            "sync job completion field `{key}` must not be empty"
        )),
        _ => Err(anyhow!(
            "sync job completion field `{key}` must be a string"
        )),
    }
}

pub(super) fn optional_string(value: &Value, key: &str) -> Result<Option<String>> {
    match field(value, key)? {
        Value::Null => Ok(None),
        Value::String(text) if !text.is_empty() => Ok(Some(text.clone())),
        Value::String(_) => Err(anyhow!(
            "sync job completion field `{key}` must not be empty"
        )),
        _ => Err(anyhow!(
            "sync job completion field `{key}` must be a string or null"
        )),
    }
}

pub(super) fn required_i64(value: &Value, key: &str) -> Result<i64> {
    field(value, key)?
        .as_i64()
        .ok_or_else(|| anyhow!("sync job completion field `{key}` must be a signed integer"))
}

pub(super) fn optional_i64(value: &Value, key: &str) -> Result<Option<i64>> {
    match field(value, key)? {
        Value::Null => Ok(None),
        item => item.as_i64().map(Some).ok_or_else(|| {
            anyhow!("sync job completion field `{key}` must be a signed integer or null")
        }),
    }
}

pub(super) fn required_bool(value: &Value, key: &str) -> Result<bool> {
    field(value, key)?
        .as_bool()
        .ok_or_else(|| anyhow!("sync job completion field `{key}` must be a boolean"))
}

pub(super) fn required_string_array(value: &Value, key: &str) -> Result<Vec<String>> {
    let items = match field(value, key)? {
        Value::Array(items) => items,
        _ => {
            return Err(anyhow!(
                "sync job completion field `{key}` must be an array of strings"
            ))
        }
    };
    let mut values = Vec::with_capacity(items.len());
    for item in items {
        match item {
            Value::String(text) if !text.trim().is_empty() => values.push(text.trim().to_string()),
            Value::String(_) => {
                return Err(anyhow!(
                    "sync job completion field `{key}` must not contain empty strings"
                ))
            }
            _ => {
                return Err(anyhow!(
                    "sync job completion field `{key}` must contain only strings"
                ))
            }
        }
    }
    if values.is_empty() {
        return Err(anyhow!(
            "sync job completion field `{key}` must contain at least one value"
        ));
    }
    Ok(values)
}

fn field<'a>(value: &'a Value, key: &str) -> Result<&'a Value> {
    value
        .get(key)
        .ok_or_else(|| anyhow!("sync job completion payload missing `{key}`"))
}
