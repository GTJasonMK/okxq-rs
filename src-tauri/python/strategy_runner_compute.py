"""Compute command implementation for the strategy runner."""

from strategy_runner_context import _build_strategy_context
from strategy_runner_decision import (
    _decision_from_evaluate,
    _decision_from_strategy_decision,
)
from strategy_runner_metadata import _load_module, _runtime_params
from strategy_runner_progress import (
    _args_with_backtest_progress,
    _emit_compute_progress,
    _progress_events_enabled,
    _should_emit_progress,
)


def _compute_evaluate_latest(module, config: dict, candles: list, params: dict, args: dict) -> dict:
    context = _build_strategy_context(
        config,
        candles,
        _args_with_backtest_progress(args, max(0, len(candles) - 1), len(candles)),
    )
    decision = _decision_from_evaluate(module, context, params, config)
    return {
        "ok": True,
        "actions": decision["actions"],
        "decision": {
            "actions": decision["actions"],
            "diagnostics": decision["diagnostics"],
            "execution_logs": decision["execution_logs"],
        },
        "execution_logs": decision["execution_logs"],
        "indicators": decision["indicators"],
        "diagnostics": decision["diagnostics"],
    }


def _compute_evaluate_history(module, config: dict, candles: list, params: dict, args: dict) -> dict:
    evaluate_history = getattr(module, "evaluate_history", None)
    raw_context = args.get("context")
    context_ref = str(args.get("context_ref") or "").strip()
    context_provides_candles = (
        isinstance(raw_context, dict) and isinstance(raw_context.get("candles"), dict)
    ) or bool(context_ref)
    if callable(evaluate_history) and context_provides_candles:
        progress_args = _args_with_backtest_progress(args, max(0, len(candles) - 1), len(candles))
        context = _build_strategy_context(config, candles, progress_args)
        decision = evaluate_history(context, params, candles)
        latest_decision = _decision_from_strategy_decision(decision, config)
        if _progress_events_enabled(args):
            _emit_compute_progress(progress_args, max(0, len(candles) - 1), len(candles), latest_decision["diagnostics"])
        return {
            "ok": True,
            "actions": latest_decision["actions"],
            "decision": {
                "actions": latest_decision["actions"],
                "diagnostics": latest_decision["diagnostics"],
                "execution_logs": latest_decision["execution_logs"],
            },
            "execution_logs": latest_decision["execution_logs"],
            "indicators": latest_decision["indicators"],
            "diagnostics": latest_decision["diagnostics"],
        }

    actions = []
    latest_decision = {
        "actions": [],
        "indicators": {},
        "diagnostics": {},
        "execution_logs": [],
    }
    for index in range(len(candles)):
        window = [candles[index]] if context_provides_candles else candles[: index + 1]
        progress_args = _args_with_backtest_progress(args, index, len(candles))
        context = _build_strategy_context(config, window, progress_args)
        latest_decision = _decision_from_evaluate(module, context, params, config)
        actions.extend(latest_decision["actions"])
        if _should_emit_progress(index, len(candles)):
            _emit_compute_progress(progress_args, index, len(candles), latest_decision["diagnostics"])
    return {
        "ok": True,
        "actions": actions,
        "decision": {
            "actions": actions,
            "diagnostics": latest_decision["diagnostics"],
            "execution_logs": latest_decision["execution_logs"],
        },
        "execution_logs": latest_decision["execution_logs"],
        "indicators": latest_decision["indicators"],
        "diagnostics": latest_decision["diagnostics"],
    }


def _compute(args: dict) -> dict:
    """Execute strategy compute for the given candles."""
    file_path = args["file_path"]
    config = args.get("config", {})
    candles = args.get("candles", [])

    module = _load_module(file_path)
    params = _runtime_params(module, config)
    evaluate = getattr(module, "evaluate", None)
    compute_scope = str(config.get("compute_scope") or args.get("compute_scope") or "latest").lower()

    if callable(evaluate):
        if compute_scope == "history":
            return _compute_evaluate_history(module, config, candles, params, args)
        return _compute_evaluate_latest(module, config, candles, params, args)

    return {
        "ok": False,
        "error": f"策略文件中未找到 evaluate 函数: {file_path}",
    }
