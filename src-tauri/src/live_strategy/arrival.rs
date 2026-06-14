use serde_json::Value;

use crate::{
    okx::OkxPublicClient,
    trading_semantics::{check_max_adverse_slippage, max_slippage_from_params},
};

use super::types::LiveStrategyConfig;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ArrivalQuote {
    pub(in crate::live_strategy) ts_ms: Option<i64>,
    pub(in crate::live_strategy) mid_px: Option<f64>,
    pub(in crate::live_strategy) bid_px: Option<f64>,
    pub(in crate::live_strategy) ask_px: Option<f64>,
}

pub(in crate::live_strategy) async fn fetch_arrival_quote(
    client: &OkxPublicClient,
    config: &LiveStrategyConfig,
) -> ArrivalQuote {
    match client.get_orderbook(&config.symbol, 1).await {
        Ok(book) => {
            if let Some(quote) = quote_from_orderbook(&book) {
                return quote;
            }
        }
        Err(error) => {
            tracing::debug!(
                symbol = config.symbol.as_str(),
                error = %error,
                "failed to fetch orderbook arrival quote"
            );
        }
    }

    match client.get_ticker(&config.symbol).await {
        Ok(ticker) => quote_from_ticker(&ticker).unwrap_or_default(),
        Err(error) => {
            tracing::debug!(
                symbol = config.symbol.as_str(),
                error = %error,
                "failed to fetch ticker arrival quote"
            );
            ArrivalQuote::default()
        }
    }
}

pub(crate) fn check_slippage_control(
    config: &LiveStrategyConfig,
    side: &str,
    reference_price: f64,
    arrival: ArrivalQuote,
) -> (bool, String) {
    check_max_adverse_slippage(
        max_slippage_from_params(&config.params),
        side,
        reference_price,
        arrival.bid_px,
        arrival.ask_px,
    )
}

fn quote_from_orderbook(book: &Value) -> Option<ArrivalQuote> {
    let bid = positive_f64(book.get("best_bid"))?;
    let ask = positive_f64(book.get("best_ask"))?;
    Some(ArrivalQuote {
        ts_ms: positive_i64(book.get("ts")).or_else(now_ms),
        mid_px: Some(positive_f64(book.get("mid_price")).unwrap_or((bid + ask) / 2.0)),
        bid_px: Some(bid),
        ask_px: Some(ask),
    })
}

fn quote_from_ticker(ticker: &Value) -> Option<ArrivalQuote> {
    let bid = positive_f64(ticker.get("bidPx").or_else(|| ticker.get("bid")))?;
    let ask = positive_f64(ticker.get("askPx").or_else(|| ticker.get("ask")))?;
    Some(ArrivalQuote {
        ts_ms: positive_i64(ticker.get("ts")).or_else(now_ms),
        mid_px: Some((bid + ask) / 2.0),
        bid_px: Some(bid),
        ask_px: Some(ask),
    })
}

fn positive_f64(value: Option<&Value>) -> Option<f64> {
    let parsed = match value? {
        Value::Number(item) => item.as_f64(),
        Value::String(item) => item.parse::<f64>().ok(),
        _ => None,
    }?;
    if parsed.is_finite() && parsed > 0.0 {
        Some(parsed)
    } else {
        None
    }
}

fn positive_i64(value: Option<&Value>) -> Option<i64> {
    let parsed = match value? {
        Value::Number(item) => item.as_i64(),
        Value::String(item) => item.parse::<i64>().ok(),
        _ => None,
    }?;
    if parsed > 0 {
        Some(parsed)
    } else {
        None
    }
}

fn now_ms() -> Option<i64> {
    Some(chrono::Utc::now().timestamp_millis())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;

    use super::{check_slippage_control, quote_from_orderbook, quote_from_ticker, ArrivalQuote};
    use crate::live_strategy::types::LiveStrategyConfig;

    #[test]
    fn orderbook_quote_requires_bid_and_ask() {
        let quote = quote_from_orderbook(&json!({
            "best_bid": 99.0,
            "best_ask": 101.0,
            "mid_price": 100.0
        }))
        .expect("complete orderbook quote");

        assert_eq!(quote.bid_px, Some(99.0));
        assert_eq!(quote.ask_px, Some(101.0));
        assert_eq!(quote.mid_px, Some(100.0));
        assert!(quote.ts_ms.is_some());
        assert!(quote_from_orderbook(&json!({"best_bid": 99.0})).is_none());
    }

    #[test]
    fn ticker_quote_uses_bid_ask_midpoint() {
        let quote = quote_from_ticker(&json!({
            "bidPx": "199.5",
            "askPx": "200.5"
        }))
        .expect("complete ticker quote");

        assert_eq!(quote.bid_px, Some(199.5));
        assert_eq!(quote.ask_px, Some(200.5));
        assert_eq!(quote.mid_px, Some(200.0));
        assert!(quote.ts_ms.is_some());
    }

    #[test]
    fn explicit_slippage_limit_blocks_adverse_buy_quote() {
        let config = config_with_params(json!({"_runtime_max_slippage": 0.002}));
        let quote = ArrivalQuote {
            ask_px: Some(101.0),
            bid_px: Some(99.0),
            ..ArrivalQuote::default()
        };

        let (passed, reason) = check_slippage_control(&config, "buy", 100.0, quote);

        assert!(!passed);
        assert!(reason.contains("预估滑点 100.00 bps"));
    }

    #[test]
    fn explicit_slippage_limit_passes_improved_sell_quote() {
        let config = config_with_params(json!({"max_slippage_bps": 20.0}));
        let quote = ArrivalQuote {
            ask_px: Some(101.0),
            bid_px: Some(100.2),
            ..ArrivalQuote::default()
        };

        let (passed, reason) = check_slippage_control(&config, "sell", 100.0, quote);

        assert!(passed, "{reason}");
    }

    #[test]
    fn explicit_slippage_limit_blocks_missing_bid_ask() {
        let config = config_with_params(json!({"max_slippage_bps": 20.0}));

        let (passed, reason) =
            check_slippage_control(&config, "buy", 100.0, ArrivalQuote::default());

        assert!(!passed);
        assert!(reason.contains("缺少可用 bid/ask 报价"));
    }

    #[test]
    fn slippage_check_ignores_string_params() {
        let config = config_with_params(json!({"max_slippage_bps": "20"}));

        let (passed, reason) =
            check_slippage_control(&config, "buy", 100.0, ArrivalQuote::default());

        assert!(passed, "{reason}");
    }

    fn config_with_params(params: serde_json::Value) -> LiveStrategyConfig {
        LiveStrategyConfig {
            strategy_id: "slippage_test".to_string(),
            strategy_name: "Slippage Test".to_string(),
            symbol: "BTC-USDT-SWAP".to_string(),
            timeframe: "15m".to_string(),
            inst_type: "SWAP".to_string(),
            mode: "simulated".to_string(),
            initial_capital: 1_000.0,
            position_size: 0.1,
            stop_loss: 0.0,
            take_profit: 0.0,
            risk_timeframe: "1m".to_string(),
            check_interval: 60,
            params,
            project_root: PathBuf::new(),
            risk_control_enabled: true,
            max_single_loss_ratio: 0.05,
            max_position_pct: 1.0,
            max_order_value: 10_000.0,
        }
    }
}
