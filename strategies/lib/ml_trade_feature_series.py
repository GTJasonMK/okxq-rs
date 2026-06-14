"""Reusable time-series helpers for ML trade feature construction."""

from __future__ import annotations

import bisect
from collections import deque
import math

from lib.ml_trade_feature_math import (
    finite_float,
    finite_int,
    lower_wick_ratio_from_values,
    min_required_points,
    quote_volume_from_values,
    round_float,
    upper_wick_ratio_from_values,
)

def context_cutoff_bounds(timestamps):
    if not timestamps:
        return None, None
    values = sorted({finite_int(item) for item in timestamps if finite_int(item) is not None and finite_int(item) > 0})
    if not values:
        return None, None
    return values[0] - 15 * 60_000, values[-1] - 15 * 60_000

def bounded_candle_source(candles, cutoff_start=None, cutoff_end=None, history_bars=0):
    if not isinstance(candles, list) or (cutoff_start is None and cutoff_end is None):
        return candles
    timestamps = []
    sorted_input = True
    previous = None
    for candle in candles:
        timestamp = finite_int(candle.get("timestamp") if isinstance(candle, dict) else None)
        timestamps.append(timestamp if timestamp is not None else 0)
        if previous is not None and timestamp is not None and timestamp < previous:
            sorted_input = False
        if timestamp is not None:
            previous = timestamp
    if not sorted_input:
        return candles
    start = 0
    end = len(candles)
    if cutoff_start is not None:
        start = max(0, bisect.bisect_left(timestamps, int(cutoff_start)) - max(0, int(history_bars or 0)))
    if cutoff_end is not None:
        end = min(len(candles), bisect.bisect_right(timestamps, int(cutoff_end)) + 1)
    if start >= end:
        return []
    return candles[start:end]

def precompute_lagged_returns(closes, lag):
    out = [None] * len(closes)
    for pos in range(lag, len(closes)):
        out[pos] = lagged_array_return(closes, pos, lag)
    return out

def precompute_realized_vol_from_closes(closes, window, timeframe_min):
    returns = [None] * len(closes)
    for index in range(1, len(closes)):
        prev = finite_float(closes[index - 1])
        close = finite_float(closes[index])
        if prev is not None and close is not None and prev > 0.0 and close > 0.0:
            returns[index] = close / prev - 1.0
    summary = prefix_sum_count(returns)
    out = [None] * len(closes)
    min_points = max(3, min_required_points(window))
    scale = math.sqrt(24 * 60 / max(1, timeframe_min))
    for pos in range(window, len(closes)):
        total, total_sq, count = prefix_sum_count_window(summary, pos - window + 1, pos)
        if count < min_points:
            continue
        mean = total / count
        variance = total_sq / count - mean * mean
        out[pos] = math.sqrt(max(0.0, variance)) * scale
    return out

def precompute_average_from_values(values, window):
    summary = prefix_sum_count(values, positive_only=True)
    out = [None] * len(values)
    min_points = max(3, min_required_points(window))
    for pos in range(window - 1, len(values)):
        total, _, count = prefix_sum_count_window(summary, pos - window + 1, pos)
        if count >= min_points:
            out[pos] = total / count
    return out

def precomputed_value(values, pos, fallback):
    if isinstance(values, list) and 0 <= pos < len(values):
        return values[pos]
    return fallback()

def average_from_values(values, pos, window):
    if pos < window - 1 or pos >= len(values):
        return None
    segment = [float(value) for value in values[pos - window + 1 : pos + 1] if finite_float(value) is not None and float(value) > 0.0]
    if len(segment) < max(3, min_required_points(window)):
        return None
    return sum(segment) / len(segment)

def append_finite(values, value):
    parsed = finite_float(value)
    if parsed is not None:
        values.append(parsed)

def finite_values(values):
    out = []
    for value in values:
        parsed = finite_float(value)
        if parsed is not None:
            out.append(parsed)
    return out

def rounded_or_none(value):
    return round_float(value)

def median_value(values):
    clean = sorted(finite_values(values))
    if not clean:
        return None
    mid = len(clean) // 2
    if len(clean) % 2:
        return clean[mid]
    return (clean[mid - 1] + clean[mid]) / 2.0

