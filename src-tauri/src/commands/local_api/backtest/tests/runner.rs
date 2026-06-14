use super::*;

#[test]
fn normalize_strategy_inst_id_respects_instrument_type() {
    assert_eq!(
        normalize_strategy_inst_id("btc-usdt", "SWAP").unwrap(),
        "BTC-USDT-SWAP"
    );
    assert_eq!(
        normalize_strategy_inst_id("BTC-USDT-SWAP", "SPOT").unwrap(),
        "BTC-USDT"
    );
}

#[test]
fn runtime_backtest_runner_uses_historical_live_protocol_only() {
    let source = format!(
        "{}\n{}\n{}",
        include_str!("../runner.rs"),
        include_str!("../runner/context_data.rs"),
        include_str!("../runner/context_data/candles.rs")
    );

    assert!(source.contains("compute_runtime_decision_with_context_ref_and_events"));
    assert!(source.contains("cache_runtime_context"));
    assert!(source.contains("plan_runtime_actions_for_execution"));
    assert!(source.contains("required_action_candle_count_for_timeframe"));
    assert!(!source.contains("primary_candles.len().max(3)"));
    assert!(!source.contains("compute_runtime_decision_with_context_and_progress"));
    assert!(!source.contains("evaluate_history"));
    assert!(!source.contains("run_backtest_with_runtime_actions"));
    assert!(!source.contains("run_backtest_with_runtime_evaluator"));
    assert!(!source.contains("run_backtest_with_signals"));
}

#[test]
fn runtime_backtest_runner_does_not_report_attached_risk_actions_as_skipped() {
    let source = include_str!("../runner.rs");

    assert!(source.contains("risk_actions_total += risk_actions_count"));
    assert!(source.contains("\"risk_actions\": risk_actions_count"));
    assert!(!source.contains("skipped_actions.extend(risk_actions"));
    assert!(!source.contains("\"risk_orders\":"));
    assert!(!source.contains("\"risk_orders_total\":"));
    assert!(!source.contains("risk_order_attached_or_standalone"));
}

#[test]
fn runtime_backtest_runner_uses_strict_action_batch_execution_plan() {
    let source = include_str!("../runner.rs");

    assert!(source.contains("plan_runtime_actions_for_execution"));
    assert!(!source.contains("plan_runtime_actions(&decision.actions"));
}

#[test]
fn runtime_backtest_execution_delay_defaults_to_zero() {
    let config = test_strategy_config();

    assert_eq!(backtest_execution_delay_ms(&config), 0);
    assert_eq!(backtest_execution_submit_timestamp(1_000, &config), 1_000);
}

#[test]
fn runtime_backtest_execution_delay_accepts_namespaced_backtest_aliases() {
    let mut config = test_strategy_config();
    config.params = json!({"backtest_execution_delay_ms": 2500.0});
    assert_eq!(backtest_execution_delay_ms(&config), 2500);
    assert_eq!(backtest_execution_submit_timestamp(1_000, &config), 3_500);

    config.params = json!({"historical_execution_delay_ms": "1500"});
    assert_eq!(backtest_execution_delay_ms(&config), 1500);
}

#[test]
fn runtime_backtest_execution_delay_ignores_invalid_and_saturates_timestamp() {
    let mut config = test_strategy_config();
    config.params = json!({"historical_execution_delay_ms": -10});
    assert_eq!(backtest_execution_delay_ms(&config), 0);

    config.params = json!({"historical_execution_delay_ms": 10});
    assert_eq!(
        backtest_execution_submit_timestamp(i64::MAX - 5, &config),
        i64::MAX
    );
}

#[test]
fn runtime_backtest_default_warmup_uses_live_required_window() {
    let mut config = test_strategy_config();
    config.timeframe = "15m".to_string();
    config.params = json!({});

    assert_eq!(default_backtest_primary_min_bars(&config), 160);

    config.params = json!({
        "fast_window": 20,
        "slow_window": 1000
    });
    assert_eq!(default_backtest_primary_min_bars(&config), 1001);
}

#[test]
fn backtest_instrument_rules_source_defaults_to_simulated() {
    let config = test_strategy_config();

    assert_eq!(
        backtest_instrument_rules_source(&config).unwrap(),
        BacktestInstrumentRulesSource::Simulated
    );
}

