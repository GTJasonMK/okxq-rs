use std::collections::BTreeSet;

use crate::{
    app_state::AppState,
    config::WatchedSymbolRecord,
    error::{AppError, AppResult},
    instrument::infer_spot_swap_inst_type,
};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct MarketScope {
    pub(crate) symbol: String,
    pub(crate) base_ccy: String,
    pub(crate) inst_id: String,
    pub(crate) inst_type: String,
}

pub(crate) fn normalize_symbol(value: &str) -> Option<String> {
    let mut normalized = value.trim().to_uppercase();
    if normalized.is_empty() {
        return None;
    }
    if normalized.ends_with("-SWAP") {
        normalized.truncate(normalized.len() - 5);
    }
    if !normalized.contains('-') {
        normalized = format!("{normalized}-USDT");
    }
    Some(normalized)
}

pub(crate) fn normalize_market_inst_id(value: &str, inst_type: &str) -> String {
    let mut normalized = normalize_symbol(value).unwrap_or_else(|| value.trim().to_uppercase());
    if inst_type.trim().eq_ignore_ascii_case("SWAP") && !normalized.ends_with("-SWAP") {
        normalized = format!("{normalized}-SWAP");
    }
    if inst_type.trim().eq_ignore_ascii_case("SPOT") && normalized.ends_with("-SWAP") {
        normalized.truncate(normalized.len() - 5);
    }
    normalized
}

pub(crate) fn normalize_market_inst_type(
    value: &str,
    reference_inst_id: &str,
) -> AppResult<String> {
    let requested = value.trim().to_uppercase();
    let normalized = if requested.is_empty() {
        infer_inst_type(reference_inst_id)
    } else {
        requested
    };
    match normalized.as_str() {
        "SPOT" | "SWAP" => Ok(normalized),
        _ => Err(AppError::Validation(format!(
            "当前仅支持 SPOT/SWAP，收到 inst_type={normalized}"
        ))),
    }
}

pub(crate) async fn enabled_market_instruments(
    state: &AppState,
    inst_type: &str,
) -> AppResult<BTreeSet<String>> {
    let normalized_type = normalize_market_inst_type(inst_type, "")?;
    let mut instruments = BTreeSet::new();
    for item in state.preferences.watched_symbols().await? {
        match normalized_type.as_str() {
            "SPOT" if item.sync_spot => {
                instruments.insert(item.spot_inst_id.trim().to_uppercase());
            }
            "SWAP" if item.sync_swap => {
                instruments.insert(item.swap_inst_id.trim().to_uppercase());
            }
            _ => {}
        }
    }
    Ok(instruments)
}

pub(crate) fn enabled_scopes_from_watched(items: &[WatchedSymbolRecord]) -> Vec<MarketScope> {
    let mut scopes = Vec::new();
    for item in items {
        if item.sync_spot {
            scopes.push(MarketScope {
                symbol: item.symbol.clone(),
                base_ccy: item.base_ccy.clone(),
                inst_id: item.spot_inst_id.trim().to_uppercase(),
                inst_type: "SPOT".to_string(),
            });
        }
        if item.sync_swap {
            scopes.push(MarketScope {
                symbol: item.symbol.clone(),
                base_ccy: item.base_ccy.clone(),
                inst_id: item.swap_inst_id.trim().to_uppercase(),
                inst_type: "SWAP".to_string(),
            });
        }
    }
    scopes.sort();
    scopes.dedup();
    scopes
}

pub(crate) async fn enabled_market_scopes(state: &AppState) -> AppResult<Vec<MarketScope>> {
    let watched = state.preferences.watched_symbols().await?;
    Ok(enabled_scopes_from_watched(&watched))
}

pub(crate) fn scope_key(inst_id: &str, inst_type: &str) -> (String, String) {
    (
        inst_id.trim().to_uppercase(),
        inst_type.trim().to_uppercase(),
    )
}

pub(crate) fn scope_keys(scopes: &[MarketScope]) -> BTreeSet<(String, String)> {
    scopes
        .iter()
        .map(|scope| scope_key(&scope.inst_id, &scope.inst_type))
        .collect()
}

pub(crate) async fn resolve_watched_market_inst(
    state: &AppState,
    inst_id: &str,
    inst_type: &str,
) -> AppResult<(String, String)> {
    let normalized_type = normalize_market_inst_type(inst_type, inst_id)?;
    let normalized_id = normalize_market_inst_id(inst_id, &normalized_type);
    let enabled = enabled_market_instruments(state, &normalized_type).await?;
    if enabled.contains(&normalized_id) {
        return Ok((normalized_id, normalized_type));
    }
    Err(AppError::Validation(format!(
        "{normalized_id} {normalized_type} 未在关注清单中启用，已拒绝读取或请求行情数据"
    )))
}

pub(crate) async fn resolve_enabled_market_scope(
    state: &AppState,
    inst_id: &str,
    inst_type: &str,
) -> AppResult<(String, String)> {
    let raw_inst_id = inst_id.trim().to_uppercase();
    let requested_type = inst_type.trim().to_uppercase();
    if !raw_inst_id.is_empty() && (!requested_type.is_empty() || raw_inst_id.ends_with("-SWAP")) {
        return resolve_watched_market_inst(state, &raw_inst_id, &requested_type).await;
    }

    let scopes = enabled_market_scopes(state).await?;
    if scopes.is_empty() {
        return Err(AppError::Validation(
            "关注清单没有启用任何数据目标".to_string(),
        ));
    }

    let candidates = if raw_inst_id.is_empty() {
        scopes
    } else {
        let normalized_symbol = normalize_symbol(&raw_inst_id)
            .ok_or_else(|| AppError::Validation("无效交易对".to_string()))?;
        scopes
            .into_iter()
            .filter(|scope| scope.symbol.eq_ignore_ascii_case(&normalized_symbol))
            .collect::<Vec<_>>()
    };

    match candidates.as_slice() {
        [scope] => Ok((scope.inst_id.clone(), scope.inst_type.clone())),
        [] => {
            let label = if raw_inst_id.is_empty() {
                "默认交易对".to_string()
            } else {
                raw_inst_id
            };
            Err(AppError::Validation(format!(
                "{label} 未在关注清单中启用，已拒绝读取或请求行情数据"
            )))
        }
        _ => {
            let label = if raw_inst_id.is_empty() {
                "关注清单".to_string()
            } else {
                normalize_symbol(&raw_inst_id).unwrap_or(raw_inst_id)
            };
            Err(AppError::Validation(format!(
                "{label} 同时启用了现货和合约，请指定 inst_type"
            )))
        }
    }
}

pub(crate) fn infer_inst_type(inst_id: &str) -> String {
    infer_spot_swap_inst_type(inst_id).to_string()
}

pub(crate) fn symbol_parts(symbol: &str) -> Option<(String, String, String, String)> {
    let normalized = normalize_symbol(symbol)?;
    let base = normalized
        .split('-')
        .next()
        .unwrap_or(&normalized)
        .to_string();
    Some((
        normalized.clone(),
        normalized.clone(),
        format!("{normalized}-SWAP"),
        base,
    ))
}
