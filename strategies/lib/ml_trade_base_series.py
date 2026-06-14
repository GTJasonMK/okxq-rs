"""Rolling series and rank helpers for runtime base-layer indicators."""

from __future__ import annotations

import bisect
from collections import deque
import math

from lib.ml_trade_base_values import finite

_PANDAS = None
_PANDAS_UNAVAILABLE = False


def threshold_series(values, quantile, min_samples, valid_start, lookback, floor_zero=False):
    sample = []
    out = []
    for index, value in enumerate(values):
        if index < valid_start or not finite(value):
            out.append(None)
            continue
        bisect.insort(sample, float(value))
        if lookback > 0 and index >= valid_start + lookback:
            old = values[index - lookback]
            if finite(old):
                old_pos = bisect.bisect_left(sample, float(old))
                if old_pos < len(sample) and sample[old_pos] == float(old):
                    sample.pop(old_pos)
        if len(sample) < min_samples:
            out.append(None)
            continue
        result = quantile_sorted(sample, quantile)
        out.append(max(0.0, result) if floor_zero else result)
    return out


def pandas_module():
    global _PANDAS
    global _PANDAS_UNAVAILABLE
    if _PANDAS is not None:
        return _PANDAS
    if _PANDAS_UNAVAILABLE:
        return None
    try:
        import pandas as pd  # noqa: PLC0415
    except Exception:
        _PANDAS_UNAVAILABLE = True
        return None
    _PANDAS = pd
    return _PANDAS


def rolling_rank_series(values):
    return [float(value) if finite(value) else math.nan for value in values]


def rolling_mid_rank_current(values, lookback, min_samples):
    lookback = max(1, int(lookback))
    min_samples = max(1, int(min_samples))
    if min_samples > lookback:
        return [float("nan")] * len(values)
    pd = pandas_module()
    if pd is not None:
        series = pd.Series(rolling_rank_series(values), dtype="float64")
        rolling = series.rolling(window=lookback, min_periods=min_samples)
        rank = rolling.rank(method="average")
        count = rolling.count()
        out = (rank - 1.0) / (count - 1.0)
        out = out.mask(count == 1.0, 0.5)
        return out.to_list()

    out = []
    queue = []
    sample = []
    for value in values:
        queue.append(value)
        if finite(value):
            bisect.insort(sample, float(value))
        if len(queue) > lookback:
            old = queue.pop(0)
            if finite(old):
                old_pos = bisect.bisect_left(sample, float(old))
                if old_pos < len(sample) and sample[old_pos] == float(old):
                    sample.pop(old_pos)
        if not finite(value) or len(sample) < min_samples:
            out.append(float("nan"))
            continue
        if len(sample) == 1:
            out.append(0.5)
            continue
        left = bisect.bisect_left(sample, float(value))
        right = bisect.bisect_right(sample, float(value))
        mid = (left + right - 1) / 2.0
        out.append(max(0.0, min(1.0, mid / (len(sample) - 1))))
    return out


def rolling_mid_rank_current_many(series_by_key, lookback, min_samples):
    keys = list(series_by_key)
    lookback = max(1, int(lookback))
    min_samples = max(1, int(min_samples))
    if min_samples > lookback:
        return {key: [float("nan")] * len(series_by_key[key]) for key in keys}

    pd = pandas_module()
    if pd is None:
        return {key: rolling_mid_rank_current(series_by_key[key], lookback, min_samples) for key in keys}

    frame = pd.DataFrame(
        {key: rolling_rank_series(series_by_key[key]) for key in keys},
        dtype="float64",
    )
    rolling = frame.rolling(window=lookback, min_periods=min_samples)
    rank = rolling.rank(method="average")
    count = rolling.count()
    out = (rank - 1.0) / (count - 1.0)
    out = out.mask(count == 1.0, 0.5)
    return {key: out[key].to_list() for key in keys}


def rolling_mid_rank_previous(values, lookback, min_samples):
    lookback = max(1, int(lookback))
    min_samples = max(1, int(min_samples))
    if min_samples > lookback:
        return [float("nan")] * len(values)
    out = []
    queue = []
    sample = []
    for value in values:
        if finite(value) and len(sample) >= min_samples:
            if len(sample) == 1:
                out.append(0.5)
            else:
                left = bisect.bisect_left(sample, float(value))
                right = bisect.bisect_right(sample, float(value))
                mid = (left + right - 1) / 2.0
                out.append(max(0.0, min(1.0, mid / (len(sample) - 1))))
        else:
            out.append(float("nan"))
        queue.append(value)
        if finite(value):
            bisect.insort(sample, float(value))
        if len(queue) > lookback:
            old = queue.pop(0)
            if finite(old):
                old_pos = bisect.bisect_left(sample, float(old))
                if old_pos < len(sample) and sample[old_pos] == float(old):
                    sample.pop(old_pos)
    return out


