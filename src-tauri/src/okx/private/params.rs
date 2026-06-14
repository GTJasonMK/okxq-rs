use crate::error::{AppError, AppResult};

pub(super) fn optional_inst_filter_params(
    inst_type: Option<&str>,
    inst_id: Option<&str>,
) -> Vec<(&'static str, String)> {
    let mut params = Vec::new();
    push_optional_uppercase_param(&mut params, "instType", inst_type);
    push_optional_uppercase_param(&mut params, "instId", inst_id);
    params
}

pub(super) fn push_optional_uppercase_param(
    params: &mut Vec<(&'static str, String)>,
    key: &'static str,
    value: Option<&str>,
) {
    if let Some(value) = value.map(str::trim).filter(|item| !item.is_empty()) {
        params.push((key, value.to_uppercase()));
    }
}

pub(super) fn normalized_required_inst_id(inst_id: &str, operation: &str) -> AppResult<String> {
    let normalized = inst_id.trim().to_uppercase();
    if normalized.is_empty() {
        return Err(AppError::Validation(format!(
            "OKX {operation}参数 instId 不能为空"
        )));
    }
    Ok(normalized)
}
