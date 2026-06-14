"""BTC, market breadth, and funding context features for ML trade candidates."""

from __future__ import annotations

import bisect
import math

from lib.ml_trade_feature_math import (
    btc48_bin,
    btc_regime,
    breadth_bin,
    dispersion_bin,
    finite_float,
    finite_int,
    funding_age_bin,
    funding_interval_bin,
    funding_rate_bin,
    position_bin,
    quote_volume_from_values,
    range_bin,
    rank_bin,
    relative_return_bin,
    return_bin,
    round_float,
    short_return_bin,
    side_adjusted,
    side_funding_carry_bin,
    snapshot_size_bin,
    vol_bin,
    z_score_bin,
)
from lib.ml_trade_feature_series import (
    append_finite,
    average_from_values,
    bounded_candle_source,
    btc_series_from_candles,
    context_cutoff_bounds,
    finite_values,
    high_low_range_from_arrays,
    lagged_array_return,
    median_value,
    positive_share,
    precompute_average_from_values,
    precompute_lagged_returns,
    precompute_realized_vol_from_closes,
    precomputed_value,
    range_position_from_arrays,
    rank_percentile_from_clean,
    realized_daily_vol_from_closes,
    relative_return,
    rounded_or_none,
    stdev_value,
)

DAY_MS = 24 * 60 * 60 * 1000

def add_btc_context(row, btc_candles, cache=None):
    """Add completed-BTC-bar context for the candidate entry time."""

    source = btc_candles if btc_series_ready(btc_candles) else btc_series_from_candles(btc_candles)
    if not source["timestamps"]:
        return dict(row)

    entry_time = finite_int(row.get("entry_time") if isinstance(row, dict) else None)
    if entry_time is None:
        return dict(row)

    side = str(row.get("side", ""))
    cutoff = int(entry_time) - 15 * 60_000
    pos = bisect.bisect_right(source["timestamps"], cutoff) - 1
    cache_key = None
    if isinstance(cache, dict):
        cache_key = (id(source), cutoff, pos)
        cached = cache.get(cache_key)
        if cached is not None:
            out = dict(row)
            out.update(cached)
            add_relative_btc_context(out, str(out.get("side", "")))
            return out

    out = dict(row)
    highs = source["highs"]
    lows = source["lows"]
    closes = source["closes"]
    ret4 = lagged_array_return(closes, pos, 16)
    ret24 = lagged_array_return(closes, pos, 96)
    ret48 = lagged_array_return(closes, pos, 192)
    ret7d = lagged_array_return(closes, pos, 672)
    vol24 = realized_daily_vol_from_closes(closes, pos, 96, 15)
    range24 = high_low_range_from_arrays(highs, lows, pos, 96)
    position7d = range_position_from_arrays(highs, lows, closes, pos, 672)

    out["btc4_return"] = round_float(ret4)
    out["btc24_return"] = round_float(ret24)
    out["btc48_return"] = round_float(ret48)
    out["btc7d_return"] = round_float(ret7d)
    out["btc_vol_24h"] = round_float(vol24)
    out["btc_range_24h"] = round_float(range24)
    out["btc_position_7d"] = round_float(position7d)
    out["btc4_bin"] = short_return_bin(ret4)
    out["btc24_bin"] = return_bin(ret24)
    out["btc48_bin"] = btc48_bin(ret48)
    out["btc_regime"] = btc_regime(ret48, ret7d)
    out["btc_vol_24h_bin"] = vol_bin(vol24)
    out["btc_range_24h_bin"] = range_bin(range24)
    out["btc_position_7d_bin"] = position_bin(position7d)

    if cache_key is not None:
        cache[cache_key] = {
            "btc4_return": out.get("btc4_return"),
            "btc24_return": out.get("btc24_return"),
            "btc48_return": out.get("btc48_return"),
            "btc7d_return": out.get("btc7d_return"),
            "btc_vol_24h": out.get("btc_vol_24h"),
            "btc_range_24h": out.get("btc_range_24h"),
            "btc_position_7d": out.get("btc_position_7d"),
            "btc4_bin": out.get("btc4_bin"),
            "btc24_bin": out.get("btc24_bin"),
            "btc48_bin": out.get("btc48_bin"),
            "btc_regime": out.get("btc_regime"),
            "btc_vol_24h_bin": out.get("btc_vol_24h_bin"),
            "btc_range_24h_bin": out.get("btc_range_24h_bin"),
            "btc_position_7d_bin": out.get("btc_position_7d_bin"),
        }
    add_relative_btc_context(out, side)
    return out

