use super::*;
use crate::trading_semantics::{
    parse_positive_decimal_text, required_exchange_order_price_type,
    resolve_exchange_quantity_from_base_order_quantity,
    resolve_explicit_exchange_quantity as trade_resolve_explicit_exchange_quantity,
    validate_exchange_order_price_shape as validate_trade_order_price_shape,
    validate_explicit_exchange_size as validate_trade_explicit_exchange_size,
    validate_modify_order_has_change, validate_modify_order_price_field,
    validate_modify_order_size_field,
    validate_price_matches_tick_size as validate_trade_price_matches_tick_size,
    InstrumentTradeRules, ALGO_MODIFY_ORDER_FIELDS, EXCHANGE_MODIFY_ORDER_FIELDS,
};

pub(super) async fn resolve_explicit_exchange_order_size(
    client: &OkxPublicClient,
    config: &LiveStrategyConfig,
    exchange_size: &str,
    label: &str,
    consequence: &str,
) -> AppResult<f64> {
    let quantity = parse_positive_decimal_text(exchange_size, "exchange_size")?;
    let rules = fetch_instrument_order_rules(client, config).await?;
    validate_explicit_exchange_size(&rules, quantity, label, consequence)?;
    Ok(quantity)
}

pub(super) async fn validate_exchange_order_price(
    client: &OkxPublicClient,
    config: &LiveStrategyConfig,
    order_type: &str,
    order_side: &str,
    price: f64,
) -> AppResult<()> {
    let Some(order_type) = required_exchange_order_price_type(order_type, price, |order_type| {
        format!("OKX {order_type} 订单需要有效价格，当前策略价格无效")
    })?
    else {
        return Ok(());
    };
    let rules = fetch_instrument_order_rules(client, config).await?;
    validate_exchange_order_price_with_rules(&rules, order_type, order_side, price)
}

fn validate_exchange_order_price_with_rules(
    rules: &InstrumentOrderRules,
    order_type: &str,
    order_side: &str,
    price: f64,
) -> AppResult<()> {
    validate_trade_order_price_shape(
        &rules.to_trade_rules(),
        order_type,
        order_side,
        price,
        |order_type| format!("OKX {order_type} 订单需要有效价格，当前策略价格无效"),
        "已拒绝提交以避免静默改价",
    )
}

pub(super) async fn validate_exchange_modify_order(
    client: &OkxPublicClient,
    config: &LiveStrategyConfig,
    modify_order: &StrategyModifyOrderIntent,
) -> AppResult<()> {
    validate_modify_order_has_change(
        modify_order.new_size.as_deref(),
        modify_order.new_price.as_deref(),
        EXCHANGE_MODIFY_ORDER_FIELDS,
    )?;
    let rules = fetch_instrument_order_rules(client, config).await?;
    let trade_rules = rules.to_trade_rules();
    if let Some(new_size) = modify_order.new_size.as_deref() {
        validate_modify_order_size_field(&trade_rules, new_size, EXCHANGE_MODIFY_ORDER_FIELDS)?;
    }
    if let Some(new_price) = modify_order.new_price.as_deref() {
        validate_modify_order_price_field(&trade_rules, new_price, EXCHANGE_MODIFY_ORDER_FIELDS)?;
    }
    Ok(())
}

pub(super) async fn validate_exchange_algo_modify_order(
    client: &OkxPublicClient,
    config: &LiveStrategyConfig,
    modify_order: &StrategyModifyOrderIntent,
) -> AppResult<()> {
    validate_modify_order_has_change(
        modify_order.new_size.as_deref(),
        modify_order.new_price.as_deref(),
        ALGO_MODIFY_ORDER_FIELDS,
    )?;
    let rules = fetch_instrument_order_rules(client, config).await?;
    let trade_rules = rules.to_trade_rules();
    if let Some(new_size) = modify_order.new_size.as_deref() {
        validate_modify_order_size_field(&trade_rules, new_size, ALGO_MODIFY_ORDER_FIELDS)?;
    }
    if let Some(new_price) = modify_order.new_price.as_deref() {
        validate_modify_order_price_field(&trade_rules, new_price, ALGO_MODIFY_ORDER_FIELDS)?;
    }
    Ok(())
}

