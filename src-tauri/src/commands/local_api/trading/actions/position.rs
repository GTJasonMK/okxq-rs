use serde_json::Value;

use super::{super::*, order::*};

/// POST /api/trading/contract/set-leverage — 设置杠杆倍数
pub(crate) async fn trading_set_leverage(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let inst_id = normalize_order_inst_id(&body_string(req, "inst_id", ""), "SWAP");
    let lever = body_string(req, "lever", "");
    let mgn_mode = body_string(req, "mgn_mode", "cross").trim().to_lowercase();
    let requested_pos_side = body_string(req, "pos_side", "");
    if inst_id.is_empty() || lever.is_empty() {
        return Err(AppError::Validation("inst_id/lever 为必填参数".to_string()));
    }
    let client = okx_private_client(state, &mode).await?;
    let inst_type = "SWAP";
    let account_config = if inst_type == "SWAP" && mgn_mode == "isolated" {
        Some(client.get_account_config().await?)
    } else {
        None
    };
    let pos_side = resolve_leverage_pos_side(
        inst_type,
        &mgn_mode,
        &requested_pos_side,
        account_config.as_ref(),
    )?;
    let result = client
        .set_leverage(&inst_id, &lever, &mgn_mode, &pos_side)
        .await?;
    Ok(code_ok(result))
}

/// POST /api/trading/contract/set-position-mode — 设置持仓模式
pub(crate) async fn trading_set_position_mode(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let pos_mode = body_string(req, "pos_mode", "");
    if pos_mode.is_empty() {
        return Err(AppError::Validation("pos_mode 为必填参数".to_string()));
    }
    let client = okx_private_client(state, &mode).await?;
    let result = client.set_position_mode(&pos_mode).await?;
    Ok(code_ok(result))
}

fn resolve_leverage_pos_side(
    inst_type: &str,
    mgn_mode: &str,
    requested: &str,
    account_config: Option<&Value>,
) -> AppResult<String> {
    if inst_type != "SWAP" || mgn_mode != "isolated" {
        return Ok(String::new());
    }

    let pos_mode = account_config
        .and_then(account_position_mode)
        .ok_or_else(|| {
            AppError::Validation(
                "无法从 OKX account config 读取 posMode，已拒绝设置逐仓杠杆以避免 posSide 参数错误"
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
                "当前 OKX 账户为双向持仓 long_short_mode，逐仓杠杆必须指定 pos_side=long/short"
                    .to_string(),
            )),
        };
    }

    Err(AppError::Validation(
        "无法从 OKX account config 读取 posMode，已拒绝设置逐仓杠杆以避免 posSide 参数错误"
            .to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn leverage_pos_side_is_omitted_for_cross_even_in_long_short_mode() {
        let config = json!({"posMode": "long_short_mode"});

        let pos_side = resolve_leverage_pos_side("SWAP", "cross", "short", Some(&config)).unwrap();

        assert_eq!(pos_side, "");
    }

    #[test]
    fn leverage_pos_side_is_required_for_isolated_long_short_mode() {
        let config = json!({"posMode": "long_short_mode"});

        let pos_side =
            resolve_leverage_pos_side("SWAP", "isolated", "short", Some(&config)).unwrap();

        assert_eq!(pos_side, "short");
    }

    #[test]
    fn leverage_pos_side_is_omitted_for_isolated_net_mode() {
        let config = json!({"pos_mode": "net_mode"});

        let pos_side =
            resolve_leverage_pos_side("SWAP", "isolated", "short", Some(&config)).unwrap();

        assert_eq!(pos_side, "");
    }
}