def add_relative_btc_context(out, side):
    rel4 = relative_return(out.get("local_ret_4h"), out.get("btc4_return"))
    rel24 = relative_return(out.get("local_ret_24h"), out.get("btc24_return"))
    out["asset_vs_btc_4h"] = round_float(rel4)
    out["asset_vs_btc_24h"] = round_float(rel24)
    out["asset_vs_btc_4h_bin"] = relative_return_bin(rel4)
    out["asset_vs_btc_24h_bin"] = relative_return_bin(rel24)
    out["side_asset_vs_btc_24h_bin"] = relative_return_bin(side_adjusted(rel24, side))

def btc_series_ready(value):
    if not isinstance(value, dict):
        return False
    return all(isinstance(value.get(key), list) for key in ("timestamps", "highs", "lows", "closes"))

def build_market_context_from_runtime(context, symbols, min_symbol_count=20, timestamps=None):
    """Build a point-in-time market context from runtime candle payloads."""

    candles = context.get("candles") if isinstance(context, dict) else {}
    if not isinstance(candles, dict):
        return None

    series = {}
    requested = sorted({str(symbol) for symbol in symbols if str(symbol).strip()})
    cutoff_start, cutoff_end = context_cutoff_bounds(timestamps)
    for symbol in requested:
        timeframe_map = candles.get(symbol)
        if not isinstance(timeframe_map, dict):
            continue
        source = market_series_from_candles(
            timeframe_map.get("15m"),
            cutoff_start=cutoff_start,
            cutoff_end=cutoff_end,
        )
        if source["timestamps"]:
            series[symbol] = source

    if len(series) < max(1, int(min_symbol_count or 1)):
        return None
    return {
        "timeframe": "15m",
        "requested_symbols": requested,
        "symbols": sorted(series),
        "series": series,
        "snapshot_cache": {},
    }

def market_series_from_candles(candles, cutoff_start=None, cutoff_end=None):
    rows = []
    source = bounded_candle_source(candles, cutoff_start, cutoff_end, history_bars=100)
    if isinstance(source, list):
        for candle in source:
            if not isinstance(candle, dict):
                continue
            timestamp = finite_int(candle.get("timestamp"))
            close = finite_float(candle.get("close"))
            quoted = quote_volume_from_values(
                finite_float(candle.get("volume_ccy")),
                finite_float(candle.get("volume")),
                close,
            )
            if timestamp is None or close is None or close <= 0.0:
                continue
            rows.append((timestamp, close, quoted if quoted is not None else 0.0))
    rows.sort(key=lambda item: item[0])
    return {
        "rows": len(rows),
        "timestamps": [item[0] for item in rows],
        "closes": [item[1] for item in rows],
        "quote_volumes": [item[2] for item in rows],
        "features": market_series_features([item[1] for item in rows], [item[2] for item in rows]),
    }

def market_series_features(closes, quote_volumes):
    return {
        "ret4": precompute_lagged_returns(closes, 16),
        "ret24": precompute_lagged_returns(closes, 96),
        "vol24": precompute_realized_vol_from_closes(closes, 96, 15),
        "quote_volume_24h": precompute_average_from_values(quote_volumes, 96),
    }

