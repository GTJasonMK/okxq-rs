from __future__ import annotations

import contextlib
import io
import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import lib.ml_trade_candidate_ranking as candidate_ranking  # noqa: E402
import lib.ml_trade_resource_limits as resource_limits  # noqa: E402
import lib.ml_trade_runtime_actions as runtime_actions  # noqa: E402
import lib.ml_trade_selector_runtime as selector_runtime  # noqa: E402
import lib.ml_trade_student_selector as student_selector  # noqa: E402
from lib.ml_trade_selector_runtime import (  # noqa: E402
    build_runtime_decision,
    model_risk_penalty_bps,
    order_from_ranked_item,
    protective_stop_reason,
)
from runtime.ml_trade_selector_forward_candidate_v1 import (  # noqa: E402
    DATA_REQUIREMENTS as PRODUCTION_DATA_REQUIREMENTS,
    RUNTIME_CONFIG as PRODUCTION_RUNTIME_CONFIG,
)


def test_order_from_ranked_item_preserves_planned_exit_fields():
    timestamp = 1_700_000_000_000
    item = ranked_item(timestamp)

    order = order_from_ranked_item(
        item,
        1,
        {},
        timestamp,
        100.0,
        {"global_weight": 0.72, "rank_weight": 0.35},
        600.0,
        20.0,
    )

    assert_planned_exit_contract(order, timestamp)


def test_latest_decision_returns_explicit_actions_not_legacy_orders_only():
    timestamp = 1_700_000_000_000
    candles = [
        {"timestamp": timestamp, "close": 100.0},
        {"timestamp": timestamp + 900_000, "close": 101.0},
    ]
    context = {
        "candles": {
            "BTC-USDT-SWAP": {"15m": candles},
            "ETH-USDT-SWAP": {
                "15m": [
                    {"timestamp": timestamp, "close": 200.0},
                    {"timestamp": timestamp + 900_000, "close": 202.0},
                ]
            },
        },
        "ml_trade_candidates": [
            streaming_candidate_row(timestamp + 900_000, "ETH-USDT-SWAP", 10.0, 0.9, 0.1)
        ],
    }

    decision = build_with_fake_student_scores(
        context,
        streaming_params({"student_execution_probability_threshold": 0.7, "risk_probability_cap": 0.4}),
    )
    open_actions = [action for action in decision["actions"] if action.get("action") == "open_position"]
    risk_actions = [action for action in decision["actions"] if action.get("action") == "place_risk_order"]

    assert len(open_actions) == 1
    assert len(risk_actions) == 1
    assert "orders" not in decision
    assert "risk_orders" not in decision
    assert "price" not in open_actions[0]
    assert open_actions[0]["reference_price"] == 202.0
    assert open_actions[0]["price_source"] == "strategy_reference"
    assert "price" not in risk_actions[0]
    assert risk_actions[0]["trigger_price"] > 0
    assert_planned_exit_contract(open_actions[0], timestamp + 900_000)
    assert decision["diagnostics"]["action_contract"]["status"] == "planned_exit_complete"
    assert decision["diagnostics"]["action_contract"]["open_actions_with_planned_exit"] == 1
    assert decision["diagnostics"]["action_intent"] == "open_position"
    assert decision["diagnostics"]["action_count"] == len(decision["actions"])
    assert decision["diagnostics"]["open_action_count"] == 1
    assert decision["diagnostics"]["risk_action_count"] == 1
    assert "triggered" not in decision["diagnostics"]
    assert "entry_triggered" not in decision["diagnostics"]
    assert "exit_triggered" not in decision["diagnostics"]
    assert "decision_intent" not in decision["diagnostics"]
    assert "signal_intent" not in decision["diagnostics"]
    assert "signal" not in decision["diagnostics"]


def test_production_data_requirements_expose_state_used_by_streaming_gate():
    assert PRODUCTION_DATA_REQUIREMENTS["positions"]["required"] is True
    assert PRODUCTION_DATA_REQUIREMENTS["orders"]["open"] is True
    assert PRODUCTION_DATA_REQUIREMENTS["orders"]["recent_fills"] is True


