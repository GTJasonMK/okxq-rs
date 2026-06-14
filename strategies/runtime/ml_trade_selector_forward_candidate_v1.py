"""Forward runtime entrypoint for the 2026-06-07 trade-level ML selector.

The executable strategy is deliberately separated from the research platform:
this file and `strategies/lib/ml_trade_selector_runtime.py` are runtime-owned
copies/contracts. They must not import from `scripts/research`.
"""

from __future__ import annotations

import sys
from pathlib import Path


_STRATEGIES_ROOT = Path(__file__).resolve().parents[1]
if str(_STRATEGIES_ROOT) not in sys.path:
    sys.path.insert(0, str(_STRATEGIES_ROOT))

from lib.ml_trade_selector_runtime import (  # noqa: E402
    FROZEN_CANDIDATE,
    UNIVERSE_SYMBOLS,
    build_runtime_decision,
    empty_indicators,
)


CORE_5M_SYMBOLS = ("BTC-USDT-SWAP", "ETH-USDT-SWAP", "SOL-USDT-SWAP", "XRP-USDT-SWAP")
MIN_BARS_15M = 10_000
MIN_BARS_5M = 4_500

STRATEGY_ID = "ml_trade_selector_forward_candidate_v1"
STRATEGY_NAME = "ML Trade Selector Forward Candidate V1"
STRATEGY_DESCRIPTION = (
    "Frozen 2026-06-07 RF leaf100 bad-penalty-18 trade selector contract with additive "
    "weak-state score penalty, streaming teacher-gate selection, and 450bps path stop. "
    "Runtime candidate generation is strategy-owned and must live under strategies/lib."
)
STRATEGY_TYPE = "multi_symbol_selector"

RUNTIME_CONFIG = {
    "symbol": "BTC-USDT-SWAP",
    "inst_type": "SWAP",
    "timeframe": "15m",
    "risk_timeframe": "1m",
    "initial_capital": 1000,
    "position_size": 0.35,
    "stop_loss": 0.045,
    "take_profit": 0.0,
    "check_interval": 60,
    "mode": "simulated",
    "params": {
        "contract_mode": True,
        "leverage": 3,
        "position_size_mode": "notional",
        "max_leverage": 5,
        "risk_control_enabled": True,
        # 35% strategy notional plus execution precision tolerance.
        "max_symbol_exposure_pct": 0.36,
        "backtest_instrument_rules_source": "okx",
        "require_stop_loss": True,
        "selection_rule": "streaming_teacher_gate_v1",
        "target_trades_per_day": 0.8,
        "daily_open_cap": 3,
        "per_eval_open_cap": 3,
        "student_target": "net_positive",
        "student_selector_model_file": "student_selector_v1.joblib",
        "student_selector_contract_file": "student_selector_contract.json",
        "student_execution_probability_threshold": 0.772435,
        "risk_probability_cap": 0.362968,
        "block_existing_symbol_exposure": True,
        "global_weight": 1.0,
        "rank_weights": [0.35, 0.35, 0.35, 0.35, 0.35],
        "unique_symbol_selection": True,
        "score_context_penalty_bps": 40.0,
        "path_stop_loss_bps": 450.0,
        "max_slippage_bps": 20.0,
        "allow_external_ml_candidates": True,
        "model_artifact_required": True,
        "strict_context_gating": False,
        "require_btc_context": False,
        "require_market_context": False,
        "require_funding_context": False,
        "min_market_context_count": 20,
        "model_artifact_dir": "artifacts/ml_trade_selector_forward_candidate_v1",
        "candidate_generator_owner": "strategies/lib/ml_trade_selector_runtime.py",
        "history_candidate_warmup_days": 3,
        "runtime_candidate_layers": [
            "v20_loose",
            "spread_velocity",
            "universe_candidate_v1",
            "reversion_long",
            "dual_calendar",
            "v9",
        ],
        "runtime_candidate_generation_status": "base_layers_copied_structure_and_sample_row_parity_pass",
        "universe_candidate_modes": "breakout,fade",
        "universe_candidate_holds": "32,72",
        "memory_budget_gb": 14.0,
        "memory_budget_reserve_gb": 1.0,
        "parallel_base_layer_worker_memory_gb": 1.0,
        "parallel_universe_worker_memory_gb": 1.0,
        "parallel_model_batch_worker_memory_gb": 1.0,
        "memory_limited_model_prediction": True,
        "memory_limited_model_estimator_jobs": True,
        "model_score_chunk_rows": 50000,
        "backtest_candidate_rank_cache": True,
        "backtest_candidate_rank_cache_chunk_size": 12288,
        "model_estimator_jobs": 2,
        "parallel_universe_candidate_generation": True,
        "parallel_universe_min_timestamps": 256,
        "parallel_universe_max_workers": 12,
        "parallel_base_layer_generation": True,
        "parallel_base_layer_enrichment": True,
        "parallel_base_layer_min_timestamps": 256,
        "parallel_base_layer_max_workers": 12,
        "parallel_model_prediction": False,
        "parallel_model_min_rows": 10000,
        "parallel_model_max_rows": 100000,
        "parallel_model_outer_jobs": 1,
        "parallel_model_estimator_jobs": 1,
        "parallel_model_batch_scoring": True,
        "parallel_model_batch_min_rows": 1000,
        "parallel_model_batch_max_workers": 10,
        "parallel_model_batch_estimator_jobs": 2,
        "fast_model_dataframe_from_records": True,
        "min_rank_samples": 1000,
        "rank_lookback": 5000,
        "correlation_groups": {
            "crypto_swaps": UNIVERSE_SYMBOLS,
        },
    },
}

