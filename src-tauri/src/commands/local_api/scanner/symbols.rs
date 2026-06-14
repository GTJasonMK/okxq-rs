use std::collections::BTreeSet;

use super::super::*;

fn normalize_scanner_inst_type(inst_type: &str) -> AppResult<String> {
    let normalized = inst_type.trim().to_uppercase();
    match normalized.as_str() {
        "SPOT" | "SWAP" => Ok(normalized),
        _ => Err(AppError::Validation(format!(
            "当前扫描器仅支持 SPOT/SWAP，收到 inst_type={normalized}"
        ))),
    }
}

pub(super) async fn resolve_scanner_inst_type(
    state: &AppState,
    inst_type: &str,
) -> AppResult<String> {
    let requested = inst_type.trim();
    if !requested.is_empty() {
        let normalized = normalize_scanner_inst_type(requested)?;
        if enabled_scanner_symbol_ids(state, &normalized)
            .await?
            .is_empty()
        {
            return Err(AppError::Validation(format!(
                "关注清单未启用 {normalized} 数据目标，已拒绝扫描"
            )));
        }
        return Ok(normalized);
    }

    let watched = state.preferences.watched_symbols().await?;
    let mut types = BTreeSet::new();
    for item in watched {
        if item.sync_spot {
            types.insert("SPOT".to_string());
        }
        if item.sync_swap {
            types.insert("SWAP".to_string());
        }
    }
    let mut types = types.into_iter();
    match (types.next(), types.next()) {
        (Some(inst_type), None) => Ok(inst_type),
        (None, _) => Err(AppError::Validation(
            "关注清单没有启用任何扫描数据目标".to_string(),
        )),
        (Some(_), Some(_)) => Err(AppError::Validation(
            "关注清单同时启用了现货和合约，请指定扫描 inst_type".to_string(),
        )),
    }
}

pub(super) async fn enabled_scanner_symbol_ids(
    state: &AppState,
    inst_type: &str,
) -> AppResult<Vec<String>> {
    let mut symbols = Vec::new();
    for item in state.preferences.watched_symbols().await? {
        match inst_type {
            "SPOT" if item.sync_spot => symbols.push(item.spot_inst_id),
            "SWAP" if item.sync_swap => symbols.push(item.swap_inst_id),
            _ => {}
        }
    }
    Ok(symbols)
}

pub(super) async fn normalize_requested_scanner_symbols(
    state: &AppState,
    symbols: &[String],
    inst_type: &str,
) -> AppResult<Vec<String>> {
    let enabled = enabled_scanner_symbol_ids(state, inst_type)
        .await?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut normalized = Vec::new();
    for symbol in symbols {
        let inst_id = normalize_market_inst_id(symbol, inst_type);
        if !enabled.contains(&inst_id) {
            return Err(AppError::Validation(format!(
                "{inst_id} {inst_type} 未在关注清单中启用，已拒绝扫描"
            )));
        }
        normalized.push(inst_id);
    }
    normalized.sort();
    normalized.dedup();
    Ok(normalized)
}