def test_latest_decision_actions_use_json_numbers_for_execution_numeric_fields():
    timestamp = 1_700_000_000_000
    context = {
        "candles": {
            "BTC-USDT-SWAP": {
                "15m": [
                    {"timestamp": timestamp, "close": 100.0},
                    {"timestamp": timestamp + 900_000, "close": 101.0},
                ]
            },
            "ETH-USDT-SWAP": {
                "15m": [
                    {"timestamp": timestamp, "close": 200.0},
                    {"timestamp": timestamp + 900_000, "close": 202.0},
                ]
            },
        },
        "orders": {"open": [], "recent_fills": [], "recent_rejections": []},
        "ml_trade_candidates": [
            streaming_candidate_row(timestamp + 900_000, "ETH-USDT-SWAP", 20.0, 0.82, 0.20),
        ],
    }

    decision = build_with_fake_student_scores(
        context,
        streaming_params({"student_execution_probability_threshold": 0.7, "risk_probability_cap": 0.4}),
    )

    assert decision["execution_logs"]
    for action in decision["actions"]:
        assert_json_number(action, "timestamp")
        if action["action"] == "open_position":
            assert "price" not in action
            assert_json_number(action, "reference_price")
            assert_json_number(action, "position_size")
            assert_json_number(action, "strength")
            assert_json_number(action, "planned_exit_time")
            assert_json_number(action, "stop_loss_bps")
            assert_json_number(action, "max_slippage_bps")
        if action["action"] == "place_risk_order":
            assert "price" not in action
            assert_json_number(action, "trigger_price")
            assert_json_number(action, "stop_loss_bps")
            assert_json_number(action, "max_slippage_bps")


def test_live_runtime_emits_internal_progress_strategy_logs():
    timestamp = 1_700_000_000_000
    row = streaming_candidate_row(timestamp + 900_000, "ETH-USDT-SWAP", 20.0, 0.82, 0.20)
    context = {
        "runtime": {"strategy_log_events": True},
        "candles": {
            "BTC-USDT-SWAP": {
                "15m": [
                    {"timestamp": timestamp, "close": 100.0},
                    {"timestamp": timestamp + 900_000, "close": 101.0},
                ]
            },
            "ETH-USDT-SWAP": {
                "15m": [
                    {"timestamp": timestamp, "close": 200.0},
                    {"timestamp": timestamp + 900_000, "close": 202.0},
                ]
            },
        },
    }
    original_loader = selector_runtime.load_ranked_candidate_rows

    def fake_loader(_context, _params, _timestamp, progress_callback=None):
        if callable(progress_callback):
            progress_callback("candidate_generation", "fake live progress", 0.42)
        ranked = [{
            "row": row,
            "symbol": row["symbol"],
            "side": row["side"],
            "score": row["adjusted_score"],
            "adjusted_score": row["adjusted_score"],
            "teacher_score": row["adjusted_score"],
            "teacher_adjusted_score": row["adjusted_score"],
            "teacher_risk_prob": row["teacher_risk_prob"],
        }]
        return [row], {"candidate_source": "test"}, ranked, {"model_score_count": 1}

    selector_runtime.load_ranked_candidate_rows = fake_loader
    stdout = io.StringIO()
    try:
        with contextlib.redirect_stdout(stdout):
            decision = build_with_fake_student_scores(
                context,
                streaming_params({"student_execution_probability_threshold": 0.7, "risk_probability_cap": 0.4}),
            )
    finally:
        selector_runtime.load_ranked_candidate_rows = original_loader

    events = [
        json.loads(line)
        for line in stdout.getvalue().splitlines()
        if line.strip()
    ]

    assert decision["actions"]
    assert any(
        event.get("event") == "strategy_log"
        and event.get("stage") == "candidate_generation"
        and event.get("details", {}).get("progress") == 0.42
        for event in events
    )


def test_runtime_protective_stop_trigger_aligns_to_okx_tick_size():
    timestamp = 1_700_000_000_000
    context = {
        "candles": {
            "BTC-USDT-SWAP": {"15m": [{"timestamp": timestamp, "close": 100.0}]},
            "FIL-USDT-SWAP": {"15m": [{"timestamp": timestamp, "close": 0.8854}]},
        },
        "orders": {"open": [], "recent_fills": [], "recent_rejections": []},
        "ml_trade_candidates": [
            streaming_candidate_row(timestamp, "FIL-USDT-SWAP", 20.0, 0.90, 0.10),
        ],
    }
    params = streaming_params(
        {
            "path_stop_loss_bps": 450.0,
            "_backtest_instrument_rules_by_symbol": {
                "FIL-USDT-SWAP": {
                    "instId": "FIL-USDT-SWAP",
                    "tickSz": "0.0001",
                }
            },
        }
    )

    decision = build_with_fake_student_scores(context, params)

    risk_actions = [action for action in decision["actions"] if action.get("action") == "place_risk_order"]
    assert len(risk_actions) == 1
    assert risk_actions[0]["trigger_price"] == 0.8456
    assert runtime_actions.protective_stop_price(
        100.03,
        "short",
        450.0,
        "BTC-USDT-SWAP",
        {"_backtest_instrument_rules_by_symbol": {"BTC-USDT-SWAP": {"tickSz": "0.1"}}},
    ) == 104.5


