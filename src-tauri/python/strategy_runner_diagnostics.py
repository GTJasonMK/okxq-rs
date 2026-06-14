"""Diagnose command implementation for strategy runner decisions."""

from strategy_runner_context import _build_strategy_context
from strategy_runner_decision import _decision_from_evaluate
from strategy_runner_metadata import _load_module, _runtime_params


def _diagnose(args: dict) -> dict:
    """Diagnose the latest strategy decision."""
    file_path = args["file_path"]
    config = args["config"]
    candles = args["candles"]

    module = _load_module(file_path)
    params = _runtime_params(module, config)

    if not candles:
        actions = []
        execution_logs = []
        return {
            "ok": True,
            "actions": actions,
            "execution_logs": execution_logs,
            "indicators": {},
            "diagnostics": {
                "summary": "K 线数据为空，无法评估当前决策。",
                "blocked_by": ["empty_candles"],
            },
        }

    evaluate = getattr(module, "evaluate", None)
    if callable(evaluate):
        context = _build_strategy_context(config, candles, args)
        decision = _decision_from_evaluate(module, context, params, config)
        return {
            "ok": True,
            "actions": decision["actions"],
            "execution_logs": decision["execution_logs"],
            "indicators": decision["indicators"],
            "diagnostics": decision["diagnostics"],
        }

    return {
        "ok": False,
        "error": f"策略文件中未找到 evaluate 函数，决策诊断只支持 actions 协议: {file_path}",
    }