def positive_share(values):
    clean = finite_values(values)
    if not clean:
        return None
    return sum(1 for value in clean if value > 0.0) / len(clean)

def stdev_value(values):
    clean = finite_values(values)
    if len(clean) < 2:
        return None
    mean = sum(clean) / len(clean)
    variance = sum((value - mean) * (value - mean) for value in clean) / len(clean)
    return math.sqrt(max(0.0, variance))

def rank_percentile(value, values):
    parsed = finite_float(value)
    if parsed is None:
        return None
    clean = sorted(finite_values(values))
    return rank_percentile_from_clean(parsed, clean)

def rank_percentile_from_clean(value, clean):
    parsed = finite_float(value)
    if parsed is None:
        return None
    if not clean:
        return None
    if len(clean) == 1:
        return 0.5
    left = bisect.bisect_left(clean, parsed)
    right = bisect.bisect_right(clean, parsed)
    mid_rank = (left + right - 1) / 2.0
    return max(0.0, min(1.0, mid_rank / (len(clean) - 1)))

def btc_series_from_candles(candles, timestamps=None):
    cutoff_start, cutoff_end = context_cutoff_bounds(timestamps)
    source = bounded_candle_source(candles, cutoff_start, cutoff_end, history_bars=680)
    rows = []
    if isinstance(source, list):
        for candle in source:
            if not isinstance(candle, dict):
                continue
            timestamp = finite_int(candle.get("timestamp"))
            high = finite_float(candle.get("high"))
            low = finite_float(candle.get("low"))
            close = finite_float(candle.get("close"))
            if timestamp is None or high is None or low is None or close is None:
                continue
            rows.append((timestamp, high, low, close))
    rows.sort(key=lambda item: item[0])
    return {
        "timestamps": [item[0] for item in rows],
        "highs": [item[1] for item in rows],
        "lows": [item[2] for item in rows],
        "closes": [item[3] for item in rows],
    }

def lagged_array_return(closes, pos, lag):
    if pos < lag or pos >= len(closes):
        return None
    base = finite_float(closes[pos - lag])
    close = finite_float(closes[pos])
    if base is None or close is None or base <= 0.0:
        return None
    return close / base - 1.0

def realized_daily_vol_from_closes(closes, pos, window, timeframe_min):
    if pos < window or pos >= len(closes):
        return None
    returns = []
    for index in range(pos - window + 1, pos + 1):
        prev = finite_float(closes[index - 1])
        close = finite_float(closes[index])
        if prev is not None and close is not None and prev > 0.0 and close > 0.0:
            returns.append(close / prev - 1.0)
    if len(returns) < max(3, min_required_points(window)):
        return None
    mean = sum(returns) / len(returns)
    variance = sum((item - mean) * (item - mean) for item in returns) / len(returns)
    return math.sqrt(max(0.0, variance)) * math.sqrt(24 * 60 / max(1, timeframe_min))

def high_low_range_from_arrays(highs, lows, pos, window):
    if pos < window - 1 or pos >= len(highs) or pos >= len(lows):
        return None
    high = max(float(item) for item in highs[pos - window + 1 : pos + 1])
    low = min(float(item) for item in lows[pos - window + 1 : pos + 1])
    return high / low - 1.0 if low > 0.0 else None

def range_position_from_arrays(highs, lows, closes, pos, window):
    if pos < window - 1 or pos >= len(highs) or pos >= len(lows) or pos >= len(closes):
        return None
    high = max(float(item) for item in highs[pos - window + 1 : pos + 1])
    low = min(float(item) for item in lows[pos - window + 1 : pos + 1])
    close = finite_float(closes[pos])
    span = high - low
    return (close - low) / span if close is not None and span > 0.0 else None

def relative_return(left, right):
    if left is None or right is None:
        return None
    left_value = finite_float(left)
    right_value = finite_float(right)
    if left_value is None or right_value is None:
        return None
    return left_value - right_value

def timeframe_minutes(value):
    value = str(value).strip().lower()
    if value.endswith("m"):
        return max(1, int(value[:-1] or 1))
    if value.endswith("h"):
        return max(1, int(value[:-1] or 1) * 60)
    return 15

def bars_for_hours(timeframe_min, hours):
    return max(1, int(round(hours * 60 / max(1, timeframe_min))))

