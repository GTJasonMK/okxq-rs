"""Shared value helpers for the strategy runner protocol."""


def _last_candle(candles: list) -> dict:
    return candles[-1] if candles and isinstance(candles[-1], dict) else {}


def _numeric(value):
    try:
        if value is None:
            return None
        parsed = float(value)
        if parsed != parsed or parsed <= -1e20 or parsed >= 1e20:
            return None
        return parsed
    except (TypeError, ValueError):
        return None


def _text_value(value):
    if value is None:
        return None
    if isinstance(value, str):
        value = value.strip()
        return value or None
    if isinstance(value, (int, float, bool)):
        return str(value)
    return None
