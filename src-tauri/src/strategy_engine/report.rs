use serde_json::{json, Value};

use crate::okx::OkxCandle;

use super::{
    numbers::{round6, round8},
    stats::{average, sharpe, sortino},
    types::{BacktestReport, StrategyConfig, TradeRecord},
};

pub(super) fn build_backtest_report(
    config: &StrategyConfig,
    candles: &[OkxCandle],
    days: i64,
    initial_capital: f64,
    final_capital: f64,
    max_drawdown: f64,
    total_commission: f64,
    equity_returns: Vec<f64>,
    trades: Vec<TradeRecord>,
    equity_curve: Vec<Value>,
    extra_detail: Value,
) -> BacktestReport {
    let total_return = if initial_capital > 0.0 {
        (final_capital / initial_capital - 1.0) * 100.0
    } else {
        0.0
    };
    let annual_return = if days > 0 && initial_capital > 0.0 && final_capital > 0.0 {
        ((final_capital / initial_capital).powf(365.0 / days as f64) - 1.0) * 100.0
    } else {
        total_return
    };

    let closed_pnls = trades
        .iter()
        .filter_map(|trade| trade.pnl)
        .collect::<Vec<_>>();
    let winning_trades = closed_pnls.iter().filter(|pnl| **pnl > 0.0).count() as i64;
    let losing_trades = closed_pnls.iter().filter(|pnl| **pnl < 0.0).count() as i64;
    let gross_profit = closed_pnls.iter().filter(|pnl| **pnl > 0.0).sum::<f64>();
    let gross_loss = closed_pnls
        .iter()
        .filter(|pnl| **pnl < 0.0)
        .map(|pnl| pnl.abs())
        .sum::<f64>();
    let win_rate = if !closed_pnls.is_empty() {
        winning_trades as f64 / closed_pnls.len() as f64 * 100.0
    } else {
        0.0
    };
    let profit_factor = if gross_loss > 0.0 {
        gross_profit / gross_loss
    } else if gross_profit > 0.0 {
        gross_profit
    } else {
        0.0
    };
    let avg_profit = average(closed_pnls.iter().copied().filter(|pnl| *pnl > 0.0));
    let avg_loss = average(closed_pnls.iter().copied().filter(|pnl| *pnl < 0.0));
    let largest_profit = closed_pnls.iter().copied().fold(0.0_f64, f64::max);
    let largest_loss = closed_pnls.iter().copied().fold(0.0_f64, f64::min);
    let sharpe_ratio = sharpe(&equity_returns);
    let sortino_ratio = sortino(&equity_returns);
    let calmar_ratio = if max_drawdown > 0.0 {
        annual_return / max_drawdown
    } else {
        0.0
    };
    let omega_ratio = {
        let pos_sum = equity_returns.iter().filter(|r| **r > 0.0).sum::<f64>();
        let neg_sum = equity_returns
            .iter()
            .filter(|r| **r < 0.0)
            .map(|r| r.abs())
            .sum::<f64>();
        if neg_sum > 0.0 {
            pos_sum / neg_sum
        } else if pos_sum > 0.0 {
            f64::INFINITY
        } else {
            0.0
        }
    };

    let sample_step = sample_step(candles.len(), 900);
    let sampled_indices = sample_indices(candles.len(), sample_step);
    let sampled_candles = sampled_indices
        .iter()
        .filter_map(|index| candles.get(*index))
        .map(candle_json)
        .collect::<Vec<_>>();
    let full_equity = compact_equity_curve(equity_curve);
    let equity_points_total = full_equity.len();
    let trade_cashflows = trade_cashflow_values(&trades);
    let (trades_json, trade_events_total, trades_truncated) = trade_detail_values(&trades);

    let start_time = candles
        .first()
        .map(|item| timestamp_to_text(item.timestamp))
        .unwrap_or_default();
    let end_time = candles
        .last()
        .map(|item| timestamp_to_text(item.timestamp))
        .unwrap_or_default();
    let created_at = chrono::Utc::now().to_rfc3339();

    let mut detail = json!({
        "candles": sampled_candles,
        "equity_curve": full_equity,
        "equity_curve_sampled": false,
        "equity_points_total": equity_points_total,
        "trades": trades_json,
        "trade_cashflows": trade_cashflows,
        "trade_cashflows_total": trade_events_total,
        "trade_events_total": trade_events_total,
        "trades_truncated": trades_truncated,
        "indicators": {}
    });
    if let (Some(base), Some(extra)) = (detail.as_object_mut(), extra_detail.as_object()) {
        for (key, value) in extra {
            base.insert(key.clone(), value.clone());
        }
    }

    BacktestReport {
        strategy_name: config.strategy_name.clone(),
        strategy_id: config.strategy_id.clone(),
        symbol: config.symbol.clone(),
        inst_type: config.inst_type.clone(),
        timeframe: config.timeframe.clone(),
        days,
        start_time,
        end_time,
        initial_capital: round6(initial_capital),
        final_capital: round6(final_capital),
        total_return: round6(total_return),
        annual_return: round6(annual_return),
        max_drawdown: round6(max_drawdown),
        sharpe_ratio: round6(sharpe_ratio),
        sortino_ratio: round6(sortino_ratio),
        calmar_ratio: round6(calmar_ratio),
        omega_ratio: round6(omega_ratio),
        win_rate: round6(win_rate),
        profit_factor: round6(profit_factor),
        total_trades: closed_pnls.len() as i64,
        winning_trades,
        losing_trades,
        avg_profit: round6(avg_profit),
        avg_loss: round6(avg_loss),
        largest_profit: round6(largest_profit),
        largest_loss: round6(largest_loss),
        total_commission: round6(total_commission),
        params: config.params.clone(),
        detail,
        sample_step,
        created_at,
    }
}