def local_series_context(candles, timeframe, cache, signal_index=None):
    key = ("local_series", id(candles), str(timeframe))
    cached = cache.get(key)
    requested_index = finite_int(signal_index)
    timeframe_min = timeframe_minutes(timeframe)
    requested_start = local_series_start_offset(candles, timeframe_min, requested_index)
    if cached is not None and local_series_covers(cached, requested_index, requested_start):
        return cached

    start_offset = requested_start
    source_candles = candles[start_offset:] if start_offset > 0 else candles
    opens = []
    highs = []
    lows = []
    closes = []
    volumes = []
    quote_volumes = []
    upper_wicks = []
    lower_wicks = []
    returns = []
    true_ranges = []
    log_closes = []
    valid_log = []

    previous_close = None
    for candle in source_candles:
        open_ = finite_float(candle.get("open")) if candle else None
        high = finite_float(candle.get("high")) if candle else None
        low = finite_float(candle.get("low")) if candle else None
        close = finite_float(candle.get("close")) if candle else None
        volume = finite_float(candle.get("volume")) if candle else None
        quote = quote_volume_from_values(finite_float(candle.get("volume_ccy")) if candle else None, volume, close)

        opens.append(open_ if open_ is not None else 0.0)
        highs.append(high if high is not None else 0.0)
        lows.append(low if low is not None else 0.0)
        closes.append(close if close is not None else 0.0)
        volumes.append(volume if volume is not None else 0.0)
        quote_volumes.append(quote if quote is not None and quote > 0.0 else None)
        upper_wicks.append(upper_wick_ratio_from_values(open_, high, close))
        lower_wicks.append(lower_wick_ratio_from_values(open_, low, close))

        if previous_close is not None and close is not None and previous_close > 0.0 and close > 0.0:
            returns.append(close / previous_close - 1.0)
        else:
            returns.append(None)

        if previous_close is not None and high is not None and low is not None and previous_close > 0.0:
            true_ranges.append(max(high - low, abs(high - previous_close), abs(low - previous_close)) / previous_close)
        else:
            true_ranges.append(None)

        if close is not None and close > 0.0:
            log_closes.append(math.log(close))
            valid_log.append(True)
        else:
            log_closes.append(0.0)
            valid_log.append(False)
        previous_close = close

    initial = local_series_prefix_initials(candles, start_offset)
    series = {
        "index_offset": start_offset,
        "timeframe_min": timeframe_min,
        "opens": opens,
        "highs": highs,
        "lows": lows,
        "closes": closes,
        "volumes": volumes,
        "quote_volumes": quote_volumes,
        "upper_wicks": upper_wicks,
        "lower_wicks": lower_wicks,
        "returns": returns,
        "true_ranges": true_ranges,
        "log_closes": log_closes,
        "valid_log": valid_log,
        "return_prefix": prefix_sum_count(
            returns,
            initial_sum=initial["return_sum"],
            initial_sum_sq=initial["return_sum_sq"],
            initial_count=initial["return_count"],
        ),
        "quote_prefix": prefix_sum_count(
            quote_volumes,
            positive_only=True,
            initial_sum=initial["quote_sum"],
            initial_sum_sq=initial["quote_sum_sq"],
            initial_count=initial["quote_count"],
        ),
        "true_range_prefix": prefix_sum_count(
            true_ranges,
            initial_sum=initial["true_range_sum"],
            initial_sum_sq=initial["true_range_sum_sq"],
            initial_count=initial["true_range_count"],
        ),
        "volume_prefix": prefix_sum(volumes, initial=initial["volume_sum"]),
        "log_prefix": prefix_sum(log_closes, initial=initial["log_sum"]),
        "log_sq_prefix": prefix_sum([value * value for value in log_closes], initial=initial["log_sum_sq"]),
        "log_index_prefix": prefix_sum(
            [(start_offset + index) * value for index, value in enumerate(log_closes)],
            initial=initial["log_index_sum"],
        ),
        "valid_log_prefix": prefix_count(valid_log),
        "upper_threshold_prefix": prefix_count([value is not None and value >= 0.008 for value in upper_wicks]),
        "lower_threshold_prefix": prefix_count([value is not None and value >= 0.008 for value in lower_wicks]),
        "rolling": {},
    }
    cache[key] = series
    return series

