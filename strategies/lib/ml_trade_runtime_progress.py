"""Progress event helpers for ML trade selector runtime evaluation."""

from __future__ import annotations

import json
import math
import sys


def backtest_progress(context, stage, fallback_message, label="ML selector"):
    if not isinstance(context, dict):
        return None
    payload = context.get("backtest")
    if not isinstance(payload, dict):
        return None
    processed = progress_int(payload.get("processed_candles"), "processed_candles")
    total = progress_int(payload.get("total_candles"), "total_candles")
    progress = payload.get("progress")
    assert isinstance(progress, (int, float)) and not isinstance(progress, bool), (
        "backtest progress must be numeric"
    )
    assert math.isfinite(progress), "backtest progress must be finite"
    assert 0.0 <= progress <= 1.0, "backtest progress must be between 0 and 1"
    message = str(payload.get("message") or fallback_message)
    if processed > 0 and total > 0:
        message = f"{label}: {processed}/{total} candles"
    return {
        "progress": progress,
        "stage": str(payload.get("stage") or stage),
        "message": message,
        "processed_candles": processed,
        "total_candles": total,
    }

def backtest_progress_events_enabled(context):
    if not isinstance(context, dict):
        return False
    payload = context.get("backtest")
    return isinstance(payload, dict) and bool(payload.get("progress_events"))

def emit_history_progress(
    context,
    progress,
    stage,
    message,
    processed_candles=None,
    total_candles=None,
    extra=None,
):
    if not backtest_progress_events_enabled(context):
        return
    assert isinstance(progress, (int, float)) and not isinstance(progress, bool), (
        "history progress must be numeric"
    )
    assert math.isfinite(progress), "history progress must be finite"
    assert 0.0 <= progress <= 1.0, "history progress must be between 0 and 1"
    event = {
        "event": "progress",
        "progress": progress,
        "stage": str(stage),
        "message": str(message),
    }
    strategy_progress = {
        "progress": progress,
        "stage": event["stage"],
        "message": event["message"],
    }
    total = progress_int(total_candles, "total_candles") if total_candles is not None else 0
    processed = (
        progress_int(processed_candles, "processed_candles")
        if processed_candles is not None
        else 0
    )
    if total > 0:
        assert processed <= total, "processed_candles must not exceed total_candles"
        event["processed"] = processed
        event["total"] = total
        strategy_progress["processed_candles"] = processed
        strategy_progress["total_candles"] = total
    if isinstance(extra, dict):
        strategy_progress.update(extra)
    event["strategy_progress"] = strategy_progress
    sys.stdout.write(json.dumps(event, default=str) + "\n")
    sys.stdout.flush()

def progress_int(value, field="progress"):
    assert isinstance(value, int) and not isinstance(value, bool), f"{field} must be an integer"
    assert value >= 0, f"{field} must be non-negative"
    return value
