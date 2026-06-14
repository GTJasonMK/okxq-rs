"""Numeric candle and binning helpers for ML trade feature construction."""

from __future__ import annotations

import math

def safe_ratio(numerator, denominator):
    if numerator is None or denominator is None:
        return None
    return numerator / denominator if denominator > 0.0 and math.isfinite(numerator) and math.isfinite(denominator) else None


def log_trend_slope_and_quality(candles, pos, window):
    if pos < window - 1 or pos >= len(candles):
        return None, None
    closes = [finite_float(item.get("close")) for item in candles[pos - window + 1 : pos + 1]]
    if len(closes) < max(3, min_required_points(window)) or any(close is None or close <= 0.0 for close in closes):
        return None, None
    ys = [math.log(close) for close in closes]
    n = len(ys)
    x_mean = (n - 1) / 2.0
    y_mean = sum(ys) / n
    denom = sum((index - x_mean) ** 2 for index in range(n))
    if denom <= 0.0:
        return None, None
    slope = sum((index - x_mean) * (value - y_mean) for index, value in enumerate(ys)) / denom
    fitted = [y_mean + slope * (index - x_mean) for index in range(n)]
    sst = sum((value - y_mean) ** 2 for value in ys)
    sse = sum((value - fitted[index]) ** 2 for index, value in enumerate(ys))
    quality = 0.0 if sst <= 0.0 else max(0.0, min(1.0, 1.0 - sse / sst))
    return math.exp(slope * max(1, n - 1)) - 1.0, quality


def quote_volume(candle):
    if not candle:
        return None
    value = finite_float(candle.get("volume_ccy"))
    volume = finite_float(candle.get("volume"))
    close = finite_float(candle.get("close"))
    return quote_volume_from_values(value, volume, close)


def quote_volume_from_values(value, volume, close):
    if value is not None and value > 0.0:
        return value
    return volume * close if volume is not None and close is not None and volume > 0.0 and close > 0.0 else None


def average_quote_volume(candles, pos, window):
    if pos < window or pos >= len(candles):
        return None
    values = [quote_volume(item) for item in candles[pos - window : pos]]
    values = [item for item in values if item is not None and item > 0.0]
    return sum(values) / len(values) if len(values) >= max(3, min_required_points(window)) else None


def candle_close_location(candle):
    high = finite_float(candle.get("high")) if candle else None
    low = finite_float(candle.get("low")) if candle else None
    close = finite_float(candle.get("close")) if candle else None
    if high is None or low is None or close is None or high <= low:
        return None
    return (close - low) / (high - low)


def true_range_ratio(candles, pos):
    if pos <= 0 or pos >= len(candles):
        return None
    prev = finite_float(candles[pos - 1].get("close"))
    high = finite_float(candles[pos].get("high"))
    low = finite_float(candles[pos].get("low"))
    if prev is None or high is None or low is None or prev <= 0.0:
        return None
    return max(high - low, abs(high - prev), abs(low - prev)) / prev


def average_true_range(candles, pos, window):
    if pos < window or pos >= len(candles):
        return None
    values = [true_range_ratio(candles, index) for index in range(pos - window + 1, pos + 1)]
    values = [item for item in values if item is not None]
    return sum(values) / len(values) if len(values) >= max(3, min_required_points(window)) else None


def side_path_excursion(candles, pos, window, side, kind):
    if pos < window - 1 or pos >= len(candles):
        return None
    close = finite_float(candles[pos].get("close"))
    if close is None or close <= 0.0:
        return None
    segment = candles[pos - window + 1 : pos + 1]
    high = max(finite_float(item.get("high")) or 0.0 for item in segment)
    low = min(finite_float(item.get("low")) or 0.0 for item in segment)
    if side == "long":
        return max(0.0, (close - low) / close) if kind == "adverse" else max(0.0, (high - close) / close)
    if side == "short":
        return max(0.0, (high - close) / close) if kind == "adverse" else max(0.0, (close - low) / close)
    return None


def max_wick_ratio(candles, pos, window, which):
    if pos < window - 1 or pos >= len(candles):
        return None
    values = []
    for candle in candles[pos - window + 1 : pos + 1]:
        value = upper_wick_ratio(candle) if which == "upper" else lower_wick_ratio(candle)
        if value is not None:
            values.append(value)
    return max(values) if values else None


def side_wick_count(candles, pos, window, side, kind):
    if pos < window - 1 or pos >= len(candles):
        return None
    count = 0
    for candle in candles[pos - window + 1 : pos + 1]:
        upper = upper_wick_ratio(candle)
        lower = lower_wick_ratio(candle)
        value = side_adverse_wick(upper, lower, side) if kind == "adverse" else side_favorable_wick(upper, lower, side)
        if value is not None and value >= 0.008:
            count += 1
    return count