def local_series_covers(series, requested_index, requested_start):
    if requested_index is None:
        return True
    offset = int(series.get("index_offset", 0) or 0)
    return offset <= int(requested_start or 0) and offset <= requested_index < offset + len(series.get("closes") or [])

def local_series_start_offset(candles, timeframe_min, requested_index):
    if requested_index is None:
        return 0
    lag_72h = bars_for_hours(timeframe_min, 72)
    return max(0, min(len(candles), int(requested_index)) - lag_72h)

def local_series_prefix_initials(candles, end):
    out = {
        "return_sum": 0.0,
        "return_sum_sq": 0.0,
        "return_count": 0,
        "quote_sum": 0.0,
        "quote_sum_sq": 0.0,
        "quote_count": 0,
        "true_range_sum": 0.0,
        "true_range_sum_sq": 0.0,
        "true_range_count": 0,
        "volume_sum": 0.0,
        "log_sum": 0.0,
        "log_sum_sq": 0.0,
        "log_index_sum": 0.0,
    }
    if end <= 0:
        return out
    previous_close = None
    for index, candle in enumerate(candles[:end]):
        high = finite_float(candle.get("high")) if candle else None
        low = finite_float(candle.get("low")) if candle else None
        close = finite_float(candle.get("close")) if candle else None
        volume = finite_float(candle.get("volume")) if candle else None
        quote = quote_volume_from_values(finite_float(candle.get("volume_ccy")) if candle else None, volume, close)

        if previous_close is not None and close is not None and previous_close > 0.0 and close > 0.0:
            value = close / previous_close - 1.0
            out["return_sum"] += value
            out["return_sum_sq"] += value * value
            out["return_count"] += 1

        if quote is not None and quote > 0.0:
            out["quote_sum"] += quote
            out["quote_sum_sq"] += quote * quote
            out["quote_count"] += 1

        out["volume_sum"] += volume if volume is not None else 0.0

        if previous_close is not None and high is not None and low is not None and previous_close > 0.0:
            value = max(high - low, abs(high - previous_close), abs(low - previous_close)) / previous_close
            out["true_range_sum"] += value
            out["true_range_sum_sq"] += value * value
            out["true_range_count"] += 1

        if close is not None and close > 0.0:
            value = math.log(close)
            out["log_sum"] += value
            out["log_sum_sq"] += value * value
            out["log_index_sum"] += index * value
        previous_close = close
    return out

def prefix_sum(values, initial=0.0):
    out = [float(initial or 0.0)]
    total = float(initial or 0.0)
    for value in values:
        total += float(value or 0.0)
        out.append(total)
    return out

def prefix_count(values):
    out = [0]
    total = 0
    for value in values:
        if value:
            total += 1
        out.append(total)
    return out

def prefix_sum_count(values, positive_only=False, initial_sum=0.0, initial_sum_sq=0.0, initial_count=0):
    total = float(initial_sum or 0.0)
    total_sq = float(initial_sum_sq or 0.0)
    count = int(initial_count or 0)
    sums = [total]
    squares = [total_sq]
    counts = [count]
    for value in values:
        parsed = finite_float(value)
        if parsed is not None and (not positive_only or parsed > 0.0):
            total += parsed
            total_sq += parsed * parsed
            count += 1
        sums.append(total)
        squares.append(total_sq)
        counts.append(count)
    return {"sum": sums, "sum_sq": squares, "count": counts}

def prefix_window(prefix, start, end):
    return prefix[end + 1] - prefix[start]

def prefix_sum_count_window(summary, start, end):
    return (
        summary["sum"][end + 1] - summary["sum"][start],
        summary["sum_sq"][end + 1] - summary["sum_sq"][start],
        summary["count"][end + 1] - summary["count"][start],
    )

def series_lagged_return(series, pos, lag):
    closes = series["closes"]
    if pos < lag or pos >= len(closes):
        return None
    base = closes[pos - lag]
    close = closes[pos]
    if base <= 0.0 or close <= 0.0:
        return None
    return close / base - 1.0

def rolling_pair(series, window):
    cache = series["rolling"].setdefault(window, {})
    pair = cache.get("high_low")
    if pair is None:
        pair = (
            rolling_extreme(series["highs"], window, want_max=True),
            rolling_extreme(series["lows"], window, want_max=False),
        )
        cache["high_low"] = pair
    return pair

