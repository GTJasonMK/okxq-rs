use serde_json::Value;

use crate::{
    error::{AppError, AppResult},
    live_strategy::types::LiveStrategyConfig,
    okx::OkxPrivateClient,
    trading_semantics::{
        close_order_side_for_target_position_side, close_target_position_side,
        close_target_position_side_label, infer_single_close_target_position_side,
        is_contract_inst_type, position_side_from_okx_position, symbol_currencies,
    },
};

use super::{
    json_values::{json_finite_f64, json_positive_f64, json_text},
    td_mode_from_config,
};

#[derive(Clone, Debug)]
pub(super) struct ResolvedCloseOrder {
    pub(super) order_side: String,
    pub(super) quantity: f64,
}

#[derive(Clone, Debug)]
pub(super) struct ResolvedRiskCloseOrder {
    pub(super) order_side: String,
    pub(super) quantity: f64,
    pub(super) average_price: Option<f64>,
}

pub(super) async fn resolve_risk_close_exchange_order(
    private_client: &OkxPrivateClient,
    config: &LiveStrategyConfig,
    order_side: Option<&str>,
) -> AppResult<ResolvedRiskCloseOrder> {
    if !is_contract_inst_type(&config.inst_type) {
        let order_side = order_side
            .map(str::trim)
            .filter(|side| !side.is_empty())
            .unwrap_or("sell");
        let quantity = resolve_spot_close_quantity(private_client, config, order_side).await?;
        return Ok(ResolvedRiskCloseOrder {
            order_side: order_side.to_ascii_lowercase(),
            quantity,
            average_price: None,
        });
    }
    let positions = private_client
        .get_positions(Some(&config.inst_type), Some(&config.symbol))
        .await?;
    if let Some(order_side) = order_side.map(str::trim).filter(|side| !side.is_empty()) {
        return risk_close_order_from_positions(&positions, &config.symbol, order_side)?
            .ok_or_else(|| {
                AppError::Validation(format!(
                    "OKX 当前没有 {} 可平的 {} 持仓，已拒绝提交独立保护单",
                    config.symbol,
                    close_target_position_side_label(order_side)
                ))
            });
    }
    infer_risk_close_order_from_positions(&positions, &config.symbol)
}

fn risk_close_order_from_positions(
    positions: &[Value],
    symbol: &str,
    order_side: &str,
) -> AppResult<Option<ResolvedRiskCloseOrder>> {
    let target = close_target_position_side(order_side).ok_or_else(|| {
        AppError::Validation(format!(
            "无法根据保护单方向 {order_side} 推导 OKX 目标持仓方向"
        ))
    })?;
    let mut quantity = 0.0;
    let mut average_price_notional = 0.0;
    let mut average_price_quantity = 0.0;
    for item in positions {
        if !json_text(item, "instId").eq_ignore_ascii_case(symbol) {
            continue;
        }
        let pos = json_finite_f64(item, "pos").ok_or_else(|| {
            AppError::Runtime(format!(
                "OKX 持仓 {symbol} 缺少有效 pos，无法确认保护单数量"
            ))
        })?;
        if pos.abs() <= f64::EPSILON {
            continue;
        }
        if position_side_from_okx_position(&json_text(item, "posSide"), pos) != Some(target) {
            continue;
        }
        let available = json_finite_f64(item, "availPos").ok_or_else(|| {
            AppError::Runtime(format!(
                "OKX 持仓 {symbol} 缺少有效 availPos，无法确认保护单数量"
            ))
        })?;
        if available <= f64::EPSILON {
            continue;
        }
        let available = available.abs();
        quantity += available;
        if let Some(avg_px) = json_positive_f64(item, "avgPx") {
            average_price_notional += avg_px * available;
            average_price_quantity += available;
        }
    }
    Ok(
        (quantity.is_finite() && quantity > 0.0).then(|| ResolvedRiskCloseOrder {
            order_side: order_side.trim().to_ascii_lowercase(),
            quantity,
            average_price: (average_price_quantity > 0.0)
                .then_some(average_price_notional / average_price_quantity),
        }),
    )
}

fn infer_risk_close_order_from_positions(
    positions: &[Value],
    symbol: &str,
) -> AppResult<ResolvedRiskCloseOrder> {
    let long_close = risk_close_order_from_positions(positions, symbol, "sell")?;
    let short_close = risk_close_order_from_positions(positions, symbol, "buy")?;
    let target = infer_single_close_target_position_side(
        long_close.is_some(),
        short_close.is_some(),
        format!("OKX 当前没有 {symbol} 可平持仓，已拒绝提交独立保护单"),
        format!(
            "OKX 当前同时存在 {symbol} 多头和空头可平持仓，place_risk_order 必须显式提供 order_side/close_side"
        ),
    )?;
    match close_order_side_for_target_position_side(target) {
        "sell" => Ok(long_close.expect("long close is present after target inference")),
        "buy" => Ok(short_close.expect("short close is present after target inference")),
        _ => unreachable!("close target side maps only to buy or sell"),
    }
}