def test_streaming_gate_selects_only_probability_and_risk_qualified_candidates():
    timestamp = 1_700_000_000_000
    candles = [
        {"timestamp": timestamp, "close": 100.0},
        {"timestamp": timestamp + 900_000, "close": 101.0},
    ]
    context = {
        "candles": {
            "BTC-USDT-SWAP": {"15m": candles},
            "ETH-USDT-SWAP": {"15m": [{"timestamp": timestamp + 900_000, "close": 202.0}]},
            "SOL-USDT-SWAP": {"15m": [{"timestamp": timestamp + 900_000, "close": 52.0}]},
            "XRP-USDT-SWAP": {"15m": [{"timestamp": timestamp + 900_000, "close": 0.52}]},
        },
        "orders": {"open": [], "recent_fills": [], "recent_rejections": []},
        "ml_trade_candidates": [
            streaming_candidate_row(timestamp + 900_000, "ETH-USDT-SWAP", 20.0, 0.82, 0.20),
            streaming_candidate_row(timestamp + 900_000, "SOL-USDT-SWAP", 30.0, 0.62, 0.10),
            streaming_candidate_row(timestamp + 900_000, "XRP-USDT-SWAP", 40.0, 0.91, 0.55),
        ],
    }

    decision = build_with_fake_student_scores(
        context,
        streaming_params({"student_execution_probability_threshold": 0.7, "risk_probability_cap": 0.4}),
    )

    open_actions = [action for action in decision["actions"] if action.get("action") == "open_position"]
    assert len(open_actions) == 1
    assert open_actions[0]["symbol"] == "ETH-USDT-SWAP"
    gate = decision["diagnostics"]["streaming_gate"]
    assert gate["selection_rule"] == "streaming_teacher_gate_v1"
    assert gate["input_count"] == 3
    assert gate["selected_count"] == 1
    assert gate["blocked_by_reason"]["student_probability_below_threshold"] == 1
    assert gate["blocked_by_reason"]["teacher_risk_probability_above_cap"] == 1
    assert decision["indicators"]["student_execution_prob"] == [0.82]
    assert "student_top3_prob" not in decision["indicators"]
    assert decision["indicators"]["teacher_risk_prob"] == [0.2]


def test_streaming_gate_holds_when_daily_open_cap_is_reached():
    timestamp = 1_700_000_000_000
    candles = [
        {"timestamp": timestamp, "close": 100.0},
        {"timestamp": timestamp + 900_000, "close": 101.0},
    ]
    context = {
        "candles": {
            "BTC-USDT-SWAP": {"15m": candles},
            "ETH-USDT-SWAP": {"15m": [{"timestamp": timestamp + 900_000, "close": 202.0}]},
        },
        "orders": {
            "open": [],
            "recent_fills": [
                {"order_id": "o-1", "action": "open_position", "status": "filled", "success": True, "timestamp": timestamp},
                {"order_id": "o-2", "action": "open_position", "status": "filled", "success": True, "timestamp": timestamp + 1},
                {"order_id": "o-3", "action": "open_position", "status": "filled", "success": True, "timestamp": timestamp + 2},
            ],
            "recent_rejections": [],
        },
        "ml_trade_candidates": [
            streaming_candidate_row(timestamp + 900_000, "ETH-USDT-SWAP", 20.0, 0.95, 0.10),
        ],
    }

    decision = build_with_fake_student_scores(context, streaming_params())

    assert decision["actions"] == []
    assert decision["diagnostics"]["action_intent"] == "hold"
    gate = decision["diagnostics"]["streaming_gate"]
    assert gate["daily_open_count"] == 3
    assert gate["daily_open_remaining"] == 0
    assert gate["blocked_by_reason"]["daily_open_cap_reached"] == 1


