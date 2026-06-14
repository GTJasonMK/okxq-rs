use serde_json::Value;

use super::super::super::*;

pub(super) fn normalize_order_inst_type(requested: &str, inst_id: &str) -> String {
    let requested = requested.trim().to_uppercase();
    if requested == "SWAP" || inst_id.trim().to_uppercase().ends_with("-SWAP") {
        "SWAP".to_string()
    } else {
        "SPOT".to_string()
    }
}

pub(in crate::commands::local_api::trading::actions) fn normalize_order_inst_id(
    inst_id: &str,
    inst_type: &str,
) -> String {
    let mut normalized = inst_id.trim().to_uppercase();
    if inst_type == "SWAP" && !normalized.is_empty() && !normalized.ends_with("-SWAP") {
        normalized.push_str("-SWAP");
    }
    if inst_type == "SPOT" && normalized.ends_with("-SWAP") {
        normalized.truncate(normalized.len() - "-SWAP".len());
    }
    normalized
}

pub(super) fn resolve_order_td_mode(inst_type: &str, requested: &str) -> String {
    let normalized = requested.trim().to_lowercase();
    if !normalized.is_empty() {
        return normalized;
    }
    if inst_type == "SWAP" {
        "cross".to_string()
    } else {
        "cash".to_string()
    }
}

pub(super) fn resolve_order_pos_side(
    inst_type: &str,
    requested: &str,
    account_config: Option<&Value>,
) -> AppResult<String> {
    if inst_type != "SWAP" {
        return Ok(String::new());
    }

    let pos_mode = account_config
        .and_then(account_position_mode)
        .ok_or_else(|| {
            AppError::Validation(
                "无法从 OKX account config 读取 posMode，已拒绝合约下单以避免 posSide 参数错误"
                    .to_string(),
            )
        })?;
    if pos_mode == "net_mode" {
        return Ok(String::new());
    }

    let normalized = requested.trim().to_lowercase();
    if pos_mode == "long_short_mode" {
        return match normalized.as_str() {
            "long" | "short" => Ok(normalized),
            _ => Err(AppError::Validation(
                "当前 OKX 账户为双向持仓 long_short_mode，合约下单必须指定 pos_side=long/short"
                    .to_string(),
            )),
        };
    }

    Err(AppError::Validation(
        "无法从 OKX account config 读取 posMode，已拒绝合约下单以避免 posSide 参数错误".to_string(),
    ))
}

pub(in crate::commands::local_api::trading::actions) fn account_position_mode(
    config: &Value,
) -> Option<&str> {
    config
        .get("posMode")
        .or_else(|| config.get("pos_mode"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}