def rolling_right_rank_previous(values, lookback, min_samples):
    lookback = max(1, int(lookback))
    min_samples = max(1, int(min_samples))
    if min_samples > lookback:
        return [float("nan")] * len(values)
    pd = pandas_module()
    if pd is not None:
        series = pd.Series(rolling_rank_series(values), dtype="float64")
        rolling = series.rolling(window=lookback + 1, min_periods=min_samples + 1)
        rank = rolling.rank(method="max")
        count = rolling.count()
        return ((rank - 1.0) / (count - 1.0)).to_list()

    out = []
    queue = []
    sample = []
    for value in values:
        if finite(value) and len(sample) >= min_samples:
            out.append(bisect.bisect_right(sample, float(value)) / len(sample))
        else:
            out.append(float("nan"))
        queue.append(value)
        if finite(value):
            bisect.insort(sample, float(value))
        if len(queue) > lookback:
            old = queue.pop(0)
            if finite(old):
                old_pos = bisect.bisect_left(sample, float(old))
                if old_pos < len(sample):
                    sample.pop(old_pos)
    return out


def rolling_right_rank_previous_many(series_by_key, lookback, min_samples):
    keys = list(series_by_key)
    lookback = max(1, int(lookback))
    min_samples = max(1, int(min_samples))
    if min_samples > lookback:
        return {key: [float("nan")] * len(series_by_key[key]) for key in keys}

    pd = pandas_module()
    if pd is None:
        return {key: rolling_right_rank_previous(series_by_key[key], lookback, min_samples) for key in keys}

    frame = pd.DataFrame(
        {key: rolling_rank_series(series_by_key[key]) for key in keys},
        dtype="float64",
    )
    rolling = frame.rolling(window=lookback + 1, min_periods=min_samples + 1)
    rank = rolling.rank(method="max")
    count = rolling.count()
    out = (rank - 1.0) / (count - 1.0)
    return {key: out[key].to_list() for key in keys}


def rolling_right_rank_current(values, lookback, min_samples):
    lookback = max(1, int(lookback))
    min_samples = max(1, int(min_samples))
    if min_samples > lookback:
        return [float("nan")] * len(values)
    pd = pandas_module()
    if pd is not None:
        series = pd.Series(rolling_rank_series(values), dtype="float64")
        rolling = series.rolling(window=lookback, min_periods=min_samples)
        rank = rolling.rank(method="max")
        count = rolling.count()
        return (rank / count).to_list()

    out = []
    queue = []
    sample = []
    for value in values:
        queue.append(value)
        if finite(value):
            bisect.insort(sample, float(value))
        if len(queue) > lookback:
            old = queue.pop(0)
            if finite(old):
                old_pos = bisect.bisect_left(sample, float(old))
                if old_pos < len(sample) and sample[old_pos] == float(old):
                    sample.pop(old_pos)
        if finite(value) and len(sample) >= min_samples:
            out.append(bisect.bisect_right(sample, float(value)) / len(sample))
        else:
            out.append(float("nan"))
    return out


def rolling_right_rank_current_many(series_by_key, lookback, min_samples):
    keys = list(series_by_key)
    lookback = max(1, int(lookback))
    min_samples = max(1, int(min_samples))
    if min_samples > lookback:
        return {key: [float("nan")] * len(series_by_key[key]) for key in keys}

    pd = pandas_module()
    if pd is None:
        return {key: rolling_right_rank_current(series_by_key[key], lookback, min_samples) for key in keys}

    frame = pd.DataFrame(
        {key: rolling_rank_series(series_by_key[key]) for key in keys},
        dtype="float64",
    )
    rolling = frame.rolling(window=lookback, min_periods=min_samples)
    rank = rolling.rank(method="max")
    count = rolling.count()
    out = rank / count
    return {key: out[key].to_list() for key in keys}


def rolling_quantile_rank(values, lookback, min_samples, include_current=True):
    if include_current:
        return rolling_mid_rank_current(values, lookback, min_samples)
    return rolling_mid_rank_previous(values, lookback, min_samples)