pub(super) fn validate_price_matches_tick_size(
    rules: &InstrumentOrderRules,
    price: f64,
    label: &str,
    consequence: &str,
) -> AppResult<()> {
    validate_trade_price_matches_tick_size(&rules.to_trade_rules(), price, label, consequence)
}

pub(super) fn validate_explicit_exchange_size(
    rules: &InstrumentOrderRules,
    size: f64,
    label: &str,
    consequence: &str,
) -> AppResult<()> {
    validate_trade_explicit_exchange_size(&rules.to_trade_rules(), size, label, consequence)
}

pub(super) async fn fetch_instrument_order_rules(
    client: &OkxPublicClient,
    config: &LiveStrategyConfig,
) -> AppResult<InstrumentOrderRules> {
    let inst_type = config.inst_type.trim();
    let symbol = config.symbol.trim().to_ascii_uppercase();
    if inst_type.is_empty() || symbol.is_empty() {
        return Err(AppError::Validation(
            "实时策略缺少 inst_type 或 symbol，无法校验交易规格".to_string(),
        ));
    }
    let instruments = client.get_instruments(inst_type).await?;
    let Some(item) = instruments
        .into_iter()
        .find(|item| json_text(item, "instId").eq_ignore_ascii_case(&symbol))
    else {
        return Err(AppError::Validation(format!(
            "OKX instruments 未返回 {symbol}，无法校验下单数量"
        )));
    };
    let rules = InstrumentOrderRules {
        inst_id: symbol,
        state: json_text(&item, "state"),
        min_sz: json_positive_f64(&item, "minSz"),
        lot_sz: json_positive_f64(&item, "lotSz"),
        tick_sz: json_positive_f64(&item, "tickSz"),
        ct_val: json_positive_f64(&item, "ctVal"),
        ct_val_ccy: json_text(&item, "ctValCcy").to_ascii_uppercase(),
    };
    if !rules.state.trim().is_empty() && !rules.state.eq_ignore_ascii_case("live") {
        return Err(AppError::Validation(format!(
            "OKX instrument {} 当前状态为 {}，已拒绝实时策略下单",
            rules.inst_id, rules.state
        )));
    }
    Ok(rules)
}

#[derive(Clone, Debug)]
pub(super) struct InstrumentOrderRules {
    pub(super) inst_id: String,
    pub(super) state: String,
    pub(super) min_sz: Option<f64>,
    pub(super) lot_sz: Option<f64>,
    pub(super) tick_sz: Option<f64>,
    pub(super) ct_val: Option<f64>,
    pub(super) ct_val_ccy: String,
}

impl InstrumentOrderRules {
    fn to_trade_rules(&self) -> InstrumentTradeRules {
        InstrumentTradeRules {
            inst_id: self.inst_id.clone(),
            min_sz: self.min_sz,
            lot_sz: self.lot_sz,
            tick_sz: self.tick_sz,
            ct_val: self.ct_val,
            ct_val_ccy: self.ct_val_ccy.clone(),
        }
    }
}

pub(super) fn resolve_explicit_entry_exchange_quantity(
    config: &LiveStrategyConfig,
    rules: &InstrumentOrderRules,
    raw_exchange_size: &str,
    price: f64,
) -> AppResult<crate::trading_semantics::ResolvedExchangeQuantity> {
    trade_resolve_explicit_exchange_quantity(
        &rules.to_trade_rules(),
        &config.inst_type,
        &config.symbol,
        raw_exchange_size,
        price,
        "策略显式 exchange_size",
        "已拒绝下单以避免静默改量",
        "策略显式 exchange_size 必须是有效正数",
        "实时策略开仓价格无效，无法估算显式 exchange_size 的风险敞口",
    )
}

pub(super) fn resolve_entry_exchange_quantity_from_base(
    config: &LiveStrategyConfig,
    rules: &InstrumentOrderRules,
    base_quantity: f64,
    price: f64,
) -> AppResult<crate::trading_semantics::ResolvedExchangeQuantity> {
    resolve_exchange_quantity_from_base_order_quantity(
        &rules.to_trade_rules(),
        &config.inst_type,
        &config.symbol,
        base_quantity,
        price,
        "实时策略开仓数量无效",
        "实时策略开仓价格无效，无法把策略数量换算为合约张数",
        "实时策略换算后的 OKX 下单数量无效",
    )
}