def add_cross_sectional_context(row, market_context, cache=None):
    if not market_context:
        return dict(row)

    entry_time = finite_int(row.get("entry_time") if isinstance(row, dict) else None)
    if entry_time is None:
        out = dict(row)
        out.update(default_cross_sectional_context())
        return out
    asset = str(row.get("asset", ""))
    side = str(row.get("side", ""))
    cache_key = None
    if isinstance(cache, dict):
        cache_key = (id(market_context), entry_time, asset)
        cached = cache.get(cache_key)
        if cached is not None:
            out = dict(row)
            out.update(cached)
            raw_asset_vs_market_24h = cached.get("_raw_asset_vs_market_24h")
            out.pop("_raw_asset_vs_market_24h", None)
            add_side_market_fields(out, raw_asset_vs_market_24h, side)
            return out

    out = dict(row)
    row_keys = set(out)
    out.update(default_cross_sectional_context())
    snapshot = market_snapshot(market_context, entry_time)
    asset_row = snapshot.get("assets", {}).get(asset)
    market_count = int(snapshot.get("count", 0) or 0)
    out["market_context_count"] = market_count
    out["market_breadth_4h"] = rounded_or_none(snapshot.get("breadth_4h"))
    out["market_breadth_24h"] = rounded_or_none(snapshot.get("breadth_24h"))
    out["market_dispersion_4h"] = rounded_or_none(snapshot.get("dispersion_4h"))
    out["market_dispersion_24h"] = rounded_or_none(snapshot.get("dispersion_24h"))
    out["market_median_ret_4h"] = rounded_or_none(snapshot.get("median_ret_4h"))
    out["market_median_ret_24h"] = rounded_or_none(snapshot.get("median_ret_24h"))
    out["market_snapshot_size_bin"] = snapshot_size_bin(market_count, int(snapshot.get("total_symbols", 0) or 0))
    out["market_breadth_4h_bin"] = breadth_bin(snapshot.get("breadth_4h"))
    out["market_breadth_24h_bin"] = breadth_bin(snapshot.get("breadth_24h"))
    out["market_dispersion_24h_bin"] = dispersion_bin(snapshot.get("dispersion_24h"))
    if not asset_row:
        return out

    asset_vs_market_24h = relative_return(asset_row.get("ret24"), snapshot.get("median_ret_24h"))
    out["asset_xs_ret_4h_rank"] = rounded_or_none(asset_row.get("ret4_rank"))
    out["asset_xs_ret_24h_rank"] = rounded_or_none(asset_row.get("ret24_rank"))
    out["asset_xs_vol_24h_rank"] = rounded_or_none(asset_row.get("vol24_rank"))
    out["asset_xs_quote_volume_rank"] = rounded_or_none(asset_row.get("quote_volume_rank"))
    out["asset_vs_market_24h"] = rounded_or_none(asset_vs_market_24h)
    out["asset_xs_ret_4h_rank_bin"] = rank_bin(asset_row.get("ret4_rank"))
    out["asset_xs_ret_24h_rank_bin"] = rank_bin(asset_row.get("ret24_rank"))
    out["asset_xs_vol_24h_rank_bin"] = rank_bin(asset_row.get("vol24_rank"))
    out["asset_xs_quote_volume_rank_bin"] = rank_bin(asset_row.get("quote_volume_rank"))
    out["asset_vs_market_24h_bin"] = relative_return_bin(asset_vs_market_24h)
    add_side_market_fields(out, asset_vs_market_24h, str(out.get("side", "")))
    if cache_key is not None:
        cache[cache_key] = {
            key: value
            for key, value in out.items()
            if key not in row_keys and key not in SIDE_MARKET_KEYS
        }
        cache[cache_key]["_raw_asset_vs_market_24h"] = asset_vs_market_24h
    return out

SIDE_MARKET_KEYS = {
    "side_asset_vs_market_24h",
    "side_asset_vs_market_24h_bin",
}

def add_side_market_fields(out, asset_vs_market_24h, side):
    side_asset_vs_market_24h = side_adjusted(asset_vs_market_24h, side)
    out["side_asset_vs_market_24h"] = rounded_or_none(side_asset_vs_market_24h)
    out["side_asset_vs_market_24h_bin"] = relative_return_bin(side_asset_vs_market_24h)

def build_funding_context_from_runtime(context, symbols):
    funding = context.get("funding") if isinstance(context, dict) else {}
    if not isinstance(funding, dict):
        return None

    series = {}
    requested = sorted({str(symbol) for symbol in symbols if str(symbol).strip()})
    for symbol in requested:
        payload = funding.get(symbol)
        if not isinstance(payload, dict):
            continue
        rows = funding_rows_from_payload(payload)
        if rows:
            series[symbol] = build_funding_series(rows)
    return {
        "enabled": bool(series),
        "requested_symbols": requested,
        "symbols": sorted(series),
        "series": series,
    }

def funding_rows_from_payload(payload):
    by_timestamp = {}
    history = payload.get("history")
    if isinstance(history, list):
        for row in history:
            parsed = funding_row_from_dict(row)
            if parsed is not None:
                by_timestamp[parsed[0]] = parsed
    latest = payload.get("latest")
    if isinstance(latest, dict):
        parsed = funding_row_from_dict(latest)
        if parsed is not None:
            by_timestamp[parsed[0]] = parsed
    return sorted(by_timestamp.values(), key=lambda item: item[0])

