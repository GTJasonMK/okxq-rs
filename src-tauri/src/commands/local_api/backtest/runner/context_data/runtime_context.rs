use serde_json::{json, Value};

use crate::{
    strategy_engine::{HistoricalLiveBacktest, HistoricalMarketData},
    strategy_executor::RuntimeStateRequirements,
};

pub(in crate::commands::local_api::backtest::runner) fn dynamic_backtest_context(
    config_json: &Value,
    state_requirements: RuntimeStateRequirements,
    timestamp: i64,
    backtest: &HistoricalLiveBacktest,
    market: &HistoricalMarketData,
) -> Value {
    json!({
        "account": if state_requirements.account {
            backtest.account_context(timestamp, market)
        } else {
            json!({})
        },
        "positions": if state_requirements.positions {
            backtest.positions_context(timestamp, market)
        } else {
            json!({})
        },
        "orders": if state_requirements.orders {
            backtest.orders_context()
        } else {
            json!({})
        },
        "time": {
            "timestamp": timestamp,
            "timeframe": config_text(config_json, "timeframe"),
        },
        "runtime": {
            "strategy_id": config_text(config_json, "strategy_id"),
            "strategy_name": config_text(config_json, "strategy_name"),
            "symbol": config_text(config_json, "symbol"),
            "inst_type": config_text(config_json, "inst_type"),
            "timeframe": config_text(config_json, "timeframe"),
        },
    })
}

fn config_text<'a>(config_json: &'a Value, key: &str) -> &'a str {
    config_json
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("backtest runtime config {key} should be a string"))
}

pub(in crate::commands::local_api::backtest::runner) fn context_with_backtest_progress(
    context: &Value,
    processed_candles: usize,
    total_candles: usize,
) -> Value {
    let mut next = context.clone();
    assert!(
        total_candles > 0,
        "backtest progress total should be positive"
    );
    assert!(
        processed_candles <= total_candles,
        "backtest processed candles should not exceed total candles"
    );
    let progress = processed_candles as f64 / total_candles as f64;
    let object = next
        .as_object_mut()
        .expect("backtest runtime context should be an object");
    object.insert(
        "backtest".to_string(),
        json!({
            "processed_candles": processed_candles,
            "total_candles": total_candles,
            "progress": progress,
            "stage": "historical_live",
            "message": format!("Historical live step {processed_candles}/{total_candles}"),
        }),
    );
    next
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{
        okx::OkxCandle,
        strategy_engine::{
            HistoricalCandleSeries, HistoricalLiveBacktest, HistoricalMarketData, StrategyConfig,
        },
        strategy_executor::RuntimeStateRequirements,
    };

    use super::{context_with_backtest_progress, dynamic_backtest_context};

    #[test]
    fn dynamic_backtest_context_hides_unrequested_private_state_like_live() {
        let config = test_config();
        let candles = vec![candle(1)];
        let market = market(&config, &candles);
        let backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
        let context = dynamic_backtest_context(
            &config_json(&config),
            RuntimeStateRequirements::default(),
            1,
            &backtest,
            &market,
        );

        assert_eq!(context["account"], json!({}));
        assert_eq!(context["positions"], json!({}));
        assert_eq!(context["orders"], json!({}));
        assert_eq!(context["time"]["timestamp"].as_i64(), Some(1));
        assert_eq!(
            context["runtime"]["strategy_id"].as_str(),
            Some("runtime-context-test")
        );
    }

    #[test]
    fn dynamic_backtest_context_exposes_requested_private_state_like_live() {
        let config = test_config();
        let candles = vec![candle(1)];
        let market = market(&config, &candles);
        let backtest = HistoricalLiveBacktest::new(&config, &candles, 1);
        let context = dynamic_backtest_context(
            &config_json(&config),
            RuntimeStateRequirements {
                account: true,
                positions: true,
                orders: true,
            },
            1,
            &backtest,
            &market,
        );

        assert_eq!(
            context["account"]["source"].as_str(),
            Some("historical_live_backtest")
        );
        assert_eq!(
            context["positions"]["open"].as_array().map(Vec::len),
            Some(0)
        );
        assert_eq!(context["orders"]["open"].as_array().map(Vec::len), Some(0));
        assert_eq!(
            context["orders"]["recent_fills"].as_array().map(Vec::len),
            Some(0)
        );
        assert_eq!(
            context["orders"]["recent_rejections"]
                .as_array()
                .map(Vec::len),
            Some(0)
        );
    }

    #[test]
    fn context_with_backtest_progress_requires_valid_loop_bounds() {
        let context = json!({"runtime": {"strategy_id": "fixture"}});

        let context = context_with_backtest_progress(&context, 3, 10);

        assert_eq!(context["backtest"]["processed_candles"], json!(3));
        assert_eq!(context["backtest"]["total_candles"], json!(10));
        assert_eq!(context["backtest"]["progress"], json!(0.3));
        assert_eq!(
            context["backtest"]["message"].as_str(),
            Some("Historical live step 3/10")
        );
    }

    fn test_config() -> StrategyConfig {
        StrategyConfig {
            strategy_id: "runtime-context-test".to_string(),
            strategy_name: "Runtime Context Test".to_string(),
            symbol: "BTC-USDT-SWAP".to_string(),
            inst_type: "SWAP".to_string(),
            timeframe: "1m".to_string(),
            initial_capital: 10_000.0,
            position_size: 0.2,
            stop_loss: 0.05,
            take_profit: 0.1,
            params: json!({}),
        }
    }

    fn config_json(config: &StrategyConfig) -> serde_json::Value {
        json!({
            "strategy_id": config.strategy_id,
            "strategy_name": config.strategy_name,
            "symbol": config.symbol,
            "inst_type": config.inst_type,
            "timeframe": config.timeframe,
        })
    }

    fn candle(timestamp: i64) -> OkxCandle {
        OkxCandle {
            timestamp,
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close: 100.0,
            volume: 1.0,
            volume_ccy: 100.0,
            volume_quote: 100.0,
            confirm: "1".to_string(),
        }
    }

    fn market(config: &StrategyConfig, candles: &[OkxCandle]) -> HistoricalMarketData {
        HistoricalMarketData::new(vec![HistoricalCandleSeries {
            symbol: config.symbol.clone(),
            inst_type: config.inst_type.clone(),
            timeframe: config.timeframe.clone(),
            candles: candles.to_vec(),
        }])
    }
}