def min_required_points(window):
    return max(3, min(window, int(math.ceil(window * 0.5))))


def side_adjusted(value, side):
    if value is None:
        return None
    if side == "short":
        return -float(value)
    if side == "long":
        return float(value)
    return None


def upper_wick_ratio(candle):
    open_ = finite_float(candle.get("open")) if candle else None
    high = finite_float(candle.get("high")) if candle else None
    close = finite_float(candle.get("close")) if candle else None
    return upper_wick_ratio_from_values(open_, high, close)


def upper_wick_ratio_from_values(open_, high, close):
    if open_ is None or high is None or close is None or open_ <= 0.0:
        return None
    return max(0.0, high - max(open_, close)) / open_


def lower_wick_ratio(candle):
    open_ = finite_float(candle.get("open")) if candle else None
    low = finite_float(candle.get("low")) if candle else None
    close = finite_float(candle.get("close")) if candle else None
    return lower_wick_ratio_from_values(open_, low, close)


def lower_wick_ratio_from_values(open_, low, close):
    if open_ is None or low is None or close is None or open_ <= 0.0:
        return None
    return max(0.0, min(open_, close) - low) / open_


def side_adverse_wick(upper, lower, side):
    if upper is None or lower is None:
        return None
    if side == "long":
        return upper
    if side == "short":
        return lower
    return None


def side_favorable_wick(upper, lower, side):
    if upper is None or lower is None:
        return None
    if side == "long":
        return lower
    if side == "short":
        return upper
    return None


def wick_skew(left, right):
    return None if left is None or right is None else float(left) - float(right)


def candle_direction(candle):
    open_ = finite_float(candle.get("open")) if candle else None
    close = finite_float(candle.get("close")) if candle else None
    if open_ is None or close is None or open_ <= 0.0:
        return "unknown"
    body = (close - open_) / open_
    if body > 0.001:
        return "bull"
    if body < -0.001:
        return "bear"
    return "doji"


def candle_range(candle):
    open_ = finite_float(candle.get("open")) if candle else None
    high = finite_float(candle.get("high")) if candle else None
    low = finite_float(candle.get("low")) if candle else None
    if open_ is None or high is None or low is None or open_ <= 0.0:
        return None
    return (high - low) / open_


def strength_bin(value):
    value = abs(float(value or 0.0))
    if value <= 0.0:
        return "zero"
    if value < 0.25:
        return "weak"
    if value < 0.75:
        return "medium"
    if value < 1.50:
        return "strong"
    return "extreme"


def hold_bucket(hold_bars):
    if hold_bars <= 32:
        return "short"
    if hold_bars <= 72:
        return "medium"
    return "long"


def return_bin(value):
    if value is None:
        return "unknown"
    value = float(value)
    if value <= -0.08:
        return "crash"
    if value <= -0.03:
        return "down"
    if value <= -0.01:
        return "soft_down"
    if value < 0.01:
        return "flat"
    if value < 0.03:
        return "soft_up"
    if value < 0.08:
        return "up"
    return "surge"


def short_return_bin(value):
    if value is None:
        return "unknown"
    value = float(value)
    if value <= -0.03:
        return "sharp_down"
    if value <= -0.01:
        return "down"
    if value < 0.01:
        return "flat"
    if value < 0.03:
        return "up"
    return "sharp_up"


def relative_return_bin(value):
    if value is None:
        return "unknown"
    value = float(value)
    if value <= -0.06:
        return "major_lag"
    if value <= -0.02:
        return "lag"
    if value < 0.02:
        return "inline"
    if value < 0.06:
        return "lead"
    return "major_lead"


def btc48_bin(value):
    if value is None:
        return "unknown"
    value = float(value)
    if value <= -0.04:
        return "down40"
    if value <= -0.015:
        return "down15"
    if value < 0.015:
        return "flat"
    if value < 0.04:
        return "up15"
    return "up40"


def btc_regime(ret48, ret7d):
    if ret48 is None or ret7d is None:
        return "unknown"
    ret48 = float(ret48)
    ret7d = float(ret7d)
    if ret48 > 0.0 and ret7d < 0.0:
        return "bounce"
    if ret48 < 0.0 and ret7d > 0.0:
        return "pullback"
    if ret48 > 0.0 and ret7d > 0.0:
        return "up_up"
    if ret48 < 0.0 and ret7d < 0.0:
        return "down_down"
    return "flat_mixed"


def vol_bin(value):
    if value is None:
        return "unknown"
    value = float(value)
    if value < 0.025:
        return "quiet"
    if value < 0.055:
        return "normal"
    if value < 0.100:
        return "active"
    return "extreme"