#[test]
fn backtest_instrument_rules_source_accepts_params_and_okx_modes() {
    let mut config = test_strategy_config();
    config.params = json!({ "backtest_instrument_rules_source": "params" });
    assert_eq!(
        backtest_instrument_rules_source(&config).unwrap(),
        BacktestInstrumentRulesSource::Params
    );

    config.params = json!({ "backtest_instrument_rules_source": "okx" });
    assert_eq!(
        backtest_instrument_rules_source(&config).unwrap(),
        BacktestInstrumentRulesSource::Okx
    );
}

#[test]
fn backtest_instrument_rules_source_rejects_unknown_mode() {
    let mut config = test_strategy_config();
    config.params = json!({ "backtest_instrument_rules_source": "guess" });

    let error = backtest_instrument_rules_source(&config)
        .unwrap_err()
        .to_string();

    assert!(error.contains("simulated、params、okx"));
}

#[test]
fn simulated_instrument_rules_fill_missing_contract_specs_without_overwriting_user_values() {
    let mut config = test_strategy_config();
    config.symbol = "BTC-USDT-SWAP".to_string();
    config.inst_type = "SWAP".to_string();
    config.params = json!({
        "ctVal": 0.01,
        "lotSz": 0.01
    });

    let config = config_with_simulated_instrument_rules(&config);

    assert_eq!(config.params["ctVal"].as_f64(), Some(0.01));
    assert_eq!(config.params["lotSz"].as_f64(), Some(0.01));
    assert_eq!(config.params["ctValCcy"].as_str(), Some("BTC"));
    assert_eq!(config.params["minSz"].as_f64(), Some(1.0));
    assert_eq!(config.params["tickSz"].as_f64(), Some(0.00000001));
    assert_eq!(
        config.params["_backtest_instrument_rules_source_resolved"].as_str(),
        Some("simulated")
    );
}

#[test]
fn params_instrument_rules_source_only_marks_resolved_source() {
    let mut config = test_strategy_config();
    config.symbol = "BTC-USDT-SWAP".to_string();
    config.inst_type = "SWAP".to_string();
    config.params = json!({
        "backtest_instrument_rules_source": "params"
    });

    let config = config_with_resolved_instrument_rules_source(&config, "params");

    assert_eq!(
        config.params["_backtest_instrument_rules_source_resolved"].as_str(),
        Some("params")
    );
    assert!(config.params.get("ctVal").is_none());
    assert!(config.params.get("ctValCcy").is_none());
    assert!(config.params.get("lotSz").is_none());
    assert!(config.params.get("minSz").is_none());
    assert!(config.params.get("tickSz").is_none());
}

#[test]
fn backtest_required_instruments_include_runtime_requirement_inst_types() {
    let mut config = test_strategy_config();
    config.symbol = "BTC-USDT".to_string();
    config.inst_type = "SPOT".to_string();
    config.timeframe = "1H".to_string();

    let required = backtest_required_instruments(
        &json!({
            "candles": [
                {
                    "symbol": "btc-usdt",
                    "inst_type": "SWAP",
                    "timeframe": "5m",
                    "min_bars": 20
                }
            ],
            "orderbook": [
                {
                    "symbol": "ETH-USDT",
                    "inst_type": "SWAP",
                    "required": false
                }
            ],
            "funding": [
                {
                    "symbol": "SOL-USDT",
                    "inst_type": "SWAP",
                    "required": true
                }
            ]
        }),
        &config,
    )
    .unwrap();

    assert!(required.contains(&("BTC-USDT".to_string(), "SPOT".to_string())));
    assert!(required.contains(&("BTC-USDT-SWAP".to_string(), "SWAP".to_string())));
    assert!(required.contains(&("ETH-USDT-SWAP".to_string(), "SWAP".to_string())));
    assert!(required.contains(&("SOL-USDT-SWAP".to_string(), "SWAP".to_string())));
}

#[test]
fn strict_instrument_rules_do_not_gate_optional_runtime_feeds() {
    let mut config = test_strategy_config();
    config.symbol = "BTC-USDT".to_string();
    config.inst_type = "SPOT".to_string();
    config.timeframe = "1H".to_string();

    let data_requirements = json!({
        "candles": [
            {
                "symbol": "BTC-USDT",
                "inst_type": "SWAP",
                "timeframe": "5m",
                "min_bars": 20
            }
        ],
        "orderbook": [
            {
                "symbol": "ETH-USDT",
                "inst_type": "SWAP",
                "required": false
            }
        ],
        "funding": [
            {
                "symbol": "SOL-USDT",
                "inst_type": "SWAP",
                "required": true
            },
            {
                "symbol": "XRP-USDT",
                "inst_type": "SWAP",
                "required": false
            }
        ]
    });

    let declared = backtest_required_instruments(&data_requirements, &config).unwrap();
    let strict = backtest_strict_instrument_rules(&data_requirements, &config).unwrap();

    assert!(declared.contains(&("ETH-USDT-SWAP".to_string(), "SWAP".to_string())));
    assert!(declared.contains(&("XRP-USDT-SWAP".to_string(), "SWAP".to_string())));
    assert!(strict.contains(&("BTC-USDT".to_string(), "SPOT".to_string())));
    assert!(strict.contains(&("BTC-USDT-SWAP".to_string(), "SWAP".to_string())));
    assert!(strict.contains(&("SOL-USDT-SWAP".to_string(), "SWAP".to_string())));
    assert!(!strict.contains(&("ETH-USDT-SWAP".to_string(), "SWAP".to_string())));
    assert!(!strict.contains(&("XRP-USDT-SWAP".to_string(), "SWAP".to_string())));
}