fn sample_step(len: usize, max_points: usize) -> usize {
    if len <= max_points || max_points == 0 {
        1
    } else {
        len.div_ceil(max_points)
    }
}

fn sample_indices(len: usize, step: usize) -> Vec<usize> {
    if len == 0 {
        return Vec::new();
    }
    let step = step.max(1);
    let mut indices = (0..len).step_by(step).collect::<Vec<_>>();
    if indices.last().copied() != Some(len - 1) {
        indices.push(len - 1);
    }
    indices
}

fn candle_json(candle: &OkxCandle) -> Value {
    json!({
        "timestamp": candle.timestamp,
        "open": candle.open,
        "high": candle.high,
        "low": candle.low,
        "close": candle.close,
        "volume": candle.volume
    })
}

fn compact_equity_curve(equity_curve: Vec<Value>) -> Vec<Value> {
    let mut compacted = Vec::with_capacity(equity_curve.len());
    for point in equity_curve {
        let compact = compact_equity_point(point);
        if let (Some(last_ts), Some(current_ts)) = (
            compacted
                .last()
                .and_then(|last: &Value| last.get("timestamp"))
                .and_then(Value::as_i64),
            compact.get("timestamp").and_then(Value::as_i64),
        ) {
            if last_ts == current_ts {
                if let Some(last) = compacted.last_mut() {
                    *last = compact;
                }
                continue;
            }
        }
        compacted.push(compact);
    }
    compacted
}

fn compact_equity_point(point: Value) -> Value {
    let Some(source) = point.as_object() else {
        return point;
    };
    let mut output = serde_json::Map::new();
    for key in [
        "timestamp",
        "equity",
        "cash",
        "position_value",
        "position_notional",
        "unrealized_pnl",
        "position_side",
        "leverage",
    ] {
        if let Some(value) = source.get(key) {
            output.insert(key.to_string(), value.clone());
        }
    }
    if !output.contains_key("position_notional") {
        if let Some(value) = output.get("position_value").cloned() {
            output.insert("position_notional".to_string(), value);
        }
    }
    if !output.contains_key("position_side") {
        let side = output
            .get("position_value")
            .and_then(Value::as_f64)
            .filter(|value| *value > 0.0)
            .map(|_| "long")
            .unwrap_or("flat");
        output.insert("position_side".to_string(), json!(side));
    }
    if !output.contains_key("leverage") {
        output.insert("leverage".to_string(), json!(1.0));
    }
    if let Some(positions) = source.get("positions") {
        let (compact_positions, position_count) = compact_equity_positions(positions);
        output.insert("position_count".to_string(), json!(position_count));
        if position_count > 0 {
            output.insert("positions".to_string(), compact_positions);
        }
    }
    Value::Object(output)
}

fn compact_equity_positions(positions: &Value) -> (Value, usize) {
    if let Some(rows) = positions.as_array() {
        let mut output = Vec::new();
        for (index, value) in rows.iter().enumerate() {
            if let Some((_key, position)) = compact_equity_position(index.to_string(), value) {
                output.push(Value::Object(position));
            }
        }
        let len = output.len();
        return (Value::Array(output), len);
    }
    if let Some(rows) = positions.as_object() {
        let mut output = serde_json::Map::new();
        for (symbol, value) in rows {
            if let Some((key, position)) = compact_equity_position(symbol.clone(), value) {
                output.insert(key, Value::Object(position));
            }
        }
        let len = output.len();
        return (Value::Object(output), len);
    }
    (Value::Null, 0)
}

