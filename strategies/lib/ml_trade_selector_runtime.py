"""Runtime-only helpers for the 2026-06-07 ML trade selector.

This module is intentionally owned by `strategies/`. It may contain copied
logic from research reports/scripts after promotion, but it must not import
from `scripts/research`. The runtime strategy should depend on stable copied
contracts and artifacts, not on mutable research entrypoints.
"""

from __future__ import annotations

import json
import math

from lib.ml_trade_candidate_ranking import (
    load_ranked_candidate_rows,
)
from lib.ml_trade_model_scoring import (
    FROZEN_CANDIDATE,
    GENERATOR_STATUS_NOT_READY,
    artifact_dir,
    num,
)
from lib.ml_trade_runtime_actions import (
    action_lifecycle_contract,
    entry_action,
    hold_decision,
    order_from_ranked_item,
    protective_stop_price,
    protective_stop_reason,
    risk_close_side,
    risk_order_action,
)
from lib.ml_trade_runtime_progress import backtest_progress
from lib.ml_trade_student_selector import (
    apply_streaming_teacher_gate,
    probability_parameter_precision_warnings,
)
from lib.runtime_logging import emit_execution_log, execution_log


_ARTIFACT_AUDIT_CACHE = {}


def primary_candles(context, runtime_config):
    if not isinstance(context, dict):
        return []
    runtime = context.get("runtime") if isinstance(context.get("runtime"), dict) else {}
    symbol = runtime.get("symbol") or runtime_config["symbol"]
    timeframe = runtime.get("timeframe") or runtime_config["timeframe"]
    candles = context.get("candles")
    if not isinstance(candles, dict):
        return []
    by_timeframe = candles.get(symbol)
    if not isinstance(by_timeframe, dict):
        return []
    rows = by_timeframe.get(timeframe)
    return rows if isinstance(rows, list) else []


def context_timestamp(context, candles):
    time_context = context.get("time") if isinstance(context, dict) else {}
    if isinstance(time_context, dict):
        timestamp = num(time_context.get("timestamp"), None)
        if timestamp is not None:
            return int(timestamp)
    if candles:
        return int(candles[-1].get("timestamp", 0) or 0)
    return 0