#[test]
fn simulated_instrument_rules_fill_runtime_contract_scope_from_requirements() {
    let mut config = test_strategy_config();
    config.symbol = "BTC-USDT".to_string();
    config.inst_type = "SPOT".to_string();
    config.params = json!({});
    let required = backtest_required_instruments(
        &json!({
            "candles": [
                {
                    "symbol": "BTC-USDT",
                    "inst_type": "SWAP",
                    "timeframe": "1H",
                    "min_bars": 20
                }
            ]
        }),
        &config,
    )
    .unwrap();

    let config = config_with_simulated_instrument_rules_for(&config, &required);
    let rules_by_symbol = config.params["_backtest_instrument_rules_by_symbol"]
        .as_object()
        .unwrap();
    let spot_rules = rules_by_symbol["BTC-USDT"].as_object().unwrap();
    let swap_rules = rules_by_symbol["BTC-USDT-SWAP"].as_object().unwrap();

    assert_eq!(spot_rules["instType"].as_str(), Some("SPOT"));
    assert_eq!(spot_rules["lotSz"].as_f64(), Some(0.00000001));
    assert_eq!(swap_rules["instType"].as_str(), Some("SWAP"));
    assert_eq!(swap_rules["ctVal"].as_f64(), Some(1.0));
    assert_eq!(swap_rules["ctValCcy"].as_str(), Some("BTC"));
    assert_eq!(swap_rules["lotSz"].as_f64(), Some(1.0));
    assert_eq!(swap_rules["minSz"].as_f64(), Some(1.0));
}

#[test]
fn okx_instrument_rules_merge_runtime_requirement_inst_types() {
    let mut config = test_strategy_config();
    config.symbol = "BTC-USDT".to_string();
    config.inst_type = "SPOT".to_string();
    config.params = json!({ "backtest_instrument_rules_source": "okx" });
    let required = vec![
        ("BTC-USDT".to_string(), "SPOT".to_string()),
        ("BTC-USDT-SWAP".to_string(), "SWAP".to_string()),
    ];
    let mut instruments_by_inst_type = std::collections::BTreeMap::new();
    instruments_by_inst_type.insert(
        "SPOT".to_string(),
        vec![json!({
            "instId": "BTC-USDT",
            "instType": "SPOT",
            "state": "live",
            "minSz": "0.00001",
            "lotSz": "0.00000001",
            "tickSz": "0.1"
        })],
    );
    instruments_by_inst_type.insert(
        "SWAP".to_string(),
        vec![json!({
            "instId": "BTC-USDT-SWAP",
            "instType": "SWAP",
            "state": "live",
            "minSz": "1",
            "lotSz": "1",
            "tickSz": "0.1",
            "ctVal": "0.01",
            "ctValCcy": "BTC"
        })],
    );

    let config = config_with_okx_instrument_rules_from_instruments(
        &config,
        &required,
        &instruments_by_inst_type,
    )
    .unwrap();
    let rules_by_symbol = config.params["_backtest_instrument_rules_by_symbol"]
        .as_object()
        .unwrap();

    assert_eq!(config.params["instId"].as_str(), Some("BTC-USDT"));
    assert_eq!(config.params["instType"].as_str(), Some("SPOT"));
    assert!(rules_by_symbol.get("BTC-USDT").is_some());
    assert_eq!(
        rules_by_symbol["BTC-USDT-SWAP"]["ctVal"].as_str(),
        Some("0.01")
    );
    assert_eq!(
        config.params["_backtest_instrument_rules_source_resolved"].as_str(),
        Some("okx")
    );
}