def rolling_wick(series, window, which):
    cache = series["rolling"].setdefault(window, {})
    key = f"{which}_wick_max"
    values = cache.get(key)
    if values is None:
        source = series["upper_wicks"] if which == "upper" else series["lower_wicks"]
        values = rolling_optional_max(source, window)
        cache[key] = values
    return values

def rolling_extreme(values, window, want_max):
    out = [None] * len(values)
    items = deque()
    for index, raw in enumerate(values):
        value = float(raw or 0.0)
        while items and items[0][0] <= index - window:
            items.popleft()
        if want_max:
            while items and items[-1][1] <= value:
                items.pop()
        else:
            while items and items[-1][1] >= value:
                items.pop()
        items.append((index, value))
        if index >= window - 1:
            out[index] = items[0][1]
    return out

def rolling_optional_max(values, window):
    out = [None] * len(values)
    items = deque()
    for index, raw in enumerate(values):
        while items and items[0][0] <= index - window:
            items.popleft()
        value = finite_float(raw)
        if value is not None:
            while items and items[-1][1] <= value:
                items.pop()
            items.append((index, value))
        if index >= window - 1 and items:
            out[index] = items[0][1]
    return out

def series_realized_vol(series, pos, window):
    if pos < window or pos >= len(series["returns"]):
        return None
    start = pos - window + 1
    total, total_sq, count = prefix_sum_count_window(series["return_prefix"], start, pos)
    if count < max(3, min_required_points(window)):
        return None
    mean = total / count
    variance = total_sq / count - mean * mean
    return math.sqrt(max(0.0, variance)) * math.sqrt(24 * 60 / max(1, series["timeframe_min"]))

def series_high_low_range(series, pos, window):
    if pos < window - 1 or pos >= len(series["closes"]):
        return None
    highs, lows = rolling_pair(series, window)
    high = highs[pos]
    low = lows[pos]
    return high / low - 1.0 if high is not None and low is not None and low > 0.0 else None

def series_range_position(series, pos, window):
    if pos < window - 1 or pos >= len(series["closes"]):
        return None
    highs, lows = rolling_pair(series, window)
    high = highs[pos]
    low = lows[pos]
    close = series["closes"][pos]
    span = high - low if high is not None and low is not None else 0.0
    return (close - low) / span if close > 0.0 and span > 0.0 else None

def series_volume_ratio(series, pos, window):
    if pos < window or pos >= len(series["volumes"]):
        return None
    baseline = prefix_window(series["volume_prefix"], pos - window, pos - 1) / window
    current = series["volumes"][pos]
    return current / baseline if baseline > 0.0 else None

def series_average_quote_volume(series, pos, window):
    if pos < window or pos >= len(series["quote_volumes"]):
        return None
    total, _, count = prefix_sum_count_window(series["quote_prefix"], pos - window, pos - 1)
    return total / count if count >= max(3, min_required_points(window)) else None

def series_true_range(series, pos):
    if pos <= 0 or pos >= len(series["true_ranges"]):
        return None
    return series["true_ranges"][pos]

def series_average_true_range(series, pos, window):
    if pos < window or pos >= len(series["true_ranges"]):
        return None
    total, _, count = prefix_sum_count_window(series["true_range_prefix"], pos - window + 1, pos)
    return total / count if count >= max(3, min_required_points(window)) else None

def series_side_path_excursion(series, pos, window, side, kind):
    if pos < window - 1 or pos >= len(series["closes"]):
        return None
    close = series["closes"][pos]
    if close <= 0.0:
        return None
    highs, lows = rolling_pair(series, window)
    high = highs[pos]
    low = lows[pos]
    if high is None or low is None:
        return None
    if side == "long":
        return max(0.0, (close - low) / close) if kind == "adverse" else max(0.0, (high - close) / close)
    if side == "short":
        return max(0.0, (high - close) / close) if kind == "adverse" else max(0.0, (close - low) / close)
    return None

def series_max_wick_ratio(series, pos, window, which):
    if pos < window - 1 or pos >= len(series["closes"]):
        return None
    return rolling_wick(series, window, which)[pos]