def build_runtime_decision(strategy_id, context, params, runtime_config):
    candles = primary_candles(context, runtime_config)
    timestamp = context_timestamp(context, candles)
    price = num(candles[-1].get("close"), 0.0) if candles else 0.0
    progress = backtest_progress(context, "candidate_selection", "Evaluating ML selector.")
    strategy_audit = research_audit_snapshot(params)
    logs = [
        execution_log(
            "strategy_input",
            f"ML selector: evaluating latest context at {timestamp}",
            "info",
            {
                "strategy_id": strategy_id,
                "primary_candle_count": len(candles),
                "timestamp": timestamp,
            },
        )
    ]
    emit_execution_log(context, "strategy_input", logs[-1]["message"], "info", logs[-1]["details"])
    append_research_audit_log(context, logs, strategy_audit)
    rows, source_info, ranked, score_report = load_ranked_candidate_rows(
        context,
        params,
        timestamp,
        progress_callback=runtime_progress_logger(context),
    )
    if not rows:
        logs.append(
            execution_log(
                "candidate_generation",
                "ML selector: no candidate rows available, hold",
                "warn",
                {
                    "candidate_source": source_info.get("candidate_source"),
                    "blocked_by": source_info.get("blocked_by", [GENERATOR_STATUS_NOT_READY]),
                },
            )
        )
        emit_execution_log(context, "candidate_generation", logs[-1]["message"], "warn", logs[-1]["details"])
        extra = {
            "candidate_count": 0,
            "selected_count": 0,
            "candidate_source": source_info.get("candidate_source"),
            "strategy_research_audit": strategy_audit,
        }
        if progress is not None:
            extra["backtest_progress"] = progress
        decision = hold_decision(
            strategy_id,
            source_info.get("blocked_by", [GENERATOR_STATUS_NOT_READY]),
            source_info.get("summary", "No candidate rows available."),
            extra,
        )
        decision["execution_logs"] = logs
        return decision

    if not ranked:
        blocked_by = ["no_ranked_candidate", *sorted(score_report.get("skipped_by_reason", {}).keys())]
        logs.append(
            execution_log(
                "model_scoring",
                "ML selector: candidates were present but no row passed scoring checks",
                "warn",
                {
                    "candidate_count": len(rows),
                    "blocked_by": blocked_by,
                    "candidate_source": source_info.get("candidate_source"),
                },
            )
        )
        emit_execution_log(context, "model_scoring", logs[-1]["message"], "warn", logs[-1]["details"])
        extra = {
            "candidate_count": len(rows),
            "selected_count": 0,
            "candidate_source": source_info.get("candidate_source"),
            "scoring": score_report,
            "strategy_research_audit": strategy_audit,
        }
        if progress is not None:
            extra["backtest_progress"] = progress
        decision = hold_decision(
            strategy_id,
            blocked_by,
            "Candidate rows were present, but no row passed filters and scoring checks.",
            extra,
        )
        decision["execution_logs"] = logs
        return decision

    ranked, streaming_gate_report = apply_streaming_teacher_gate(context, params, ranked, timestamp)
    if not ranked:
        blocked_by = sorted((streaming_gate_report or {}).get("blocked_by_reason", {}).keys())
        if not blocked_by:
            blocked_by = ["student_gate_no_candidate"]
        logs.append(
            execution_log(
                "student_selection_gate",
                "ML selector: streaming student gate selected no candidates",
                "info",
                {
                    "candidate_count": len(rows),
                    "ranked_count": (streaming_gate_report or {}).get("input_count", 0),
                    "blocked_by": blocked_by,
                    "streaming_gate": streaming_gate_report,
                },
            )
        )
        emit_execution_log(
            context,
            "student_selection_gate",
            logs[-1]["message"],
            "info",
            logs[-1]["details"],
        )
        extra = {
            "candidate_count": len(rows),
            "selected_count": 0,
            "candidate_source": source_info.get("candidate_source"),
            "scoring": score_report,
            "streaming_gate": streaming_gate_report,
            "strategy_research_audit": strategy_audit,
        }
        if progress is not None:
            extra["backtest_progress"] = progress
        decision = hold_decision(
            strategy_id,
            blocked_by,
            "Ranked candidates were present, but streaming student gate selected no executable candidate.",
            extra,
        )
        decision["execution_logs"] = logs
        return decision

    stop_bps = num(params.get("path_stop_loss_bps"), FROZEN_CANDIDATE["path_stop_loss_bps"])
    max_slippage_bps = num(params.get("max_slippage_bps"), 20.0)
    orders = []
    risk_actions = []
    score_series = []
    adjusted_series = []
    weight_series = []
    student_probability_series = []
    teacher_risk_series = []
    for rank, item in enumerate(ranked, 1):
        symbol = item["symbol"]
        order = order_from_ranked_item(
            item,
            rank,
            context,
            timestamp,
            price,
            params,
            stop_bps,
            max_slippage_bps,
        )
        orders.append(order)
        risk_actions.append(
            {
                "symbol": symbol,
                "side": risk_close_side(order["side"]),
                "order_type": "stop_market",
                "trigger_price": protective_stop_price(
                    order["price"],
                    order["side"],
                    stop_bps,
                    symbol,
                    params,
                ),
                "stop_loss_bps": stop_bps,
                "max_slippage_bps": max_slippage_bps,
                "reason": protective_stop_reason(stop_bps),
                "timestamp": timestamp,
            }
        )
        score_series.append(item["score"])
        adjusted_series.append(item["adjusted_score"])
        student_probability_series.append(num(item.get("student_execution_prob"), None))
        teacher_risk_series.append(num(item.get("teacher_risk_prob"), None))
        weight_series.append(order["position_size"])
    actions = [entry_action(order) for order in orders]
    actions.extend(risk_order_action(order) for order in risk_actions)
    action_intent = "open_position" if orders else "hold"

    diagnostics = {
        "strategy_id": strategy_id,
        "action_intent": action_intent,
        "action_count": len(actions),
        "open_action_count": len(orders),
        "risk_action_count": len(risk_actions),
        "side": "multi",
        "candidate_count": len(rows),
        "selected_count": len(orders),
        "candidate_source": source_info.get("candidate_source"),
        "scoring": score_report,
        "streaming_gate": streaming_gate_report,
        "selected_symbols": [item["symbol"] for item in ranked],
        "path_stop_loss_bps": stop_bps,
        "frozen_candidate": FROZEN_CANDIDATE,
        "summary": "Selected ML candidate rows using streaming teacher gate.",
        "blocked_by": [],
        "action_contract": action_lifecycle_contract(actions),
        "strategy_research_audit": strategy_audit,
    }
    if progress is not None:
        diagnostics["backtest_progress"] = progress
    logs.append(
        execution_log(
            "candidate_selection",
            f"ML selector: selected {len(orders)} ranked candidates",
            "success" if orders else "info",
            {
                "candidate_count": len(rows),
                "selected_count": len(orders),
                "selected_symbols": [item["symbol"] for item in ranked],
                "candidate_source": source_info.get("candidate_source"),
                "streaming_gate": streaming_gate_report,
            },
        )
    )
    emit_execution_log(context, "candidate_selection", logs[-1]["message"], logs[-1]["level"], logs[-1]["details"])

    return {
        "actions": actions,
        "diagnostics": diagnostics,
        "indicators": {
            "candidate_score": score_series,
            "adjusted_score": adjusted_series,
            "student_execution_prob": student_probability_series,
            "teacher_risk_prob": teacher_risk_series,
            "rank_weight": weight_series,
        },
        "execution_logs": logs,
    }