def test_streaming_gate_blocks_candidate_for_existing_position_symbol():
    timestamp = 1_700_000_000_000
    candles = [
        {"timestamp": timestamp, "close": 100.0},
        {"timestamp": timestamp + 900_000, "close": 101.0},
    ]
    context = {
        "candles": {
            "BTC-USDT-SWAP": {"15m": candles},
            "ETH-USDT-SWAP": {"15m": [{"timestamp": timestamp + 900_000, "close": 202.0}]},
        },
        "positions": {
            "open": [
                {
                    "symbol": "ETH-USDT-SWAP",
                    "instId": "ETH-USDT-SWAP",
                    "posSide": "long",
                    "quantity": 1.0,
                }
            ]
        },
        "orders": {"open": [], "recent_fills": [], "recent_rejections": []},
        "ml_trade_candidates": [
            streaming_candidate_row(timestamp + 900_000, "ETH-USDT-SWAP", 20.0, 0.95, 0.10),
        ],
    }

    decision = build_with_fake_student_scores(context, streaming_params())

    assert decision["actions"] == []
    assert decision["diagnostics"]["action_intent"] == "hold"
    gate = decision["diagnostics"]["streaming_gate"]
    assert gate["blocked_by_reason"]["symbol_already_active"] == 1


def test_streaming_gate_does_not_fallback_to_top_k_when_student_artifact_is_missing():
    timestamp = 1_700_000_000_000
    candles = [
        {"timestamp": timestamp, "close": 100.0},
        {"timestamp": timestamp + 900_000, "close": 101.0},
    ]
    context = {
        "candles": {
            "BTC-USDT-SWAP": {"15m": candles},
            "ETH-USDT-SWAP": {"15m": [{"timestamp": timestamp + 900_000, "close": 202.0}]},
        },
        "orders": {"open": [], "recent_fills": [], "recent_rejections": []},
        "ml_trade_candidates": [
            {
                "symbol": "ETH-USDT-SWAP",
                "side": "long",
                "timestamp": timestamp + 900_000,
                "adjusted_score": 100.0,
                "student_top3_prob": 0.99,
                "teacher_risk_prob": 0.01,
            },
            {
                "symbol": "SOL-USDT-SWAP",
                "side": "long",
                "timestamp": timestamp + 900_000,
                "adjusted_score": 90.0,
                "student_top3_prob": 0.98,
                "teacher_risk_prob": 0.01,
            },
            {
                "symbol": "XRP-USDT-SWAP",
                "side": "short",
                "timestamp": timestamp + 900_000,
                "adjusted_score": 80.0,
                "student_top3_prob": 0.97,
                "teacher_risk_prob": 0.01,
            },
        ],
    }

    decision = build_runtime_decision(
        "ml_trade_selector_forward_candidate_v1",
        context,
        streaming_params(
            {
                "student_selector_model_file": "missing_student_selector.joblib",
                "student_selector_required": False,
                "top_k": 3,
            }
        ),
        {"symbol": "BTC-USDT-SWAP", "timeframe": "15m"},
    )

    assert decision["actions"] == []
    gate = decision["diagnostics"]["streaming_gate"]
    assert gate["student_model_status"] == "student_selector_artifact_missing"
    assert gate["blocked_by_reason"]["student_selector_artifact_missing"] == 3


def test_streaming_gate_ignores_incoming_student_probability_fields():
    timestamp = 1_700_000_000_000
    candles = [
        {"timestamp": timestamp, "close": 100.0},
        {"timestamp": timestamp + 900_000, "close": 101.0},
    ]
    context = {
        "candles": {
            "BTC-USDT-SWAP": {"15m": candles},
            "ETH-USDT-SWAP": {"15m": [{"timestamp": timestamp + 900_000, "close": 202.0}]},
        },
        "orders": {"open": [], "recent_fills": [], "recent_rejections": []},
        "ml_trade_candidates": [
            {
                **streaming_candidate_row(timestamp + 900_000, "ETH-USDT-SWAP", 20.0, 0.20, 0.10),
                "student_top3_prob": 0.99,
                "teacher_top3_prob": 0.99,
                "p_teacher_top3": 0.99,
            }
        ],
    }

    decision = build_with_fake_student_scores(
        context,
        streaming_params({"student_execution_probability_threshold": 0.7}),
    )

    assert decision["actions"] == []
    gate = decision["diagnostics"]["streaming_gate"]
    assert gate["selected_count"] == 0
    assert gate["blocked_by_reason"]["student_probability_below_threshold"] == 1