def funding_row_from_dict(row):
    if not isinstance(row, dict):
        return None
    timestamp = finite_int(row.get("funding_time", row.get("timestamp")))
    rate = finite_float(row.get("funding_rate", row.get("rate")))
    if timestamp is None or rate is None:
        return None
    realized = finite_float(row.get("realized_rate", row.get("realizedRate")))
    method = str(row.get("method") or "")
    formula_type = str(row.get("formula_type", row.get("formulaType")) or "")
    return timestamp, rate, realized, method, formula_type

def build_funding_series(rows):
    ordered = sorted(rows, key=lambda item: item[0])
    timestamps = [int(item[0]) for item in ordered]
    rates = [float(item[1]) for item in ordered]
    realized_rates = [item[2] for item in ordered]
    methods = [str(item[3] or "") for item in ordered]
    formula_types = [str(item[4] or "") for item in ordered]
    prefix_rates = [0.0]
    prefix_rate_squares = [0.0]
    for rate in rates:
        prefix_rates.append(prefix_rates[-1] + rate)
        prefix_rate_squares.append(prefix_rate_squares[-1] + rate * rate)
    gaps = [timestamps[index] - timestamps[index - 1] for index in range(1, len(timestamps))]
    return {
        "rows": len(ordered),
        "start": timestamps[0] if timestamps else None,
        "end": timestamps[-1] if timestamps else None,
        "timestamps": timestamps,
        "rates": rates,
        "realized_rates": realized_rates,
        "methods": methods,
        "formula_types": formula_types,
        "prefix_rates": prefix_rates,
        "prefix_rate_squares": prefix_rate_squares,
        "max_gap_ms": max(gaps, default=0),
        "median_gap_ms": int(median_value([float(gap) for gap in gaps]) or 0),
    }

def add_funding_context(row, funding_context, cache=None):
    if not funding_context or not funding_context.get("enabled"):
        out = dict(row)
        out.update(default_funding_context())
        out["funding_data_status"] = "missing_context"
        return out

    symbol = str(row.get("asset", "") if isinstance(row, dict) else "")
    source = funding_context.get("series", {}).get(symbol)
    if not source:
        out = dict(row)
        out.update(default_funding_context())
        out["funding_data_status"] = "missing_symbol"
        return out

    feature_time = finite_int(row.get("feature_bar_time") if isinstance(row, dict) else None)
    if feature_time is None:
        entry_time = finite_int(row.get("entry_time") if isinstance(row, dict) else None)
        feature_time = None if entry_time is None else entry_time - 15 * 60_000
    if feature_time is None:
        out = dict(row)
        out.update(default_funding_context())
        out["funding_data_status"] = "missing_feature_time"
        return out
    side = str(row.get("side", "") if isinstance(row, dict) else "")
    cache_key = None
    if isinstance(cache, dict):
        cache_key = (id(funding_context), symbol, feature_time)
        cached = cache.get(cache_key)
        if cached is not None:
            out = dict(row)
            out.update(cached)
            raw_funding_rate = cached.get("_raw_funding_rate")
            out.pop("_raw_funding_rate", None)
            add_side_funding_fields(out, raw_funding_rate, side)
            return out

    out = dict(row)
    row_keys = set(out)
    out.update(default_funding_context())
    timestamps = source["timestamps"]
    pos = bisect.bisect_right(timestamps, feature_time) - 1
    if pos < 0:
        out["funding_data_status"] = "no_row_before_feature_time"
        return out

    funding_time = int(timestamps[pos])
    rate = float(source["rates"][pos])
    realized = source["realized_rates"][pos]
    previous_gap = funding_time - int(timestamps[pos - 1]) if pos > 0 else None
    age_hours = max(0.0, (feature_time - funding_time) / 3_600_000.0)
    sum_24h, count_24h = funding_rate_sum_between(source, feature_time - DAY_MS, feature_time)
    avg_24h = sum_24h / count_24h if count_24h > 0 else None
    z_30d = funding_rate_z(source, pos, 30 * DAY_MS)

    out["funding_data_status"] = "available" if age_hours <= 24.0 else "stale"
    out["funding_rate"] = round_float(rate)
    out["funding_realized_rate"] = round_float(realized)
    out["funding_rate_bps"] = round_float(rate * 10_000.0)
    out["funding_realized_rate_bps"] = round_float(float(realized) * 10_000.0) if realized is not None else None
    out["funding_rate_sum_24h"] = round_float(sum_24h) if count_24h > 0 else None
    out["funding_rate_avg_24h"] = round_float(avg_24h)
    out["funding_rate_z_30d"] = round_float(z_30d)
    out["funding_event_count_24h"] = count_24h
    out["funding_last_age_hours"] = round_float(age_hours)
    out["funding_interval_hours"] = round_float(previous_gap / 3_600_000.0) if previous_gap is not None else None
    out["funding_method"] = str(source["methods"][pos] or "unknown")
    out["funding_formula_type"] = str(source["formula_types"][pos] or "unknown")
    out["funding_rate_bin"] = funding_rate_bin(rate)
    out["funding_realized_rate_bin"] = funding_rate_bin(realized)
    out["funding_rate_sum_24h_bin"] = funding_rate_bin(sum_24h if count_24h > 0 else None)
    out["funding_rate_z_30d_bin"] = z_score_bin(z_30d)
    out["funding_last_age_bin"] = funding_age_bin(age_hours)
    out["funding_interval_bin"] = funding_interval_bin(previous_gap)
    add_side_funding_fields(out, rate, str(out.get("side", "")))
    if cache_key is not None:
        cache[cache_key] = {
            key: value
            for key, value in out.items()
            if key not in row_keys and key not in SIDE_FUNDING_KEYS
        }
        cache[cache_key]["_raw_funding_rate"] = rate
    return out