#[test]
fn compact_backtest_diagnostics_is_explicit_opt_in() {
    let mut config = test_strategy_config();
    config.params = json!({});
    assert!(!compact_backtest_diagnostics_enabled(&config));

    config.params = json!({ "compact_backtest_diagnostics": true });
    assert!(compact_backtest_diagnostics_enabled(&config));

    config.params = json!({ "compact_backtest_diagnostics": "0" });
    assert!(!compact_backtest_diagnostics_enabled(&config));
}

#[test]
fn compact_strategy_diagnostics_omits_heavy_payloads_without_contract_fields() {
    let diagnostics = json!({
        "history_action_contract": {
            "status": "planned_exit_complete"
        },
        "scoring": {
            "rows": [1, 2, 3],
            "skipped_by_reason": {"bad": 2}
        },
        "candidate_rows": [
            {"symbol": "BTC-USDT-SWAP"},
            {"symbol": "ETH-USDT-SWAP"}
        ],
        "selected_symbols": ["BTC-USDT-SWAP"]
    });

    let compacted = compact_strategy_diagnostics(&diagnostics);

    assert!(compacted.get("scoring").is_none());
    assert!(compacted.get("candidate_rows").is_none());
    assert_eq!(
        compacted["scoring_omitted"]["value_type"].as_str(),
        Some("object")
    );
    assert_eq!(
        compacted["candidate_rows_omitted"]["item_count"].as_u64(),
        Some(2)
    );
    assert_eq!(
        compacted["history_action_contract"]["status"].as_str(),
        Some("planned_exit_complete")
    );
    assert_eq!(
        compacted["selected_symbols"][0].as_str(),
        Some("BTC-USDT-SWAP")
    );
}

#[test]
fn backtest_context_required_start_includes_warmup_before_window() {
    let window = BacktestWindow {
        start_ts: utc_ms(2026, 6, 1, 0, 0, 0, 0),
        end_ts: utc_ms(2026, 6, 2, 0, 0, 0, 0),
        days: 1,
    };

    assert_eq!(
        backtest_context_required_start_ts(window, "15m", 10_000),
        window.start_ts - 9_999 * 15 * 60_000
    );
    assert_eq!(
        backtest_context_required_start_ts(window, "5m", 4_500),
        window.start_ts - 4_499 * 5 * 60_000
    );
}

#[test]
fn backtest_context_required_end_aligns_to_timeframe() {
    let window = BacktestWindow {
        start_ts: utc_ms(2026, 6, 1, 0, 0, 0, 0),
        end_ts: utc_ms(2026, 6, 1, 23, 59, 59, 999),
        days: 1,
    };

    assert_eq!(
        backtest_context_required_end_ts(window, "1H"),
        utc_ms(2026, 6, 1, 23, 0, 0, 0)
    );
    assert_eq!(
        backtest_context_required_end_ts(window, "15m"),
        utc_ms(2026, 6, 1, 23, 45, 0, 0)
    );
}

#[test]
fn context_candles_cover_window_warmup_requires_range_tail() {
    let mut candles = test_candles(5);
    let required_start_ts = candles.first().unwrap().timestamp;
    let required_end_ts = candles.last().unwrap().timestamp;

    assert!(context_candles_cover_window_warmup(
        &candles,
        5,
        required_start_ts,
        required_end_ts
    ));

    candles.pop();
    assert!(!context_candles_cover_window_warmup(
        &candles,
        4,
        required_start_ts,
        required_end_ts
    ));
}

#[test]
fn backtest_context_prefix_len_preserves_take_while_boundaries() {
    let candles = test_candles(4);
    let timestamps = candles
        .iter()
        .map(|candle| candle.timestamp)
        .collect::<Vec<_>>();
    let cached_jsons = candles.iter().map(OkxCandle::to_json).collect::<Vec<_>>();
    let probe_timestamps = [
        timestamps[0] - 1,
        timestamps[0],
        timestamps[1] - 1,
        timestamps[1],
        timestamps[3],
        timestamps[3] + HOUR_MS,
    ];

    for timestamp in probe_timestamps {
        let expected_len = candles
            .iter()
            .take_while(|candle| candle.timestamp <= timestamp)
            .count();
        let actual_len = backtest_context_prefix_len_at_or_before(&timestamps, timestamp);
        let expected_jsons = candles
            .iter()
            .take_while(|candle| candle.timestamp <= timestamp)
            .map(OkxCandle::to_json)
            .collect::<Vec<_>>();

        assert_eq!(actual_len, expected_len);
        assert_eq!(cached_jsons[..actual_len].to_vec(), expected_jsons);
    }
}