fn compact_equity_position(
    fallback_key: String,
    value: &Value,
) -> Option<(String, serde_json::Map<String, Value>)> {
    let source = value.as_object()?;
    let mut position = serde_json::Map::new();
    let symbol_value = source
        .get("symbol")
        .or_else(|| source.get("instId"))
        .cloned()
        .unwrap_or_else(|| json!(fallback_key.clone()));
    let symbol = symbol_value.as_str().unwrap_or(&fallback_key).to_string();
    position.insert("symbol".to_string(), symbol_value);
    insert_first(&mut position, source, "side", &["side", "posSide"]);
    insert_first(
        &mut position,
        source,
        "inst_type",
        &["inst_type", "instType"],
    );
    insert_first(&mut position, source, "timeframe", &["timeframe"]);
    insert_first(
        &mut position,
        source,
        "entry_price",
        &["entry_price", "avgPx"],
    );
    insert_first(
        &mut position,
        source,
        "quantity",
        &[
            "base_quantity",
            "base_size",
            "basePos",
            "base_position_size",
            "quantity",
            "pos",
        ],
    );
    insert_first(
        &mut position,
        source,
        "exchange_quantity",
        &["exchange_quantity", "pos", "quantity"],
    );
    insert_first(
        &mut position,
        source,
        "entry_timestamp",
        &["entry_timestamp"],
    );
    insert_first(&mut position, source, "entry_notional", &["entry_notional"]);
    insert_first(&mut position, source, "entry_reason", &["entry_reason"]);
    insert_first(&mut position, source, "reason", &["reason"]);
    insert_first(&mut position, source, "stop_loss", &["stop_loss"]);
    insert_first(&mut position, source, "take_profit", &["take_profit"]);
    insert_first(
        &mut position,
        source,
        "planned_exit_time",
        &["planned_exit_time"],
    );
    insert_first(
        &mut position,
        source,
        "planned_exit_reason",
        &["planned_exit_reason"],
    );
    insert_first(
        &mut position,
        source,
        "planned_hold_bars",
        &["planned_hold_bars"],
    );
    insert_first(
        &mut position,
        source,
        "mark_price",
        &["mark_price", "markPx"],
    );
    insert_first(
        &mut position,
        source,
        "mark_price_source",
        &["mark_price_source"],
    );
    insert_first(
        &mut position,
        source,
        "mark_price_missing",
        &["mark_price_missing"],
    );
    insert_first(
        &mut position,
        source,
        "notional",
        &["notional", "notionalUsd"],
    );
    insert_first(
        &mut position,
        source,
        "position_notional",
        &["position_notional", "notionalUsd"],
    );
    insert_first(
        &mut position,
        source,
        "unrealized_pnl",
        &["unrealized_pnl", "upl"],
    );
    insert_first(
        &mut position,
        source,
        "unrealized_pnl_pct",
        &["unrealized_pnl_pct"],
    );
    let output_key = if fallback_key == symbol {
        symbol
    } else {
        format!("{symbol}:{fallback_key}")
    };
    Some((output_key, position))
}

fn insert_first(
    target: &mut serde_json::Map<String, Value>,
    source: &serde_json::Map<String, Value>,
    target_key: &str,
    source_keys: &[&str],
) {
    for key in source_keys {
        if let Some(field) = source.get(*key) {
            target.insert(target_key.to_string(), field.clone());
            return;
        }
    }
}

fn trade_json(trade: &TradeRecord) -> Value {
    json!({
        "symbol": trade.symbol,
        "timestamp": trade.timestamp,
        "datetime": timestamp_to_text(trade.timestamp),
        "side": trade.side,
        "pos_side": trade.pos_side,
        "action": trade.action,
        "price": round6(trade.price),
        "quantity": round8(trade.quantity),
        "base_quantity": round8(trade.quantity),
        "base_size": round8(trade.quantity),
        "exchange_quantity": round8(trade.exchange_quantity),
        "size": round8(trade.exchange_quantity),
        "value": round6(trade.value),
        "commission": round6(trade.commission),
        "pnl": trade.pnl.map(round6),
        "funding": round6(trade.funding),
        "equity": trade.equity.map(round6),
        "reason": trade.reason,
        "metadata": {
            "pos_side": trade.pos_side,
            "action": trade.action,
            "symbol": trade.symbol,
            "funding": round6(trade.funding),
            "equity": trade.equity.map(round6)
        }
    })
}

fn trade_detail_values(trades: &[TradeRecord]) -> (Vec<Value>, usize, bool) {
    let events = trades.iter().map(trade_json).collect::<Vec<_>>();
    (events, trades.len(), false)
}

fn trade_cashflow_values(trades: &[TradeRecord]) -> Vec<Value> {
    trades
        .iter()
        .filter_map(|trade| {
            let pnl = trade.pnl.unwrap_or(0.0);
            let cashflow = pnl + trade.funding;
            if !cashflow.is_finite() || cashflow == 0.0 {
                return None;
            }
            Some(json!({
                "timestamp": trade.timestamp,
                "pnl": trade.pnl.map(round6),
                "funding": round6(trade.funding),
                "cashflow": round6(cashflow),
                "action": trade.action,
                "symbol": trade.symbol,
            }))
        })
        .collect()
}

