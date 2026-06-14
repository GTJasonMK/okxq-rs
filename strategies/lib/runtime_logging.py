"""Runtime strategy execution log helpers.

Strategies should return structured logs through ``StrategyDecision.execution_logs``.
Long-running strategies may also emit ``strategy_log`` events when the runtime
context enables ``runtime.strategy_log_events``.
"""

from __future__ import annotations

import json
import sys
import time


def execution_log(stage, message, level="info", details=None):
    assert isinstance(stage, str) and stage.strip(), "execution log stage must be a non-empty string"
    assert isinstance(message, str) and message.strip(), (
        "execution log message must be a non-empty string"
    )
    assert level in {"info", "warn", "error", "success"}, "execution log level is invalid"
    assert isinstance(details, dict), "execution log details must be a dict"
    return {
        "stage": stage.strip(),
        "level": level,
        "message": message.strip(),
        "details": details,
    }


def strategy_log_events_enabled(context):
    runtime = context.get("runtime") if isinstance(context, dict) else {}
    return isinstance(runtime, dict) and bool(runtime.get("strategy_log_events"))


def emit_execution_log(context, stage, message, level="info", details=None):
    if not strategy_log_events_enabled(context):
        return
    entry = execution_log(stage, message, level, details)
    event = {
        "event": "strategy_log",
        "timestamp_ms": int(time.time() * 1000),
        **entry,
    }
    sys.stdout.write(json.dumps(event, default=str) + "\n")
    sys.stdout.flush()