SIDE_FUNDING_KEYS = {
    "side_funding_carry",
    "side_funding_carry_bps",
    "side_funding_carry_bin",
}

def add_side_funding_fields(out, rate, side):
    side_carry = side_funding_carry(rate, side)
    out["side_funding_carry"] = round_float(side_carry)
    out["side_funding_carry_bps"] = round_float(side_carry * 10_000.0) if side_carry is not None else None
    out["side_funding_carry_bin"] = side_funding_carry_bin(side_carry)

def default_funding_context():
    return {
        "funding_rate": None,
        "funding_realized_rate": None,
        "funding_rate_bps": None,
        "funding_realized_rate_bps": None,
        "funding_rate_sum_24h": None,
        "funding_rate_avg_24h": None,
        "funding_rate_z_30d": None,
        "funding_event_count_24h": 0,
        "funding_last_age_hours": None,
        "funding_interval_hours": None,
        "side_funding_carry": None,
        "side_funding_carry_bps": None,
        "funding_data_status": "unknown",
        "funding_rate_bin": "unknown",
        "funding_realized_rate_bin": "unknown",
        "funding_rate_sum_24h_bin": "unknown",
        "funding_rate_z_30d_bin": "unknown",
        "funding_last_age_bin": "unknown",
        "funding_interval_bin": "unknown",
        "side_funding_carry_bin": "unknown",
        "funding_method": "unknown",
        "funding_formula_type": "unknown",
    }

def funding_rate_sum_between(source, start, end):
    timestamps = source["timestamps"]
    prefix = source["prefix_rates"]
    left = bisect.bisect_right(timestamps, int(start))
    right = bisect.bisect_right(timestamps, int(end))
    if right <= left:
        return 0.0, 0
    return float(prefix[right] - prefix[left]), int(right - left)

def funding_rate_z(source, pos, window_ms):
    if pos <= 0:
        return None
    timestamps = source["timestamps"]
    rates = source["rates"]
    current_time = int(timestamps[pos])
    left = bisect.bisect_left(timestamps, current_time - int(window_ms))
    count = pos - left
    if count < 12:
        return None
    prefix = source.get("prefix_rates")
    prefix_squares = source.get("prefix_rate_squares")
    if not isinstance(prefix, list) or not isinstance(prefix_squares, list):
        history = [float(value) for value in rates[left:pos] if finite_float(value) is not None]
        if len(history) < 12:
            return None
        mean = sum(history) / len(history)
        variance = sum((value - mean) * (value - mean) for value in history) / len(history)
    else:
        total = float(prefix[pos] - prefix[left])
        total_sq = float(prefix_squares[pos] - prefix_squares[left])
        mean = total / count
        variance = total_sq / count - mean * mean
    stdev = math.sqrt(max(0.0, variance))
    if stdev <= 1e-12:
        return None
    return (float(rates[pos]) - mean) / stdev