def test_runtime_removes_old_history_top_k_selection_path():
    params = PRODUCTION_RUNTIME_CONFIG["params"]

    assert not hasattr(selector_runtime, "build_runtime_history_decision")
    assert not hasattr(candidate_ranking, "daily_top_ranked_by_timestamp")
    assert not hasattr(runtime_actions, "ranked_decision")
    assert "top_k" not in params
    assert "student_selector_enabled" not in params
    assert "student_selector_required" not in params
    assert student_selector.student_probability_threshold({"student_top3_probability_threshold": 0.99}) == 0.5


def test_model_risk_penalty_prefers_artifact_metadata():
    assert model_risk_penalty_bps({"risk_penalty_bps": 20.0}) == 20.0
    assert model_risk_penalty_bps({}) == 18.0


def test_protective_stop_reason_matches_configured_stop_bps():
    assert protective_stop_reason(450.0) == "protective_stop_450bps"
    assert protective_stop_reason(600.0) == "protective_stop_600bps"


def test_production_strategy_risk_cap_allows_effective_position_size():
    params = PRODUCTION_RUNTIME_CONFIG["params"]
    rank_weights = [float(value) for value in params["rank_weights"]]
    effective_position_size = max(
        float(PRODUCTION_RUNTIME_CONFIG["position_size"]),
        max(rank_weights) * float(params["global_weight"]),
    )

    assert params["position_size_mode"] == "notional"
    assert params["risk_control_enabled"] is True
    assert params["require_stop_loss"] is True
    assert params["max_symbol_exposure_pct"] > 0.2
    assert params["max_symbol_exposure_pct"] >= effective_position_size
    assert params["student_target"] == "net_positive"
    assert params["target_trades_per_day"] == 0.8
    assert params["student_execution_probability_threshold"] == 0.772435


def test_student_gate_exposes_over_precise_probability_parameters_for_audit():
    warnings = student_selector.probability_parameter_precision_warnings(
        {
            "student_execution_probability_threshold": 0.772435,
            "risk_probability_cap": 0.362968,
        }
    )
    warning_keys = {warning["key"] for warning in warnings}

    assert warning_keys == {
        "student_execution_probability_threshold",
        "risk_probability_cap",
    }
    assert all(
        warning["reason"] == "over_precise_probability_parameter_requires_oos_review"
        for warning in warnings
    )
    assert student_selector.probability_parameter_precision_warnings(
        {
            "student_execution_probability_threshold": 0.77,
            "risk_probability_cap": 0.36,
        }
    ) == []


def test_production_research_audit_is_exposed_without_changing_actions():
    timestamp = 1_700_000_000_000
    context = {
        "candles": {
            "BTC-USDT-SWAP": {
                "15m": [
                    {"timestamp": timestamp, "close": 100.0},
                    {"timestamp": timestamp + 900_000, "close": 101.0},
                ]
            }
        }
    }
    original_loader = selector_runtime.load_ranked_candidate_rows

    def fake_empty_loader(_context, _params, _timestamp, progress_callback=None):
        del progress_callback
        return [], {"candidate_source": "unit_test", "blocked_by": ["no_candidate"]}, [], {}

    selector_runtime.load_ranked_candidate_rows = fake_empty_loader
    try:
        decision = build_runtime_decision(
            "ml_trade_selector_forward_candidate_v1",
            context,
            PRODUCTION_RUNTIME_CONFIG["params"],
            PRODUCTION_RUNTIME_CONFIG,
        )
    finally:
        selector_runtime.load_ranked_candidate_rows = original_loader

    assert decision["actions"] == []
    audit = decision["diagnostics"]["strategy_research_audit"]
    assert audit["status"] == "review_required"
    assert "over_precise_probability_parameters" in audit["known_limitations"]
    assert "no_clean_final_oos_window" in audit["known_limitations"]
    assert audit["artifact"]["clean_final_oos"] is False
    warning_keys = {item["key"] for item in audit["parameter_precision_warnings"]}
    assert warning_keys == {"student_execution_probability_threshold", "risk_probability_cap"}
    assert any(log["stage"] == "strategy_audit" and log["level"] == "warn" for log in decision["execution_logs"])