def series_side_wick_count(series, pos, window, side, kind):
    if pos < window - 1 or pos >= len(series["closes"]):
        return None
    if side == "long":
        prefix = series["upper_threshold_prefix"] if kind == "adverse" else series["lower_threshold_prefix"]
    elif side == "short":
        prefix = series["lower_threshold_prefix"] if kind == "adverse" else series["upper_threshold_prefix"]
    else:
        return None
    return int(prefix_window(prefix, pos - window + 1, pos))

def series_trend(series, pos, window):
    if pos < window - 1 or pos >= len(series["log_closes"]):
        return None, None
    start = pos - window + 1
    index_offset = int(series.get("index_offset", 0) or 0)
    valid_count = prefix_window(series["valid_log_prefix"], start, pos)
    if valid_count < window or valid_count < max(3, min_required_points(window)):
        return None, None
    n = window
    x_mean = (n - 1) / 2.0
    sum_y = prefix_window(series["log_prefix"], start, pos)
    sum_y_sq = prefix_window(series["log_sq_prefix"], start, pos)
    sum_abs_index_y = prefix_window(series["log_index_prefix"], start, pos)
    sum_index_y = sum_abs_index_y - (index_offset + start) * sum_y
    y_mean = sum_y / n
    denom = n * (n * n - 1) / 12.0
    if denom <= 0.0:
        return None, None
    slope = (sum_index_y - x_mean * sum_y) / denom
    intercept = y_mean - slope * x_mean
    sum_index = n * (n - 1) / 2.0
    sum_index_sq = n * (n - 1) * (2 * n - 1) / 6.0
    fitted_cross = intercept * sum_y + slope * sum_index_y
    fitted_sq = n * intercept * intercept + 2 * intercept * slope * sum_index + slope * slope * sum_index_sq
    sst = sum_y_sq - sum_y * sum_y / n
    sse = sum_y_sq - 2 * fitted_cross + fitted_sq
    quality = 0.0 if sst <= 0.0 else max(0.0, min(1.0, 1.0 - sse / sst))
    return math.exp(slope * max(1, n - 1)) - 1.0, quality

def candle_return(candles, pos, lag):
    if pos < lag or pos >= len(candles):
        return None
    base = finite_float(candles[pos - lag].get("close"))
    close = finite_float(candles[pos].get("close"))
    if base is None or close is None or base <= 0.0 or close <= 0.0:
        return None
    return close / base - 1.0

def realized_daily_vol(candles, pos, window, timeframe_min):
    if pos < window or pos >= len(candles):
        return None
    returns = []
    for index in range(pos - window + 1, pos + 1):
        prev = finite_float(candles[index - 1].get("close"))
        close = finite_float(candles[index].get("close"))
        if prev is not None and close is not None and prev > 0.0 and close > 0.0:
            returns.append(close / prev - 1.0)
    if len(returns) < max(3, min_required_points(window)):
        return None
    mean = sum(returns) / len(returns)
    variance = sum((item - mean) * (item - mean) for item in returns) / len(returns)
    return math.sqrt(max(0.0, variance)) * math.sqrt(24 * 60 / max(1, timeframe_min))

def high_low_range(candles, pos, window):
    if pos < window - 1 or pos >= len(candles):
        return None
    segment = candles[pos - window + 1 : pos + 1]
    high = max(finite_float(item.get("high")) or 0.0 for item in segment)
    low = min(finite_float(item.get("low")) or 0.0 for item in segment)
    return high / low - 1.0 if low > 0.0 else None

def range_position(candles, pos, window):
    if pos < window - 1 or pos >= len(candles):
        return None
    segment = candles[pos - window + 1 : pos + 1]
    high = max(finite_float(item.get("high")) or 0.0 for item in segment)
    low = min(finite_float(item.get("low")) or 0.0 for item in segment)
    close = finite_float(candles[pos].get("close"))
    span = high - low
    return (close - low) / span if close is not None and span > 0.0 else None

def volume_ratio(candles, pos, window):
    if pos < window or pos >= len(candles):
        return None
    previous = [finite_float(item.get("volume")) or 0.0 for item in candles[pos - window : pos]]
    baseline = sum(previous) / len(previous) if previous else 0.0
    current = finite_float(candles[pos].get("volume")) or 0.0
    return current / baseline if baseline > 0.0 else None
