"""Canonical runtime context construction for evaluate()."""

from strategy_runner_progress import (
    _progress_events_enabled,
    _strategy_log_events_enabled,
)
from strategy_runner_context_cache import cached_context_at_timestamp
from strategy_runner_values import _last_candle, _numeric


def _context_section(args: dict, key: str, default):
    value = args.get(key)
    if isinstance(default, dict):
        return value if isinstance(value, dict) else default.copy()
    if isinstance(default, list):
        return value if isinstance(value, list) else list(default)
    return value if value is not None else default


def _normalize_orders_context(orders) -> dict:
    if not isinstance(orders, dict):
        orders = {}
    normalized = dict(orders)
    if not isinstance(normalized.get("open"), list):
        normalized["open"] = []
    if not isinstance(normalized.get("recent_fills"), list):
        normalized["recent_fills"] = []
    if not isinstance(normalized.get("recent_rejections"), list):
        normalized["recent_rejections"] = []
    return normalized


def _orders_context(args: dict) -> dict:
    return _normalize_orders_context(args.get("orders"))


def _context_at_timestamp(raw_context: dict, timestamp: int) -> dict:
    context = raw_context.copy()
    candles = raw_context.get("candles")
    if isinstance(candles, dict) and timestamp > 0:
        trimmed_candles = {}
        for symbol, timeframe_map in candles.items():
            if not isinstance(timeframe_map, dict):
                continue
            trimmed_timeframes = {}
            for timeframe, rows in timeframe_map.items():
                if not isinstance(rows, list):
                    continue
                trimmed_timeframes[timeframe] = [
                    row
                    for row in rows
                    if isinstance(row, dict)
                    and int(row.get("timestamp", 0) or 0) <= timestamp
                ]
            trimmed_candles[symbol] = trimmed_timeframes
        context["candles"] = trimmed_candles

    funding = raw_context.get("funding")
    if isinstance(funding, dict) and timestamp > 0:
        context["funding"] = _funding_context_at_timestamp(funding, timestamp)

    time_context = context.get("time")
    if isinstance(time_context, dict):
        time_context = time_context.copy()
    else:
        time_context = {}
    if timestamp > 0:
        time_context["timestamp"] = timestamp
    context["time"] = time_context
    return context


def _context_already_at_timestamp(raw_context: dict, timestamp: int) -> bool:
    if timestamp <= 0:
        return False
    time_context = raw_context.get("time")
    if not isinstance(time_context, dict):
        return False
    raw_timestamp = _numeric(time_context.get("timestamp"))
    return raw_timestamp is not None and int(raw_timestamp) == timestamp


def _funding_context_at_timestamp(funding: dict, timestamp: int) -> dict:
    trimmed = {}
    for symbol, payload in funding.items():
        if not isinstance(payload, dict):
            continue
        history = payload.get("history")
        if isinstance(history, list):
            trimmed_history = [
                row
                for row in history
                if isinstance(row, dict)
                and int(row.get("funding_time", row.get("timestamp", 0)) or 0) <= timestamp
            ]
            next_payload = payload.copy()
            next_payload["history"] = trimmed_history
            next_payload["latest"] = trimmed_history[-1] if trimmed_history else {}
            trimmed[symbol] = next_payload
            continue

        latest = payload.get("latest")
        if isinstance(latest, dict):
            funding_time = int(latest.get("funding_time", latest.get("timestamp", 0)) or 0)
            if funding_time <= timestamp:
                trimmed[symbol] = payload
            else:
                next_payload = payload.copy()
                next_payload["latest"] = {}
                trimmed[symbol] = next_payload
    return trimmed


def _context_with_ref(args: dict, timestamp: int):
    context_ref = str(args.get("context_ref") or "").strip()
    if not context_ref:
        return args.get("context")
    cached = cached_context_at_timestamp(context_ref, timestamp)
    overlay = args.get("context")
    if isinstance(overlay, dict):
        merged = cached.copy()
        merged.update(overlay)
        if "candles" not in overlay:
            merged["candles"] = cached.get("candles", {})
        if "funding" not in overlay:
            merged["funding"] = cached.get("funding", {})
        if "_runtime_cache" not in overlay and "_runtime_cache" in cached:
            merged["_runtime_cache"] = cached["_runtime_cache"]
        return merged
    return cached


def _build_strategy_context(config: dict, candles: list, args: dict | None = None) -> dict:
    """Build the canonical runtime context passed to evaluate()."""
    args = args or {}
    symbol = str(config.get("symbol") or "")
    inst_type = str(config.get("inst_type") or "")
    timeframe = str(config.get("timeframe") or "")
    candle = _last_candle(candles)
    timestamp = int(candle.get("timestamp", 0) or 0)
    initial_capital = _numeric(config.get("initial_capital"))

    context = {
        "candles": {symbol: {timeframe: candles}} if symbol and timeframe else {},
        "funding": _context_section(args, "funding", {}),
        "orderbook": _context_section(args, "orderbook", {}),
        "positions": _context_section(args, "positions", {}),
        "account": _context_section(
            args,
            "account",
            {"initial_capital": initial_capital} if initial_capital is not None else {},
        ),
        "orders": _orders_context(args),
        "backtest": _context_section(args, "backtest", {}),
        "time": {
            "timestamp": timestamp,
            "timeframe": timeframe,
        },
        "runtime": {
            "strategy_id": config.get("strategy_id"),
            "strategy_name": config.get("strategy_name"),
            "symbol": symbol,
            "inst_type": inst_type,
            "timeframe": timeframe,
        },
    }

    raw_context = _context_with_ref(args, timestamp)
    if isinstance(raw_context, dict):
        if not _context_already_at_timestamp(raw_context, timestamp):
            raw_context = _context_at_timestamp(raw_context, timestamp)
        merged = context.copy()
        merged.update(raw_context)
        merged.setdefault("candles", context["candles"])
        merged["orders"] = _normalize_orders_context(merged.get("orders", context["orders"]))
        merged.setdefault("runtime", context["runtime"])
        merged.setdefault("time", context["time"])
        merged.setdefault("backtest", context["backtest"])
        _attach_progress_event_flag(merged, args)
        _attach_strategy_log_event_flag(merged, args)
        return merged
    _attach_progress_event_flag(context, args)
    _attach_strategy_log_event_flag(context, args)
    return context


def _attach_progress_event_flag(context: dict, args: dict) -> None:
    if not _progress_events_enabled(args):
        return
    backtest = context.get("backtest")
    if not isinstance(backtest, dict):
        backtest = {}
    else:
        backtest = dict(backtest)
    backtest["progress_events"] = True
    context["backtest"] = backtest


def _attach_strategy_log_event_flag(context: dict, args: dict) -> None:
    if not _strategy_log_events_enabled(args):
        return
    runtime = context.get("runtime")
    if not isinstance(runtime, dict):
        runtime = {}
    else:
        runtime = dict(runtime)
    runtime["strategy_log_events"] = True
    context["runtime"] = runtime