def rolling_quantile_rank_right(values, lookback, min_samples):
    """Match the promoted spread_velocity_v1 rank contract exactly."""
    return rolling_right_rank_previous(values, lookback, min_samples)


def rolling_quantile_rank_current_right(values, lookback, min_samples):
    """Match the promoted dual_calendar_v2 rank contract exactly."""
    return rolling_right_rank_current(values, lookback, min_samples)


def ma_spread(closes, fast, slow):
    fast_ma = rolling_mean_full(closes, fast)
    slow_ma = rolling_mean_full(closes, slow)
    return [fast_ma[index] / slow_ma[index] - 1.0 if slow_ma[index] > 0.0 else float("nan") for index in range(len(closes))]


def spread_velocity(spread, lag):
    out = [float("nan")] * len(spread)
    for index in range(lag, len(spread)):
        if finite(spread[index]) and finite(spread[index - lag]):
            out[index] = spread[index] - spread[index - lag]
    return out


def momentum_series(closes, window):
    out = [0.0] * len(closes)
    for index in range(window, len(closes)):
        if closes[index - window] > 0.0:
            out[index] = closes[index] / closes[index - window] - 1.0
    return out


def lagged_return(closes, lag):
    out = [float("nan")] * len(closes)
    for index in range(lag, len(closes)):
        if closes[index - lag] > 0.0:
            out[index] = closes[index] / closes[index - lag] - 1.0
    return out


def rolling_range(highs, lows, window):
    out = [float("nan")] * len(highs)
    high_queue = deque()
    low_queue = deque()
    for index, (high, low) in enumerate(zip(highs, lows)):
        while high_queue and high_queue[0] <= index - window:
            high_queue.popleft()
        while low_queue and low_queue[0] <= index - window:
            low_queue.popleft()
        while high_queue and highs[high_queue[-1]] <= high:
            high_queue.pop()
        while low_queue and lows[low_queue[-1]] >= low:
            low_queue.pop()
        high_queue.append(index)
        low_queue.append(index)
        if index < window - 1:
            continue
        high = highs[high_queue[0]]
        low = lows[low_queue[0]]
        if low > 0.0:
            out[index] = high / low - 1.0
    return out


def rolling_std(values, window):
    out = [float("nan")] * len(values)
    min_count = max(3, int(window * 0.5))
    queue = deque()
    total = 0.0
    total_sq = 0.0
    count = 0
    for index, value in enumerate(values):
        queue.append(value)
        if finite(value):
            parsed = float(value)
            total += parsed
            total_sq += parsed * parsed
            count += 1
        if len(queue) > window:
            old = queue.popleft()
            if finite(old):
                parsed_old = float(old)
                total -= parsed_old
                total_sq -= parsed_old * parsed_old
                count -= 1
        if index < window - 1 or count < min_count:
            continue
        mean = total / count
        variance = total_sq / count - mean * mean
        out[index] = math.sqrt(max(0.0, variance))
    return out


def rolling_mean_full(values, window):
    out = []
    total = 0.0
    queue = deque()
    for value in values:
        queue.append(value)
        total += value
        if len(queue) > window:
            total -= queue.popleft()
        out.append(total / len(queue))
    return out


def prefix_sum_full(values):
    out = [0.0]
    total = 0.0
    for value in values:
        total += float(value)
        out.append(total)
    return out


def sma_at(values, index, window):
    if index < window - 1:
        return sum(values[: index + 1]) / max(1, index + 1)
    return sum(values[index - window + 1 : index + 1]) / window


def quantile_sorted(sample, quantile):
    if not sample:
        return 0.0
    quantile = max(0.0, min(1.0, float(quantile)))
    pos = (len(sample) - 1) * quantile
    left = int(pos)
    right = min(left + 1, len(sample) - 1)
    if left == right:
        return sample[left]
    return sample[left] * (1.0 - (pos - left)) + sample[right] * (pos - left)


def overextension_series(closes, fast, slow, momentum_window):
    fast_ma = rolling_mean_full(closes, fast)
    slow_ma = rolling_mean_full(closes, slow)
    spread = [
        0.0 if slow_ma[index] <= 0.0 else fast_ma[index] / slow_ma[index] - 1.0
        for index in range(len(closes))
    ]
    momentum = [
        0.0
        if index < momentum_window or closes[index - momentum_window] <= 0.0
        else closes[index] / closes[index - momentum_window] - 1.0
        for index in range(len(closes))
    ]
    return spread, momentum