def research_audit_snapshot(params):
    """Expose research-risk evidence without changing live trading decisions."""

    manifest_info = artifact_manifest_audit(params)
    precision_warnings = probability_parameter_precision_warnings(params)
    known_limitations = []
    if precision_warnings:
        known_limitations.append("over_precise_probability_parameters")
    if manifest_info.get("clean_final_oos") is False:
        known_limitations.append("no_clean_final_oos_window")
    if manifest_info.get("audit_window_consumed") is True:
        known_limitations.append("audit_window_consumed_by_promotion")
    if manifest_info.get("manifest_loaded") is False:
        known_limitations.append("artifact_manifest_unavailable")

    return {
        "status": "review_required" if known_limitations else "ok",
        "parameter_precision_warnings": precision_warnings,
        "known_limitations": known_limitations,
        "artifact": manifest_info,
        "split_contract": manifest_info.get("split_contract") or FROZEN_CANDIDATE.get("split_contract"),
    }


def artifact_manifest_audit(params):
    directory = artifact_dir(params)
    manifest_path = directory / "manifest.json"
    cache_key = str(manifest_path)
    manifest = _ARTIFACT_AUDIT_CACHE.get(cache_key)
    if manifest is None:
        try:
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            manifest = {}
        _ARTIFACT_AUDIT_CACHE[cache_key] = manifest

    split_contract = manifest.get("split_contract") if isinstance(manifest, dict) else {}
    production_selection = manifest.get("production_selection") if isinstance(manifest, dict) else {}
    if not isinstance(split_contract, dict):
        split_contract = {}
    if not isinstance(production_selection, dict):
        production_selection = {}
    return {
        "manifest_loaded": bool(manifest),
        "manifest_path": str(manifest_path),
        "source_experiment_id": manifest.get("source_experiment_id") if isinstance(manifest, dict) else None,
        "clean_final_oos": split_contract.get("clean_final_oos"),
        "audit_window_consumed": "consumed_audit_2025_09_2026_06" in production_selection,
        "promotion_gate": production_selection.get("promotion_gate"),
        "split_contract": split_contract,
    }


def append_research_audit_log(context, logs, strategy_audit):
    if not isinstance(strategy_audit, dict) or strategy_audit.get("status") != "review_required":
        return
    limitations = strategy_audit.get("known_limitations") or []
    log = execution_log(
        "strategy_audit",
        "ML selector: research audit warnings are present; live actions are unchanged.",
        "warn",
        {
            "known_limitations": limitations,
            "parameter_precision_warning_count": len(
                strategy_audit.get("parameter_precision_warnings") or []
            ),
            "clean_final_oos": (strategy_audit.get("artifact") or {}).get("clean_final_oos"),
        },
    )
    logs.append(log)
    emit_execution_log(context, log["stage"], log["message"], log["level"], log["details"])


def runtime_progress_logger(context):
    def report(stage, message, progress):
        assert isinstance(progress, (int, float)) and not isinstance(progress, bool), (
            "runtime progress must be numeric"
        )
        assert math.isfinite(progress), "runtime progress must be finite"
        assert 0.0 <= progress <= 1.0, "runtime progress must be between 0 and 1"
        emit_execution_log(
            context,
            stage,
            message,
            "info",
            {
                "progress": progress,
                "source": "ml_trade_selector_runtime",
            },
        )

    return report
