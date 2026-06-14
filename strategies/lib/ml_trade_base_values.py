"""Shared value, parameter, and candle helpers for runtime base layers."""

from __future__ import annotations

from datetime import datetime
import math

from lib.ml_trade_base_calendars import BJ_TZ


def context_candles(context, symbol, timeframe):
    candles = context.get("candles") if isinstance(context, dict) else {}
    by_timeframe = candles.get(symbol) if isinstance(candles, dict) else {}
    rows = by_timeframe.get(timeframe) if isinstance(by_timeframe, dict) else []
    return rows if isinstance(rows, list) else []


def close_values(candles):
    return [finite_float(item.get("close")) or 0.0 for item in candles]


def bjt_weekday_hour(candle):
    timestamp = int(candle.get("timestamp", 0) or 0)
    if timestamp <= 0:
        return 0, 0
    dt = datetime.fromtimestamp(timestamp / 1000, BJ_TZ)
    return dt.weekday(), dt.hour


def in_calendar_list(calendar, coin, weekday, hour):
    for weekdays, hours in calendar.get(coin, []):
        if weekday in weekdays and hour in hours:
            return True
    return False


def in_dual_calendar(calendar, coin, weekday, hour):
    for cal_weekday, hours in calendar.get(coin, []):
        if weekday == cal_weekday and hour in hours:
            return True
    return False


def runtime_timeframe(params):
    return str(params.get("_runtime_timeframe") or params.get("timeframe") or "15m").strip().lower()


def runtime_coin(params):
    value = params.get("_runtime_symbol") or params.get("symbol") or params.get("inst_id") or ""
    coin = str(value).strip().upper().split("-")[0]
    return coin or None


def asset_key(symbol):
    return str(symbol).split("-")[0].lower()


def symbolic_list(value):
    if isinstance(value, (list, tuple)):
        return [str(item).strip() for item in value if str(item).strip()]
    return [item.strip() for item in str(value or "").split(",") if item.strip()]


def normalized_side(signal):
    side = str(signal.get("side", "") if isinstance(signal, dict) else "").strip().lower()
    if side in {"sell", "short"}:
        return "short"
    if side in {"buy", "long"}:
        return "long"
    return ""


def value_at(indicators, key, index):
    values = indicators.get(key)
    if not isinstance(values, list) or index < 0 or index >= len(values):
        return None
    value = values[index]
    return float(value) if finite(value) else None


def int_param(params, key, default):
    try:
        return max(1, int(params.get(key, default)))
    except (TypeError, ValueError):
        return max(1, int(default))


def num_param(params, key, default):
    try:
        parsed = float(params.get(key, default))
        return parsed if math.isfinite(parsed) else float(default)
    except (TypeError, ValueError):
        return float(default)


def bool_param(params, key, default):
    value = params.get(key, default)
    if isinstance(value, bool):
        return value
    return str(value).strip().lower() in {"1", "true", "yes", "on"}


def finite(value):
    if isinstance(value, float):
        return math.isfinite(value)
    if isinstance(value, int):
        return True
    try:
        return math.isfinite(float(value))
    except (TypeError, ValueError):
        return False


def finite_float(value):
    if value is None:
        return None
    if isinstance(value, float):
        return value if math.isfinite(value) else None
    if isinstance(value, int):
        parsed = float(value)
        return parsed if math.isfinite(parsed) else None
    if value == "":
        return None
    try:
        parsed = float(value)
    except (TypeError, ValueError):
        return None
    return parsed if math.isfinite(parsed) else None


def round_float(value, digits=6):
    if value is None:
        return None
    if isinstance(value, float):
        return round(value, digits) if math.isfinite(value) else None
    if isinstance(value, int):
        parsed = float(value)
        return round(parsed, digits) if math.isfinite(parsed) else None
    parsed = finite_float(value)
    return None if parsed is None else round(parsed, digits)