def test_production_strategy_resource_limits_match_i9_laptop_profile():
    params = PRODUCTION_RUNTIME_CONFIG["params"]

    assert params["memory_budget_gb"] == 14.0
    assert params["memory_budget_reserve_gb"] == 1.0
    assert params["parallel_base_layer_max_workers"] == 12
    assert params["parallel_universe_max_workers"] == 12
    assert params["parallel_model_batch_max_workers"] == 10
    assert params["parallel_model_batch_estimator_jobs"] == 2
    assert params["model_score_chunk_rows"] == 50000
    assert candidate_ranking.rank_cache_chunk_size(params) == 12288


def test_memory_limited_worker_count_uses_non_proc_rss_fallback():
    original_current_rss = resource_limits.current_rss_gb
    try:
        resource_limits.current_rss_gb = lambda: 3.25
        workers = resource_limits.memory_limited_worker_count(
            {
                "memory_budget_gb": 6.0,
                "memory_budget_reserve_gb": 1.0,
                "worker_memory_gb": 1.0,
            },
            configured_workers=8,
            item_count=100,
            worker_memory_key="worker_memory_gb",
            default_worker_memory_gb=1.0,
        )
    finally:
        resource_limits.current_rss_gb = original_current_rss

    assert workers == 1


def test_resource_rss_fallback_handles_macos_byte_units():
    original_getrusage = resource_limits.resource.getrusage
    original_platform = resource_limits.sys.platform

    class Usage:
        ru_maxrss = 2.0 * 1024.0 * 1024.0 * 1024.0

    try:
        resource_limits.resource.getrusage = lambda _scope: Usage()
        resource_limits.sys.platform = "darwin"
        rss_gb = resource_limits._current_rss_gb_from_resource()
    finally:
        resource_limits.resource.getrusage = original_getrusage
        resource_limits.sys.platform = original_platform

    assert abs(rss_gb - 2.0) < 1e-9


def test_model_candidate_batch_scoring_shares_timestamp_and_aggregate_paths():
    items = [
        {
            "row": {"feature_a": 1.0},
            "symbol": "BTC-USDT-SWAP",
            "side": "long",
            "timestamp": 1_700_000_000_000,
            "rank_order": 1,
        },
        {
            "row": {"feature_a": 2.0},
            "symbol": "ETH-USDT-SWAP",
            "side": "short",
            "timestamp": 1_700_000_000_000,
            "rank_order": 2,
        },
        {
            "row": {},
            "symbol": "SOL-USDT-SWAP",
            "side": "long",
            "timestamp": 1_700_000_900_000,
            "rank_order": 3,
        },
    ]
    bundle = {
        "available": True,
        "feature_columns": ["feature_a"],
        "numeric_feature_columns": {"feature_a"},
    }
    params = {
        "fast_model_dataframe_from_records": False,
        "score_context_penalty_bps": 0.0,
    }
    progress_events = []
    original_score = candidate_ranking.score_model_items_serial_chunked_components

    def fake_score(complete, _model_rows, _bundle, _params, _fast_frame, progress_callback=None):
        if callable(progress_callback):
            progress_callback(len(complete), len(complete))
        return [
            {
                "score": float(item["row"]["feature_a"]) * 10.0,
                "return_pred": float(item["row"]["feature_a"]),
                "risk_prob": 0.1,
                "risk_penalty_bps": 0.0,
                "score_source": "unit_test_fake_model",
            }
            for item in complete
        ]

    candidate_ranking.score_model_items_serial_chunked_components = fake_score
    try:
        aggregate_report = candidate_ranking.empty_score_report()
        aggregate_selected = candidate_ranking.score_model_candidate_batch(
            items,
            bundle,
            params,
            aggregate_report,
        )
        timestamp_reports = {}
        selected_by_timestamp = {}
        candidate_ranking.score_model_candidate_batch_by_timestamp(
            items,
            bundle,
            params,
            timestamp_reports,
            selected_by_timestamp,
            progress_callback=lambda stage, message, progress: progress_events.append(
                (stage, message, progress)
            ),
        )
    finally:
        candidate_ranking.score_model_items_serial_chunked_components = original_score

    assert [item["adjusted_score"] for item in aggregate_selected] == [10.0, 20.0]
    assert aggregate_report["model_score_count"] == 2
    assert aggregate_report["skipped_missing_score_count"] == 1
    assert aggregate_report["skipped_by_reason"]["candidate_feature_columns_missing"] == 1

    selected_at_ts = selected_by_timestamp[1_700_000_000_000]
    assert [item["adjusted_score"] for item in selected_at_ts] == [10.0, 20.0]
    assert timestamp_reports[1_700_000_000_000]["model_score_count"] == 2
    assert timestamp_reports[1_700_000_900_000]["skipped_missing_score_count"] == 1
    assert (
        timestamp_reports[1_700_000_900_000]["skipped_by_reason"][
            "candidate_feature_columns_missing"
        ]
        == 1
    )
    assert progress_events[-1][0] == "model_scoring"
    assert progress_events[-1][2] == 1.0