def magnitude_bin(value):
    if value is None:
        return "unknown"
    value = float(value)
    if value < 0.002:
        return "tiny"
    if value < 0.006:
        return "small"
    if value < 0.015:
        return "medium"
    if value < 0.035:
        return "large"
    return "extreme"


def body_bin(candle):
    open_ = finite_float(candle.get("open")) if candle else None
    close = finite_float(candle.get("close")) if candle else None
    return magnitude_bin(abs(close - open_) / open_) if open_ is not None and close is not None and open_ > 0.0 else "unknown"


def range_bin(value):
    if value is None:
        return "unknown"
    value = float(value)
    if value < 0.01:
        return "tight"
    if value < 0.03:
        return "normal"
    if value < 0.07:
        return "wide"
    return "extreme"


def position_bin(value):
    if value is None:
        return "unknown"
    value = float(value)
    if value < 0.20:
        return "bottom"
    if value < 0.40:
        return "low"
    if value < 0.60:
        return "mid"
    if value < 0.80:
        return "high"
    return "top"


def snapshot_size_bin(count, total_symbols):
    if count <= 0 or total_symbols <= 0:
        return "unknown"
    coverage = float(count) / max(1.0, float(total_symbols))
    if count < 4 or coverage < 0.50:
        return "thin"
    if coverage < 0.80:
        return "partial"
    return "broad"


def breadth_bin(value):
    if value is None:
        return "unknown"
    value = float(value)
    if value < 0.25:
        return "weak"
    if value < 0.45:
        return "mixed_low"
    if value <= 0.55:
        return "balanced"
    if value <= 0.75:
        return "mixed_high"
    return "strong"


def dispersion_bin(value):
    if value is None:
        return "unknown"
    value = float(value)
    if value < 0.015:
        return "compressed"
    if value < 0.040:
        return "normal"
    if value < 0.080:
        return "wide"
    return "extreme"


def rank_bin(value):
    if value is None:
        return "unknown"
    value = float(value)
    if value < 0.20:
        return "bottom"
    if value < 0.40:
        return "low"
    if value <= 0.60:
        return "mid"
    if value <= 0.80:
        return "high"
    return "top"


def funding_rate_bin(value):
    if value is None:
        return "unknown"
    value = float(value)
    if value <= -0.0010:
        return "very_negative"
    if value <= -0.0003:
        return "negative"
    if value < 0.0003:
        return "neutral"
    if value < 0.0010:
        return "positive"
    return "very_positive"


def z_score_bin(value):
    if value is None:
        return "unknown"
    value = float(value)
    if value <= -2.0:
        return "low_extreme"
    if value <= -1.0:
        return "low"
    if value < 1.0:
        return "normal"
    if value < 2.0:
        return "high"
    return "high_extreme"


def funding_age_bin(value_hours):
    if value_hours is None:
        return "unknown"
    value = float(value_hours)
    if value <= 2.0:
        return "fresh"
    if value <= 8.0:
        return "same_cycle"
    if value <= 24.0:
        return "old"
    return "stale"


def funding_interval_bin(value_ms):
    if value_ms is None:
        return "unknown"
    hours = float(value_ms) / 3_600_000.0
    if hours <= 1.5:
        return "1h"
    if hours <= 2.5:
        return "2h"
    if hours <= 4.5:
        return "4h"
    if hours <= 6.5:
        return "6h"
    if hours <= 9.0:
        return "8h"
    return "irregular"


def side_funding_carry_bin(value):
    if value is None:
        return "unknown"
    value = float(value)
    if value <= -0.0010:
        return "very_costly"
    if value <= -0.0003:
        return "costly"
    if value < 0.0003:
        return "neutral"
    if value < 0.0010:
        return "favorable"
    return "very_favorable"


def volume_ratio_bin(value):
    if value is None:
        return "unknown"
    value = float(value)
    if value < 0.50:
        return "dry"
    if value < 0.80:
        return "quiet"
    if value < 1.20:
        return "normal"
    if value < 2.00:
        return "active"
    return "hot"


def wick_bin(value):
    if value is None:
        return "unknown"
    value = float(value)
    if value < 0.001:
        return "none"
    if value < 0.003:
        return "small"
    if value < 0.008:
        return "medium"
    if value < 0.020:
        return "large"
    return "spike"


def wick_skew_bin(value):
    if value is None:
        return "unknown"
    value = float(value)
    if value <= -0.015:
        return "lower_spike"
    if value <= -0.004:
        return "lower_bias"
    if value < 0.004:
        return "balanced"
    if value < 0.015:
        return "upper_bias"
    return "upper_spike"


