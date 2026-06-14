"""Progress and strategy-log event helpers for strategy runner commands."""

import json as _json
import math as _math
import sys as _sys


def _progress_events_enabled(args: dict) -> bool:
    return bool(args.get("progress_events"))


def _strategy_log_events_enabled(args: dict) -> bool:
    return bool(args.get("strategy_log_events"))


def _progress_emit_step(total: int) -> int:
    assert total > 0, "progress total must be positive"
    return max(1, total // 100)


def _should_emit_progress(index: int, total: int) -> bool:
    assert total > 0, "progress total must be positive"
    assert 0 <= index < total, "progress index must be within total"
    processed = index + 1
    return processed == 1 or processed == total or processed % _progress_emit_step(total) == 0


def _progress_fraction(value) -> float | None:
    if value is None:
        return None
    assert isinstance(value, (int, float)) and not isinstance(value, bool), (
        "strategy progress must be numeric"
    )
    assert _math.isfinite(value), "strategy progress must be finite"
    assert 0.0 <= value <= 1.0, "strategy progress must be between 0 and 1"
    return value


def _strategy_progress_payload(diagnostics):
    if not isinstance(diagnostics, dict):
        return None
    for key in ("backtest_progress", "task_progress", "run_progress"):
        if key in diagnostics:
            return diagnostics.get(key)
    return None


def _args_with_backtest_progress(args: dict, index: int, total: int) -> dict:
    next_args = dict(args)
    assert total >= 0, "backtest progress total must be non-negative"
    assert total == 0 or 0 <= index < total, "backtest progress index must be within total"
    processed = index + 1 if total > 0 else 0
    progress = (processed / total) if total > 0 else 0.0
    next_args["backtest"] = {
        "processed_candles": processed,
        "total_candles": total,
        "progress": progress,
        "stage": "strategy",
        "message": f"Strategy evaluation {processed}/{total} candles",
    }
    return next_args


def _emit_compute_progress(args: dict, index: int, total: int, diagnostics=None) -> None:
    if not _progress_events_enabled(args) or total <= 0:
        return
    assert 0 <= index < total, "progress index must be within total"
    processed = index + 1
    strategy_progress = _strategy_progress_payload(diagnostics)
    progress = None
    stage = "strategy"
    message = f"执行策略 {processed}/{total} 根K线"
    if isinstance(strategy_progress, dict):
        progress = _progress_fraction(strategy_progress.get("progress"))
        stage = str(strategy_progress.get("stage") or stage)
        message = str(strategy_progress.get("message") or message)

    if progress is None:
        progress = processed / total
    event = {
        "event": "progress",
        "stage": stage,
        "message": message,
        "progress": progress,
        "processed": processed,
        "total": total,
    }
    if strategy_progress is not None:
        event["strategy_progress"] = strategy_progress
    _sys.stdout.write(_json.dumps(event, default=str) + "\n")
    _sys.stdout.flush()