def test_order_from_ranked_item_derives_planned_exit_time_from_context_candles():
    timestamp = 1_700_000_000_000
    item = ranked_item_without_exit_time(timestamp)
    context = {
        "candles": {
            "ETH-USDT-SWAP": {
                "15m": [
                    {"timestamp": timestamp, "close": 200.0},
                    {"timestamp": timestamp + 900_000, "close": 202.0},
                    {"timestamp": timestamp + 1_800_000, "close": 204.0},
                ]
            }
        }
    }

    order = order_from_ranked_item(
        item,
        1,
        context,
        timestamp,
        100.0,
        {"global_weight": 0.72, "rank_weight": 0.35},
        600.0,
        20.0,
    )

    assert order["planned_exit_time"] == timestamp + 1_800_000
    assert order["planned_exit_reason"] == "max_hold_bars"
    assert order["planned_hold_bars"] == 2
    assert order["hold_bars"] == 2


def test_order_from_ranked_item_derives_planned_exit_time_from_entry_time_when_future_candle_missing():
    timestamp = 1_700_000_000_000
    item = ranked_item_without_exit_time(timestamp)

    order = order_from_ranked_item(
        item,
        1,
        {},
        timestamp,
        100.0,
        {"global_weight": 0.72, "rank_weight": 0.35},
        600.0,
        20.0,
    )

    assert order["planned_exit_time"] == timestamp + 1_800_000
    assert order["planned_exit_reason"] == "max_hold_bars"


def assert_planned_exit_contract(action, timestamp):
    assert action["planned_exit_time"] == timestamp + 900_000
    assert action["planned_exit_reason"] == "max_hold_bars"
    assert "exit_time" not in action
    assert "exit_reason" not in action
    assert "action_contract_version" not in action
    assert action["planned_hold_bars"] == 40
    assert action["hold_bars"] == 40
    assert action["entry_index"] == 1
    assert action["source_index"] == 0
    assert action["entry_time"] == timestamp
    assert action["source_time"] == timestamp - 900_000
    assert action["feature_bar_time"] == timestamp - 900_000
    assert "signal_index" not in action
    assert "signal_time" not in action
    assert action["layer_id"] == "contract_test_layer"
    assert action["family"] == "contract_test_family"
    assert action["timeframe"] == "15m"
    assert action["candidate_source"] == "contract_test_source"
    assert action["candidate_entry_price"] == 201.0


def assert_json_number(action, key):
    value = action.get(key)
    assert isinstance(value, (int, float)) and not isinstance(value, bool), (
        f"{action.get('action')}.{key} must be a JSON number, got {value!r}"
    )


def ranked_item(timestamp):
    return {
        "symbol": "ETH-USDT-SWAP",
        "side": "long",
        "score": 12.0,
        "adjusted_score": 10.0,
        "row": candidate_row(timestamp),
    }


def ranked_item_without_exit_time(timestamp):
    return {
        "symbol": "ETH-USDT-SWAP",
        "side": "long",
        "score": 12.0,
        "adjusted_score": 10.0,
        "row": {
            "symbol": "ETH-USDT-SWAP",
            "side": "long",
            "timestamp": timestamp,
            "adjusted_score": 10.0,
            "planned_hold_bars": 2,
            "hold_bars": 2,
            "entry_index": 0,
            "signal_index": 0,
            "entry_time": timestamp,
            "signal_time": timestamp - 900_000,
            "feature_bar_time": timestamp - 900_000,
            "layer_id": "fallback_exit_layer",
            "family": "fallback_exit_family",
            "timeframe": "15m",
            "candidate_source": "fallback_exit_source",
            "entry_price": 201.0,
        },
    }