fn timestamp_to_text(timestamp: i64) -> String {
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(timestamp)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trade_json_exposes_live_like_top_level_action_fields() {
        let trade = TradeRecord {
            symbol: Some("BTC-USDT-SWAP".to_string()),
            timestamp: 1_765_584_000_000,
            side: "sell".to_string(),
            pos_side: Some("short".to_string()),
            action: Some("open_position".to_string()),
            price: 100.1234567,
            quantity: 2.0,
            exchange_quantity: 20.0,
            value: 200.246913,
            commission: 0.12,
            pnl: None,
            funding: 0.03,
            equity: Some(999.91),
            reason: "entry".to_string(),
        };

        let row = trade_json(&trade);

        assert_eq!(row["symbol"], json!("BTC-USDT-SWAP"));
        assert_eq!(row["action"], json!("open_position"));
        assert_eq!(row["pos_side"], json!("short"));
        assert_eq!(row["quantity"], json!(2.0));
        assert_eq!(row["base_quantity"], json!(2.0));
        assert_eq!(row["exchange_quantity"], json!(20.0));
        assert_eq!(row["size"], json!(20.0));
        assert_eq!(row["funding"], json!(0.03));
        assert_eq!(row["equity"], json!(999.91));
        assert_eq!(row["metadata"]["action"], json!("open_position"));
        assert_eq!(row["metadata"]["pos_side"], json!("short"));
    }

    #[test]
    fn trade_details_and_cashflows_keep_full_history_for_pairing() {
        let trade_count = 502;
        let trades = (0..trade_count)
            .map(|index| TradeRecord {
                symbol: Some("BTC-USDT-SWAP".to_string()),
                timestamp: index as i64,
                side: "funding".to_string(),
                pos_side: Some("long".to_string()),
                action: Some("funding".to_string()),
                price: 100.0,
                quantity: 1.0,
                exchange_quantity: 1.0,
                value: 100.0,
                commission: 0.0,
                pnl: None,
                funding: -0.01,
                equity: None,
                reason: "funding_rate".to_string(),
            })
            .collect::<Vec<_>>();

        let (detail_trades, total, truncated) = trade_detail_values(&trades);
        let cashflows = trade_cashflow_values(&trades);

        assert_eq!(total, trade_count);
        assert!(!truncated);
        assert_eq!(detail_trades.len(), trade_count);
        assert_eq!(detail_trades[0]["timestamp"], json!(0));
        assert_eq!(
            detail_trades.last().unwrap()["timestamp"],
            json!((trade_count - 1) as i64)
        );
        assert_eq!(cashflows.len(), trade_count);
        assert_eq!(cashflows[0]["timestamp"], json!(0));
        assert_eq!(
            cashflows.last().unwrap()["timestamp"],
            json!((trade_count - 1) as i64)
        );
    }

    #[test]
    fn compact_equity_point_keeps_live_like_position_arrays() {
        let point = compact_equity_point(json!({
            "timestamp": 1,
            "equity": 1008.0,
            "cash": 990.0,
            "position_notional": 400.0,
            "position_side": "long",
            "positions": [
                {
                    "instId": "ADA-USDT-SWAP",
                    "symbol": "ADA-USDT-SWAP",
                    "instType": "SWAP",
                    "posSide": "long",
                    "pos": 143100.0,
                    "basePos": 1431.0,
                    "avgPx": 0.2447,
                    "markPx": 0.2471,
                    "mark_price_source": "historical_last_close",
                    "mark_price_missing": false,
                    "notionalUsd": 353.6,
                    "upl": 3.4
                }
            ]
        }));

        let positions = point
            .get("positions")
            .and_then(Value::as_array)
            .expect("positions should be preserved");
        let position = positions
            .first()
            .and_then(Value::as_object)
            .expect("position should be compacted");

        assert_eq!(point["position_count"], json!(1));
        assert_eq!(position["symbol"], json!("ADA-USDT-SWAP"));
        assert_eq!(position["side"], json!("long"));
        assert_eq!(position["inst_type"], json!("SWAP"));
        assert_eq!(position["entry_price"], json!(0.2447));
        assert_eq!(position["quantity"], json!(1431.0));
        assert_eq!(position["exchange_quantity"], json!(143100.0));
        assert_eq!(position["mark_price"], json!(0.2471));
        assert_eq!(
            position["mark_price_source"],
            json!("historical_last_close")
        );
        assert_eq!(position["mark_price_missing"], json!(false));
        assert_eq!(position["position_notional"], json!(353.6));
        assert_eq!(position["unrealized_pnl"], json!(3.4));
    }
}