pub(super) async fn resolve_close_exchange_order(
    private_client: &OkxPrivateClient,
    config: &LiveStrategyConfig,
    order_side: Option<&str>,
) -> AppResult<ResolvedCloseOrder> {
    if let Some(order_side) = order_side.map(str::trim).filter(|side| !side.is_empty()) {
        let quantity = resolve_close_exchange_quantity(private_client, config, order_side).await?;
        return Ok(ResolvedCloseOrder {
            order_side: order_side.to_ascii_lowercase(),
            quantity,
        });
    }
    if !is_contract_inst_type(&config.inst_type) {
        let quantity = resolve_spot_close_quantity(private_client, config, "sell").await?;
        return Ok(ResolvedCloseOrder {
            order_side: "sell".to_string(),
            quantity,
        });
    }
    let positions = private_client
        .get_positions(Some(&config.inst_type), Some(&config.symbol))
        .await?;
    infer_close_order_from_positions(&positions, &config.symbol)
}

async fn resolve_close_exchange_quantity(
    private_client: &OkxPrivateClient,
    config: &LiveStrategyConfig,
    order_side: &str,
) -> AppResult<f64> {
    if !is_contract_inst_type(&config.inst_type) {
        return resolve_spot_close_quantity(private_client, config, order_side).await;
    }
    let positions = private_client
        .get_positions(Some(&config.inst_type), Some(&config.symbol))
        .await?;
    close_quantity_from_positions(&positions, &config.symbol, order_side)?.ok_or_else(|| {
        AppError::Validation(format!(
            "OKX 当前没有 {} 可平的 {} 持仓，已拒绝实时策略平仓",
            config.symbol,
            close_target_position_side_label(order_side)
        ))
    })
}

pub(super) async fn resolve_spot_close_quantity(
    private_client: &OkxPrivateClient,
    config: &LiveStrategyConfig,
    order_side: &str,
) -> AppResult<f64> {
    if !order_side.trim().eq_ignore_ascii_case("sell") {
        return Err(AppError::Validation(
            "现货实时策略平仓只支持 sell 当前基础币持仓".to_string(),
        ));
    }
    let (base_ccy, _) = symbol_currencies(&config.symbol);
    if base_ccy.is_empty() {
        return Err(AppError::Validation(format!(
            "无法从 symbol={} 推导现货基础币，已拒绝实时策略平仓",
            config.symbol
        )));
    }
    let td_mode = td_mode_from_config(config)?;
    let max_avail = private_client
        .get_max_avail_size(&config.symbol, &td_mode)
        .await?;
    spot_available_sell_size(&max_avail, &config.symbol).ok_or_else(|| {
        AppError::Validation(format!(
            "OKX 当前没有可卖出的 {base_ccy} 余额，已拒绝实时策略平仓"
        ))
    })
}

pub(super) fn close_quantity_from_positions(
    positions: &[Value],
    symbol: &str,
    order_side: &str,
) -> AppResult<Option<f64>> {
    let target = close_target_position_side(order_side).ok_or_else(|| {
        AppError::Validation(format!(
            "无法根据平仓订单方向 {order_side} 推导 OKX 目标持仓方向"
        ))
    })?;
    let mut quantity = 0.0;
    for item in positions {
        if !json_text(item, "instId").eq_ignore_ascii_case(symbol) {
            continue;
        }
        let pos = json_finite_f64(item, "pos").ok_or_else(|| {
            AppError::Runtime(format!("OKX 持仓 {symbol} 缺少有效 pos，无法确认可平数量"))
        })?;
        if pos.abs() <= f64::EPSILON {
            continue;
        }
        if position_side_from_okx_position(&json_text(item, "posSide"), pos) != Some(target) {
            continue;
        }
        let available = json_finite_f64(item, "availPos").ok_or_else(|| {
            AppError::Runtime(format!(
                "OKX 持仓 {symbol} 缺少有效 availPos，无法确认可平数量"
            ))
        })?;
        if available <= f64::EPSILON {
            continue;
        }
        quantity += available.abs();
    }
    Ok((quantity.is_finite() && quantity > 0.0).then_some(quantity))
}

pub(super) fn infer_close_order_from_positions(
    positions: &[Value],
    symbol: &str,
) -> AppResult<ResolvedCloseOrder> {
    let long_quantity = close_quantity_from_positions(positions, symbol, "sell")?.unwrap_or(0.0);
    let short_quantity = close_quantity_from_positions(positions, symbol, "buy")?.unwrap_or(0.0);
    let has_long = long_quantity.is_finite() && long_quantity > 0.0;
    let has_short = short_quantity.is_finite() && short_quantity > 0.0;
    let target = infer_single_close_target_position_side(
        has_long,
        has_short,
        format!("OKX 当前没有 {symbol} 可平持仓，已拒绝实时策略平仓"),
        format!(
            "OKX 当前同时存在 {symbol} 多头和空头可平持仓，策略 close_position 必须显式提供 order_side/close_side"
        ),
    )?;
    match close_order_side_for_target_position_side(target) {
        "sell" => Ok(ResolvedCloseOrder {
            order_side: "sell".to_string(),
            quantity: long_quantity,
        }),
        "buy" => Ok(ResolvedCloseOrder {
            order_side: "buy".to_string(),
            quantity: short_quantity,
        }),
        _ => unreachable!("close target side maps only to buy or sell"),
    }
}

fn spot_available_sell_size(item: &Value, symbol: &str) -> Option<f64> {
    let inst_id = json_text(item, "instId");
    if !inst_id.is_empty() && !inst_id.eq_ignore_ascii_case(symbol) {
        return None;
    }
    json_positive_f64(item, "availSell")
}