DATA_REQUIREMENTS = {
    "symbols": UNIVERSE_SYMBOLS,
    "timeframes": ["15m", "5m", "1m"],
    "candles": [
        *[
            {
                "symbol": symbol,
                "inst_type": "SWAP",
                "timeframe": "15m",
                "min_bars": MIN_BARS_15M,
                "role": "ml_candidate_context",
            }
            for symbol in UNIVERSE_SYMBOLS
        ],
        *[
            {
                "symbol": symbol,
                "inst_type": "SWAP",
                "timeframe": "5m",
                "min_bars": MIN_BARS_5M,
                "role": "ml_candidate_context",
            }
            for symbol in CORE_5M_SYMBOLS
        ],
    ],
    "funding": [
        {"symbol": symbol, "inst_type": "SWAP", "history_limit": 10000, "required": True}
        for symbol in UNIVERSE_SYMBOLS
    ],
    "orderbook": False,
    "account": {"required": False},
    "positions": {"required": True},
    "orders": {"open": True, "recent_fills": True, "recent_rejections": False},
}

VISUALIZATION = {
    "primary_price_series": "close",
    "indicator_series": [
        {"key": "candidate_score", "label": "ML candidate score", "unit": "bps"},
        {"key": "adjusted_score", "label": "Context-adjusted score", "unit": "bps"},
        {"key": "student_execution_prob", "label": "Student execution probability", "unit": "ratio"},
        {"key": "teacher_risk_prob", "label": "Teacher very-bad probability", "unit": "ratio"},
        {"key": "rank_weight", "label": "Rank weight", "unit": "ratio"},
    ],
    "diagnostics": [
        "candidate_count",
        "selected_count",
        "candidate_source",
        "blocked_by",
        "streaming_gate",
        "frozen_candidate",
    ],
}

DECISION_CONTRACT = {
    "action_schema_version": 1,
    "actions": ["open_position", "place_risk_order", "hold"],
    "entry_sides": ["long", "short"],
    "hold_sides": ["hold"],
    "reason_codes": [
        "ml_ranked_candidate",
        "protective_stop_450bps",
        "strategy_candidate_generator_not_implemented",
        "missing_ml_candidate_context",
        "no_ranked_candidate",
        "model_artifact_missing",
        "model_contract_missing",
        "model_dependency_missing",
        "candidate_feature_columns_missing",
        "student_selector_artifact_missing",
        "student_selector_contract_missing",
        "student_selector_dependency_missing",
        "student_selector_load_failed",
        "student_selector_contract_forbidden_features",
        "student_selector_features_missing",
        "student_selector_score_failed",
        "student_selector_score_missing",
        "student_probability_below_threshold",
        "teacher_risk_probability_above_cap",
        "daily_open_cap_reached",
        "btc_context_incomplete",
        "market_context_incomplete",
        "funding_context_incomplete",
        "model_score_failed",
        "runtime_candidate_layers_generated",
        "no_universe_candidate_signal",
        "no_runtime_candidate_signal",
        "no_base_layer_signal",
    ],
}


def evaluate(context, params):
    return build_runtime_decision(STRATEGY_ID, context, params, RUNTIME_CONFIG)


def compute_indicators(candles, params):
    del params
    return empty_indicators(len(candles))