def side_funding_carry(rate, side):
    if rate is None:
        return None
    if side == "long":
        return -float(rate)
    if side == "short":
        return float(rate)
    return None

def default_cross_sectional_context():
    return {
        "market_context_count": 0,
        "market_breadth_4h": None,
        "market_breadth_24h": None,
        "market_dispersion_4h": None,
        "market_dispersion_24h": None,
        "market_median_ret_4h": None,
        "market_median_ret_24h": None,
        "asset_xs_ret_4h_rank": None,
        "asset_xs_ret_24h_rank": None,
        "asset_xs_vol_24h_rank": None,
        "asset_xs_quote_volume_rank": None,
        "asset_vs_market_24h": None,
        "side_asset_vs_market_24h": None,
        "market_snapshot_size_bin": "unknown",
        "market_breadth_4h_bin": "unknown",
        "market_breadth_24h_bin": "unknown",
        "market_dispersion_24h_bin": "unknown",
        "asset_xs_ret_4h_rank_bin": "unknown",
        "asset_xs_ret_24h_rank_bin": "unknown",
        "asset_xs_vol_24h_rank_bin": "unknown",
        "asset_xs_quote_volume_rank_bin": "unknown",
        "asset_vs_market_24h_bin": "unknown",
        "side_asset_vs_market_24h_bin": "unknown",
    }

def market_snapshot(market_context, entry_time):
    cutoff = int(entry_time) - 15 * 60_000
    cache = market_context.setdefault("snapshot_cache", {})
    if cutoff in cache:
        return cache[cutoff]

    asset_rows = {}
    ret4_values = []
    ret24_values = []
    vol24_values = []
    quote_volume_values = []
    for symbol, source in market_context.get("series", {}).items():
        timestamps = source["timestamps"]
        closes = source["closes"]
        quote_volumes = source["quote_volumes"]
        features = source.get("features") if isinstance(source.get("features"), dict) else {}
        pos = bisect.bisect_right(timestamps, cutoff) - 1
        if pos < 0:
            continue
        ret4 = precomputed_value(features.get("ret4"), pos, lambda: lagged_array_return(closes, pos, 16))
        ret24 = precomputed_value(features.get("ret24"), pos, lambda: lagged_array_return(closes, pos, 96))
        vol24 = precomputed_value(features.get("vol24"), pos, lambda: realized_daily_vol_from_closes(closes, pos, 96, 15))
        quote_volume_24h = precomputed_value(
            features.get("quote_volume_24h"),
            pos,
            lambda: average_from_values(quote_volumes, pos, 96),
        )
        item = {
            "ret4": ret4,
            "ret24": ret24,
            "vol24": vol24,
            "quote_volume_24h": quote_volume_24h,
        }
        asset_rows[str(symbol)] = item
        append_finite(ret4_values, ret4)
        append_finite(ret24_values, ret24)
        append_finite(vol24_values, vol24)
        append_finite(quote_volume_values, quote_volume_24h)

    snapshot = {
        "cutoff": cutoff,
        "total_symbols": len(market_context.get("requested_symbols") or market_context.get("series", {})),
        "count": len(asset_rows),
        "breadth_4h": positive_share(ret4_values),
        "breadth_24h": positive_share(ret24_values),
        "dispersion_4h": stdev_value(ret4_values),
        "dispersion_24h": stdev_value(ret24_values),
        "median_ret_4h": median_value(ret4_values),
        "median_ret_24h": median_value(ret24_values),
        "assets": asset_rows,
    }
    ret4_rank_values = sorted(finite_values(ret4_values))
    ret24_rank_values = sorted(finite_values(ret24_values))
    vol24_rank_values = sorted(finite_values(vol24_values))
    quote_volume_rank_values = sorted(finite_values(quote_volume_values))
    for item in asset_rows.values():
        item["ret4_rank"] = rank_percentile_from_clean(item.get("ret4"), ret4_rank_values)
        item["ret24_rank"] = rank_percentile_from_clean(item.get("ret24"), ret24_rank_values)
        item["vol24_rank"] = rank_percentile_from_clean(item.get("vol24"), vol24_rank_values)
        item["quote_volume_rank"] = rank_percentile_from_clean(item.get("quote_volume_24h"), quote_volume_rank_values)
    cache[cutoff] = snapshot
    return snapshot