def wick_pressure_bin(value):
    if value is None:
        return "unknown"
    value = float(value)
    if value <= -0.015:
        return "adverse_spike"
    if value <= -0.004:
        return "adverse_bias"
    if value < 0.004:
        return "balanced"
    if value < 0.015:
        return "favorable_bias"
    return "favorable_spike"


def trend_quality_bin(value):
    if value is None:
        return "unknown"
    value = float(value)
    if value < 0.15:
        return "noisy"
    if value < 0.35:
        return "weak"
    if value < 0.65:
        return "coherent"
    return "strong"


def ratio_regime_bin(value):
    if value is None:
        return "unknown"
    value = float(value)
    if value < 0.60:
        return "contracting"
    if value < 0.90:
        return "soft"
    if value < 1.20:
        return "balanced"
    if value < 1.80:
        return "expanding"
    return "shock"


def range_compression_bin(value):
    if value is None:
        return "unknown"
    value = float(value)
    if value < 0.35:
        return "compressed"
    if value < 0.60:
        return "tight"
    if value < 0.90:
        return "balanced"
    if value < 1.25:
        return "wide"
    return "expanded"


def quote_volume_bin(value):
    if value is None:
        return "unknown"
    value = float(value)
    if value < 100_000:
        return "tiny"
    if value < 500_000:
        return "thin"
    if value < 2_000_000:
        return "normal"
    if value < 10_000_000:
        return "deep"
    return "massive"


def liquidity_stress_bin(quote_avg, quote_ratio, range_over_atr):
    if quote_avg is None:
        return "unknown"
    thin = quote_avg < 500_000
    dry = quote_ratio is not None and quote_ratio < 0.50
    spiky = range_over_atr is not None and range_over_atr >= 1.80
    if thin and spiky:
        return "thin_spiky"
    if thin:
        return "thin"
    if dry and spiky:
        return "dry_spiky"
    if dry:
        return "dry"
    if spiky:
        return "spiky"
    return "normal"


def atr_ratio_bin(value):
    if value is None:
        return "unknown"
    value = float(value)
    if value < 0.60:
        return "muted"
    if value < 1.10:
        return "normal"
    if value < 1.80:
        return "stretched"
    if value < 3.00:
        return "spike"
    return "extreme"


def spike_reversal_type(side, close_location, upper, lower, range_over_atr, quote_ratio):
    if close_location is None or upper is None or lower is None or range_over_atr is None:
        return "unknown"
    high_activity = quote_ratio is None or quote_ratio >= 1.0
    if range_over_atr < 1.80 or not high_activity:
        return "no_spike"
    if side == "long":
        if upper >= 0.008 and close_location <= 0.35:
            return "adverse_upper_rejection"
        if lower >= 0.008 and close_location >= 0.65:
            return "favorable_lower_reclaim"
    if side == "short":
        if lower >= 0.008 and close_location >= 0.65:
            return "adverse_lower_rejection"
        if upper >= 0.008 and close_location <= 0.35:
            return "favorable_upper_reclaim"
    return "spike_other"


def path_excursion_bin(value):
    if value is None:
        return "unknown"
    value = float(value)
    if value < 0.003:
        return "tiny"
    if value < 0.010:
        return "small"
    if value < 0.030:
        return "medium"
    if value < 0.070:
        return "large"
    return "extreme"


def wick_cluster_bin(count, window):
    if count is None:
        return "unknown"
    if count <= 0:
        return "none"
    ratio = float(count) / max(1.0, float(window))
    if count == 1:
        return "single"
    if ratio < 0.12:
        return "cluster"
    return "dense"


def hour_bucket(hour):
    if 0 <= hour <= 7:
        return "overnight"
    if 8 <= hour <= 15:
        return "asia"
    return "us"


def bjt_session(hour):
    if 0 <= hour <= 2:
        return "funding_midnight"
    if 3 <= hour <= 7:
        return "pre_asia"
    if 8 <= hour <= 11:
        return "asia_morning"
    if 12 <= hour <= 15:
        return "asia_afternoon"
    if 16 <= hour <= 19:
        return "eu_us_overlap"
    return "us_late"


def bjt_weekpart(weekday):
    if weekday == 0:
        return "monday"
    if weekday in {1, 2, 3}:
        return "midweek"
    if weekday == 4:
        return "friday"
    return "weekend"


def funding_window(hour):
    if hour in {0, 8, 16}:
        return "funding_hour"
    if hour in {7, 15, 23}:
        return "pre_funding"
    if hour in {1, 9, 17}:
        return "post_funding"
    return "normal"


def session_transition(hour):
    if hour in {0, 8, 16}:
        return "session_open"
    if hour in {7, 15, 23}:
        return "session_close"
    return "steady"


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


def finite_int(value):
    if value is None or value == "":
        return None
    try:
        return int(value)
    except (TypeError, ValueError):
        return None


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
