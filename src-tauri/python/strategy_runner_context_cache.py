"""Process-local context cache for repeated runtime strategy evaluations."""

from __future__ import annotations

import bisect


_CONTEXT_CACHE = {}


def _int_value(value, default=0):
    try:
        return int(value)
    except (TypeError, ValueError):
        return default


def _row_timestamp(row, key="timestamp"):
    if not isinstance(row, dict):
        return 0
    return _int_value(row.get(key) or row.get("timestamp") or row.get("funding_time"))


def _indexed_candles(context):
    indexed = {}
    candles = context.get("candles") if isinstance(context, dict) else {}
    if not isinstance(candles, dict):
        return indexed
    for symbol, timeframe_map in candles.items():
        if not isinstance(timeframe_map, dict):
            continue
        for timeframe, rows in timeframe_map.items():
            if not isinstance(rows, list):
                continue
            indexed[(str(symbol), str(timeframe))] = {
                "rows": rows,
                "timestamps": [_row_timestamp(row, "timestamp") for row in rows],
            }
    return indexed


def _indexed_funding(context):
    indexed = {}
    funding = context.get("funding") if isinstance(context, dict) else {}
    if not isinstance(funding, dict):
        return indexed
    for symbol, payload in funding.items():
        if not isinstance(payload, dict):
            continue
        history = payload.get("history")
        if not isinstance(history, list):
            history = []
        try:
            history_limit = int(payload.get("_history_limit") or 0)
        except (TypeError, ValueError):
            history_limit = 0
        indexed[str(symbol)] = {
            "payload": payload,
            "history": history,
            "timestamps": [_row_timestamp(row, "funding_time") for row in history],
            "history_limit": max(0, history_limit),
        }
    return indexed


def cache_context(command: dict) -> dict:
    context_id = str(command.get("context_id") or "").strip()
    context = command.get("context")
    if not context_id:
        return {"ok": False, "error": "context_id is required"}
    if not isinstance(context, dict):
        return {"ok": False, "error": "context must be an object"}

    candles = _indexed_candles(context)
    funding = _indexed_funding(context)
    runtime_cache = {
        "context_id": context_id,
        "static_context": context,
        "candles": candles,
        "funding": funding,
        "indicator_cache": {},
        "feature_context_cache": {},
    }
    _CONTEXT_CACHE[context_id] = {
        "context": context,
        "candles": candles,
        "funding": funding,
        "runtime_cache": runtime_cache,
    }
    return {
        "ok": True,
        "context_id": context_id,
        "candle_series": len(candles),
        "funding_series": len(funding),
    }


def cached_context_at_timestamp(context_id: str, timestamp: int) -> dict:
    cached = _CONTEXT_CACHE.get(str(context_id or ""))
    if cached is None:
        raise RuntimeError(f"strategy context cache not found: {context_id}")

    timestamp = int(timestamp or 0)
    base = dict(cached["context"])
    candle_tree = {}
    for (symbol, timeframe), payload in cached["candles"].items():
        end = bisect.bisect_right(payload["timestamps"], timestamp)
        symbol_entry = candle_tree.setdefault(symbol, {})
        symbol_entry[timeframe] = payload["rows"][:end]
    base["candles"] = candle_tree

    funding_tree = {}
    for symbol, payload in cached["funding"].items():
        end = bisect.bisect_right(payload["timestamps"], timestamp)
        limit = payload.get("history_limit") or 0
        start = max(0, end - limit) if limit > 0 else 0
        history = payload["history"][start:end]
        next_payload = {
            key: value
            for key, value in payload["payload"].items()
            if key != "_history_limit"
        }
        next_payload["history"] = history
        next_payload["latest"] = history[-1] if history else {}
        funding_tree[symbol] = next_payload
    base["funding"] = funding_tree

    time_context = base.get("time")
    time_context = dict(time_context) if isinstance(time_context, dict) else {}
    if timestamp > 0:
        time_context["timestamp"] = timestamp
    base["time"] = time_context
    base["_runtime_cache"] = cached["runtime_cache"]
    return base
