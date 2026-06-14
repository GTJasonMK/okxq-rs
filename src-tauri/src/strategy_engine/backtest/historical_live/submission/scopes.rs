use crate::{
    strategy_execution_contract::{StrategyExecutionIntent, StrategyRiskOrderIntent},
    trading_semantics::{
        close_order_side_for_target_position_side, infer_single_close_target_position_side,
        is_contract_inst_type,
    },
};

use super::super::{
    state::{SimOrderQuantity, SimPosition},
    values::{close_target_position_side, norm_symbol},
    HistoricalLiveBacktest,
};

impl HistoricalLiveBacktest {
    pub(super) fn risk_close_scope(
        &self,
        intent: &StrategyExecutionIntent,
        risk: &StrategyRiskOrderIntent,
    ) -> Result<(String, String, SimOrderQuantity, Option<f64>), String> {
        let requested_side = intent
            .order_side
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .or_else(|| {
                let side = risk.side.trim();
                (!side.is_empty()).then_some(side)
            });
        if let Some(side) = requested_side {
            let side = side.to_ascii_lowercase();
            let pos_side = close_target_position_side(&side)
                .ok_or_else(|| format!("无法根据保护单方向 {side} 推导目标持仓"))?
                .to_string();
            let position = self
                .positions
                .get(&(norm_symbol(&intent.symbol), pos_side.clone()))
                .filter(|position| position.quantity > 0.0 && position.exchange_quantity > 0.0)
                .ok_or_else(|| format!("没有 {} 可保护的 {} 持仓", intent.symbol, pos_side))?;
            let quantity = self.close_quantity_from_position(intent, position)?;
            return Ok((
                side,
                pos_side,
                quantity,
                standalone_risk_reference_price(position),
            ));
        }
        let has_long = self.has_closeable_position(&intent.symbol, "long", true);
        let has_short = self.has_closeable_position(&intent.symbol, "short", true);
        let target = infer_single_close_target_position_side(
            has_long,
            has_short,
            format!("没有 {} 可保护持仓", intent.symbol),
            format!(
                "{} 同时存在多空持仓，place_risk_order 必须指定 order_side/close_side",
                intent.symbol
            ),
        )
        .map_err(|error| error.to_string())?;
        let pos_side = target.as_str().to_string();
        let position = self
            .positions
            .get(&(norm_symbol(&intent.symbol), pos_side.clone()))
            .ok_or_else(|| format!("没有 {} 可保护持仓", intent.symbol))?;
        let side = close_order_side_for_target_position_side(target).to_string();
        let quantity = self.close_quantity_from_position(intent, position)?;
        Ok((
            side,
            pos_side,
            quantity,
            standalone_risk_reference_price(position),
        ))
    }

    pub(super) fn close_scope(
        &self,
        intent: &StrategyExecutionIntent,
    ) -> Result<(String, String, SimOrderQuantity), String> {
        if let Some(side) = intent
            .order_side
            .as_deref()
            .map(str::trim)
            .filter(|side| !side.is_empty())
        {
            let side = side.to_ascii_lowercase();
            let pos_side = close_target_position_side(&side)
                .ok_or_else(|| format!("无法根据平仓订单方向 {side} 推导目标持仓"))?
                .to_string();
            let position = self
                .positions
                .get(&(norm_symbol(&intent.symbol), pos_side.clone()))
                .filter(|position| position.quantity > 0.0 && position.exchange_quantity > 0.0)
                .ok_or_else(|| format!("没有 {} 可平的 {} 持仓", intent.symbol, pos_side))?;
            let quantity = self.close_quantity_from_position(intent, position)?;
            return Ok((side, pos_side, quantity));
        }
        let has_long = self.has_closeable_position(&intent.symbol, "long", false);
        let has_short = self.has_closeable_position(&intent.symbol, "short", false);
        let target = infer_single_close_target_position_side(
            has_long,
            has_short,
            format!("没有 {} 可平持仓", intent.symbol),
            format!(
                "{} 同时存在多空持仓，close_position 必须指定 order_side",
                intent.symbol
            ),
        )
        .map_err(|error| error.to_string())?;
        let pos_side = target.as_str().to_string();
        let position = self
            .positions
            .get(&(norm_symbol(&intent.symbol), pos_side.clone()))
            .ok_or_else(|| format!("没有 {} 可平持仓", intent.symbol))?;
        let quantity = self.close_quantity_from_position(intent, position)?;
        Ok((
            close_order_side_for_target_position_side(target).to_string(),
            pos_side,
            quantity,
        ))
    }

    fn has_closeable_position(
        &self,
        symbol: &str,
        pos_side: &str,
        require_exchange_quantity: bool,
    ) -> bool {
        self.positions
            .get(&(norm_symbol(symbol), pos_side.to_string()))
            .is_some_and(|position| {
                position.quantity > 0.0
                    && (!require_exchange_quantity || position.exchange_quantity > 0.0)
            })
    }
}

fn standalone_risk_reference_price(position: &SimPosition) -> Option<f64> {
    (is_contract_inst_type(&position.inst_type)
        && position.entry_price.is_finite()
        && position.entry_price > 0.0)
        .then_some(position.entry_price)
}