def candidate_row(timestamp):
    return {
        "symbol": "ETH-USDT-SWAP",
        "side": "long",
        "timestamp": timestamp,
        "adjusted_score": 10.0,
        "exit_time": timestamp + 900_000,
        "exit_reason": "max_hold_bars",
        "planned_hold_bars": 40,
        "hold_bars": 40,
        "entry_index": 1,
        "signal_index": 0,
        "entry_time": timestamp,
        "signal_time": timestamp - 900_000,
        "feature_bar_time": timestamp - 900_000,
        "layer_id": "contract_test_layer",
        "family": "contract_test_family",
        "timeframe": "15m",
        "candidate_source": "contract_test_source",
        "entry_price": 201.0,
    }


def streaming_candidate_row(timestamp, symbol, score, probability, risk_probability):
    row = candidate_row(timestamp)
    row["symbol"] = symbol
    row["adjusted_score"] = score
    row["mock_student_probability"] = probability
    row["teacher_risk_prob"] = risk_probability
    row["risk_prob"] = risk_probability
    return row


def build_with_fake_student_scores(context, params):
    return with_fake_student_scores(
        lambda: build_runtime_decision(
            "ml_trade_selector_forward_candidate_v1",
            context,
            params,
            {"symbol": "BTC-USDT-SWAP", "timeframe": "15m"},
        )
    )


def with_fake_student_scores(callback):
    original_load = student_selector.load_student_selector_bundle
    original_score = student_selector.score_student_selector_items

    def fake_load(_params):
        return {
            "available": True,
            "status": "student_selector_loaded",
            "model_path": "fake_student_selector.joblib",
            "contract_path": "fake_student_selector_contract.json",
            "feature_columns": ["mock_student_probability"],
            "numeric_feature_columns": {"mock_student_probability"},
        }

    def fake_score(_bundle, items, report):
        for item in items:
            row = item.get("row") if isinstance(item.get("row"), dict) else {}
            probability = row.get("mock_student_probability")
            if probability is None:
                report["blocked_by_reason"]["student_selector_features_missing"] = (
                    report["blocked_by_reason"].get("student_selector_features_missing", 0) + 1
                )
                continue
            student_selector.set_student_execution_probability(item, probability)
            item["student_score_source"] = "fake_student_selector_v1"

    student_selector.load_student_selector_bundle = fake_load
    student_selector.score_student_selector_items = fake_score
    try:
        return callback()
    finally:
        student_selector.load_student_selector_bundle = original_load
        student_selector.score_student_selector_items = original_score


def streaming_params(extra=None):
    params = {
        "selection_rule": "streaming_teacher_gate_v1",
        "daily_open_cap": 3,
        "per_eval_open_cap": 3,
        "student_execution_probability_threshold": 0.7,
        "risk_probability_cap": 0.4,
        "global_weight": 0.72,
        "rank_weight": 0.35,
        "path_stop_loss_bps": 600.0,
    }
    if extra:
        params.update(extra)
    return params


if __name__ == "__main__":
    test_order_from_ranked_item_preserves_planned_exit_fields()
    test_latest_decision_returns_explicit_actions_not_legacy_orders_only()
    test_production_data_requirements_expose_state_used_by_streaming_gate()
    test_live_runtime_emits_internal_progress_strategy_logs()
    test_runtime_protective_stop_trigger_aligns_to_okx_tick_size()
    test_streaming_gate_selects_only_probability_and_risk_qualified_candidates()
    test_streaming_gate_holds_when_daily_open_cap_is_reached()
    test_streaming_gate_blocks_candidate_for_existing_position_symbol()
    test_streaming_gate_does_not_fallback_to_top_k_when_student_artifact_is_missing()
    test_streaming_gate_ignores_incoming_student_probability_fields()
    test_runtime_removes_old_history_top_k_selection_path()
    test_model_risk_penalty_prefers_artifact_metadata()
    test_protective_stop_reason_matches_configured_stop_bps()
    test_production_strategy_risk_cap_allows_effective_position_size()
    test_student_gate_exposes_over_precise_probability_parameters_for_audit()
    test_production_research_audit_is_exposed_without_changing_actions()
    test_production_strategy_resource_limits_match_i9_laptop_profile()
    test_memory_limited_worker_count_uses_non_proc_rss_fallback()
    test_resource_rss_fallback_handles_macos_byte_units()
    test_model_candidate_batch_scoring_shares_timestamp_and_aggregate_paths()
    test_order_from_ranked_item_derives_planned_exit_time_from_context_candles()
    test_order_from_ranked_item_derives_planned_exit_time_from_entry_time_when_future_candle_missing()
