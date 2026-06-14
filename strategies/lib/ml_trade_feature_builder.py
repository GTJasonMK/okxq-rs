"""Runtime-owned point-in-time feature construction for ML trade candidates."""

from __future__ import annotations

from datetime import UTC, datetime, timezone, timedelta

from lib.ml_trade_context_features import (
    add_btc_context,
    add_cross_sectional_context,
    add_funding_context,
    btc_series_from_candles,
    build_funding_context_from_runtime,
    build_market_context_from_runtime,
    default_cross_sectional_context,
    default_funding_context,
    market_snapshot,
)
from lib.ml_trade_feature_math import (
    atr_ratio_bin,
    average_quote_volume,
    average_true_range,
    bjt_session,
    bjt_weekpart,
    body_bin,
    candle_close_location,
    candle_direction,
    candle_range,
    finite_float,
    finite_int,
    funding_window,
    hold_bucket,
    liquidity_stress_bin,
    log_trend_slope_and_quality,
    lower_wick_ratio,
    max_wick_ratio,
    path_excursion_bin,
    position_bin,
    quote_volume,
    quote_volume_bin,
    range_bin,
    range_compression_bin,
    ratio_regime_bin,
    return_bin,
    round_float,
    safe_ratio,
    session_transition,
    side_adjusted,
    side_adverse_wick,
    side_favorable_wick,
    side_path_excursion,
    side_wick_count,
    spike_reversal_type,
    strength_bin,
    trend_quality_bin,
    true_range_ratio,
    upper_wick_ratio,
    vol_bin,
    volume_ratio_bin,
    wick_bin,
    wick_cluster_bin,
    wick_pressure_bin,
    wick_skew,
    wick_skew_bin,
    hour_bucket,
)
from lib.ml_trade_feature_series import (
    bars_for_hours,
    candle_return,
    high_low_range,
    local_series_context,
    prefix_window,
    range_position,
    realized_daily_vol,
    series_average_true_range,
    series_high_low_range,
    series_lagged_return,
    series_max_wick_ratio,
    series_range_position,
    series_realized_vol,
    series_side_path_excursion,
    series_side_wick_count,
    series_true_range,
    series_volume_ratio,
    timeframe_minutes,
    volume_ratio,
)

BJ_TZ = timezone(timedelta(hours=8))

def add_local_and_calendar_features(row, candles, cache=None):
    """Add local OHLCV/path/calendar features visible at the signal bar."""

    signal_index = finite_int(row.get("signal_index") if isinstance(row, dict) else None)
    if signal_index is None or signal_index < 0 or signal_index >= len(candles):
        return dict(row)

    side = str(row.get("side", ""))
    signal_bar = candles[signal_index]
    feature_time = int(signal_bar.get("timestamp", row.get("feature_bar_time") or row.get("entry_time") or 0) or 0)
    entry_time = int(row.get("entry_time") or feature_time)
    timeframe = str(row.get("timeframe") or "15m")
    strength_value = finite_float(row.get("strength")) or 0.0
    hold_value = finite_int(row.get("planned_hold_bars")) or finite_int(row.get("hold_bars")) or 0
    cache_key = None
    if isinstance(cache, dict):
        cache_key = (id(candles), signal_index, timeframe, entry_time, feature_time)
        cached = cache.get(cache_key)
        if cached is not None:
            out = dict(row)
            out.update(cached)
            if str(cached.get("_local_cached_side") or "") == side:
                strip_local_cache_private_fields(out)
            else:
                apply_local_side_features_from_cache(out, side, timeframe)
            out["signal_strength_bin"] = strength_bin(strength_value)
            out["hold_bucket"] = hold_bucket(hold_value)
            return out

    out = dict(row)
    row_keys = set(out)
    out["feature_bar_time"] = feature_time
    out.update(calendar_features_for_entry(entry_time, cache))

    out["signal_bar_dir"] = candle_direction(signal_bar)
    out["signal_body_bin"] = body_bin(signal_bar)
    out["signal_range_bin"] = range_bin(candle_range(signal_bar))

    upper = upper_wick_ratio(signal_bar)
    lower = lower_wick_ratio(signal_bar)
    skew = wick_skew(upper, lower)
    adverse_signal = side_adverse_wick(upper, lower, side)
    favorable_signal = side_favorable_wick(upper, lower, side)
    out["signal_upper_wick"] = round_float(upper)
    out["signal_lower_wick"] = round_float(lower)
    out["signal_wick_skew"] = round_float(skew)
    out["side_signal_adverse_wick"] = round_float(adverse_signal)
    out["side_signal_favorable_wick"] = round_float(favorable_signal)
    out["signal_upper_wick_bin"] = wick_bin(upper)
    out["signal_lower_wick_bin"] = wick_bin(lower)
    out["signal_wick_skew_bin"] = wick_skew_bin(skew)
    out["side_signal_adverse_wick_bin"] = wick_bin(adverse_signal)
    out["side_signal_favorable_wick_bin"] = wick_bin(favorable_signal)

    minutes = timeframe_minutes(timeframe)
    lag_1h = bars_for_hours(minutes, 1)
    lag_4h = bars_for_hours(minutes, 4)
    lag_24h = bars_for_hours(minutes, 24)
    lag_72h = bars_for_hours(minutes, 72)

    series = local_series_context(candles, timeframe, cache, signal_index) if isinstance(cache, dict) else None
    series_index = signal_index - int(series.get("index_offset", 0) or 0) if series is not None else signal_index
    if series is not None:
        ret_1h = series_lagged_return(series, series_index, lag_1h)
        ret_4h = series_lagged_return(series, series_index, lag_4h)
        ret_24h = series_lagged_return(series, series_index, lag_24h)
        ret_72h = series_lagged_return(series, series_index, lag_72h)
    else:
        ret_1h = candle_return(candles, signal_index, lag_1h)
        ret_4h = candle_return(candles, signal_index, lag_4h)
        ret_24h = candle_return(candles, signal_index, lag_24h)
        ret_72h = candle_return(candles, signal_index, lag_72h)
    trend_4h, trend_quality_4h = log_trend_slope_and_quality(candles, signal_index, lag_4h)
    trend_24h, trend_quality_24h = log_trend_slope_and_quality(candles, signal_index, lag_24h)
    out["local_ret_1h"] = round_float(ret_1h)
    out["local_ret_4h"] = round_float(ret_4h)
    out["local_ret_24h"] = round_float(ret_24h)
    out["local_ret_72h"] = round_float(ret_72h)
    out["local_trend_slope_4h"] = round_float(trend_4h)
    out["local_trend_slope_24h"] = round_float(trend_24h)
    out["side_trend_slope_4h"] = round_float(side_adjusted(trend_4h, side))
    out["side_trend_slope_24h"] = round_float(side_adjusted(trend_24h, side))
    out["local_trend_quality_4h"] = round_float(trend_quality_4h)
    out["local_trend_quality_24h"] = round_float(trend_quality_24h)
    out["local_ret_1h_bin"] = return_bin(ret_1h)
    out["local_ret_4h_bin"] = return_bin(ret_4h)
    out["local_ret_24h_bin"] = return_bin(ret_24h)
    out["local_ret_72h_bin"] = return_bin(ret_72h)
    out["side_local_ret_4h_bin"] = return_bin(side_adjusted(ret_4h, side))
    out["side_local_ret_24h_bin"] = return_bin(side_adjusted(ret_24h, side))
    out["local_trend_slope_4h_bin"] = return_bin(trend_4h)
    out["local_trend_slope_24h_bin"] = return_bin(trend_24h)
    out["side_trend_slope_4h_bin"] = return_bin(side_adjusted(trend_4h, side))
    out["side_trend_slope_24h_bin"] = return_bin(side_adjusted(trend_24h, side))
    out["local_trend_quality_4h_bin"] = trend_quality_bin(trend_quality_4h)
    out["local_trend_quality_24h_bin"] = trend_quality_bin(trend_quality_24h)

    if series is not None:
        vol_4h = series_realized_vol(series, series_index, lag_4h)
        vol_24h = series_realized_vol(series, series_index, lag_24h)
    else:
        vol_4h = realized_daily_vol(candles, signal_index, lag_4h, minutes)
        vol_24h = realized_daily_vol(candles, signal_index, lag_24h, minutes)
    vol_ratio = safe_ratio(vol_4h, vol_24h)
    if series is not None:
        range_4h = series_high_low_range(series, series_index, lag_4h)
        range_24h = series_high_low_range(series, series_index, lag_24h)
    else:
        range_4h = high_low_range(candles, signal_index, lag_4h)
        range_24h = high_low_range(candles, signal_index, lag_24h)
    range_compression = safe_ratio(range_4h, range_24h)
    position_24h = series_range_position(series, series_index, lag_24h) if series is not None else range_position(candles, signal_index, lag_24h)
    volume_24h = series_volume_ratio(series, series_index, lag_24h) if series is not None else volume_ratio(candles, signal_index, lag_24h)
    quote_signal = quote_volume(signal_bar)
    quote_avg_24h = average_quote_volume(candles, signal_index, lag_24h)
    quote_ratio_24h = safe_ratio(quote_signal, quote_avg_24h)
    close_location = candle_close_location(signal_bar)
    true_range_value = series_true_range(series, series_index) if series is not None else true_range_ratio(candles, signal_index)
    atr_4h = series_average_true_range(series, series_index, lag_4h) if series is not None else average_true_range(candles, signal_index, lag_4h)
    range_over_atr = safe_ratio(true_range_value, atr_4h)

    if series is not None:
        adverse_path_1h = series_side_path_excursion(series, series_index, lag_1h, side, "adverse")
        adverse_path_4h = series_side_path_excursion(series, series_index, lag_4h, side, "adverse")
        favorable_path_1h = series_side_path_excursion(series, series_index, lag_1h, side, "favorable")
        favorable_path_4h = series_side_path_excursion(series, series_index, lag_4h, side, "favorable")
        upper_1h = series_max_wick_ratio(series, series_index, lag_1h, "upper")
        lower_1h = series_max_wick_ratio(series, series_index, lag_1h, "lower")
        upper_4h = series_max_wick_ratio(series, series_index, lag_4h, "upper")
        lower_4h = series_max_wick_ratio(series, series_index, lag_4h, "lower")
        adverse_count_1h = series_side_wick_count(series, series_index, lag_1h, side, "adverse")
        adverse_count_4h = series_side_wick_count(series, series_index, lag_4h, side, "adverse")
    else:
        adverse_path_1h = side_path_excursion(candles, signal_index, lag_1h, side, "adverse")
        adverse_path_4h = side_path_excursion(candles, signal_index, lag_4h, side, "adverse")
        favorable_path_1h = side_path_excursion(candles, signal_index, lag_1h, side, "favorable")
        favorable_path_4h = side_path_excursion(candles, signal_index, lag_4h, side, "favorable")
        upper_1h = max_wick_ratio(candles, signal_index, lag_1h, "upper")
        lower_1h = max_wick_ratio(candles, signal_index, lag_1h, "lower")
        upper_4h = max_wick_ratio(candles, signal_index, lag_4h, "upper")
        lower_4h = max_wick_ratio(candles, signal_index, lag_4h, "lower")
        adverse_count_1h = side_wick_count(candles, signal_index, lag_1h, side, "adverse")
        adverse_count_4h = side_wick_count(candles, signal_index, lag_4h, side, "adverse")
    adverse_wick_4h = side_adverse_wick(upper_4h, lower_4h, side)
    favorable_wick_4h = side_favorable_wick(upper_4h, lower_4h, side)
    pressure_4h = wick_skew(favorable_wick_4h, adverse_wick_4h)

    numeric_updates = {
        "local_vol_4h": vol_4h,
        "local_vol_24h": vol_24h,
        "local_vol_ratio_4h_24h": vol_ratio,
        "local_range_4h": range_4h,
        "local_range_24h": range_24h,
        "local_range_compression_4h_24h": range_compression,
        "local_position_24h": position_24h,
        "local_volume_ratio_24h": volume_24h,
        "local_quote_volume_signal": quote_signal,
        "local_quote_volume_avg_24h": quote_avg_24h,
        "local_quote_volume_ratio_24h": quote_ratio_24h,
        "signal_close_location": close_location,
        "signal_true_range": true_range_value,
        "signal_atr_4h": atr_4h,
        "signal_range_over_atr_4h": range_over_atr,
        "side_adverse_path_1h": adverse_path_1h,
        "side_adverse_path_4h": adverse_path_4h,
        "side_favorable_path_1h": favorable_path_1h,
        "side_favorable_path_4h": favorable_path_4h,
        "local_upper_wick_1h": upper_1h,
        "local_lower_wick_1h": lower_1h,
        "local_upper_wick_4h": upper_4h,
        "local_lower_wick_4h": lower_4h,
        "side_adverse_wick_4h": adverse_wick_4h,
        "side_favorable_wick_4h": favorable_wick_4h,
        "side_wick_pressure_4h": pressure_4h,
    }
    for key, value in numeric_updates.items():
        out[key] = round_float(value)
    out["side_adverse_wick_count_1h"] = adverse_count_1h
    out["side_adverse_wick_count_4h"] = adverse_count_4h

    out["local_vol_4h_bin"] = vol_bin(vol_4h)
    out["local_vol_24h_bin"] = vol_bin(vol_24h)
    out["local_vol_ratio_4h_24h_bin"] = ratio_regime_bin(vol_ratio)
    out["local_range_24h_bin"] = range_bin(range_24h)
    out["local_range_compression_4h_24h_bin"] = range_compression_bin(range_compression)
    out["local_position_24h_bin"] = position_bin(position_24h)
    out["local_volume_24h_bin"] = volume_ratio_bin(volume_24h)
    out["local_quote_volume_24h_bin"] = quote_volume_bin(quote_avg_24h)
    out["local_quote_volume_ratio_24h_bin"] = volume_ratio_bin(quote_ratio_24h)
    out["local_liquidity_stress_bin"] = liquidity_stress_bin(quote_avg_24h, quote_ratio_24h, range_over_atr)
    out["signal_close_location_bin"] = position_bin(close_location)
    out["signal_range_over_atr_4h_bin"] = atr_ratio_bin(range_over_atr)
    out["signal_spike_reversal"] = spike_reversal_type(side, close_location, upper, lower, range_over_atr, quote_ratio_24h)
    out["side_adverse_path_1h_bin"] = path_excursion_bin(adverse_path_1h)
    out["side_adverse_path_4h_bin"] = path_excursion_bin(adverse_path_4h)
    out["side_favorable_path_1h_bin"] = path_excursion_bin(favorable_path_1h)
    out["side_favorable_path_4h_bin"] = path_excursion_bin(favorable_path_4h)
    out["side_adverse_wick_cluster_1h_bin"] = wick_cluster_bin(adverse_count_1h, lag_1h)
    out["side_adverse_wick_cluster_4h_bin"] = wick_cluster_bin(adverse_count_4h, lag_4h)
    out["local_upper_wick_1h_bin"] = wick_bin(upper_1h)
    out["local_lower_wick_1h_bin"] = wick_bin(lower_1h)
    out["local_upper_wick_4h_bin"] = wick_bin(upper_4h)
    out["local_lower_wick_4h_bin"] = wick_bin(lower_4h)
    out["side_adverse_wick_4h_bin"] = wick_bin(adverse_wick_4h)
    out["side_favorable_wick_4h_bin"] = wick_bin(favorable_wick_4h)
    out["side_wick_pressure_4h_bin"] = wick_pressure_bin(pressure_4h)
    if cache_key is not None:
        cached_values = {
            key: value
            for key, value in out.items()
            if key not in row_keys and key not in {"signal_strength_bin", "hold_bucket"}
        }
        cached_values["_local_cached_side"] = side
        cached_values["_local_raw"] = {
            "_local_upper_wick_count_1h": local_wick_threshold_count(series, series_index, lag_1h, "upper"),
            "_local_lower_wick_count_1h": local_wick_threshold_count(series, series_index, lag_1h, "lower"),
            "_local_upper_wick_count_4h": local_wick_threshold_count(series, series_index, lag_4h, "upper"),
            "_local_lower_wick_count_4h": local_wick_threshold_count(series, series_index, lag_4h, "lower"),
            "_local_raw_signal_upper_wick": upper,
            "_local_raw_signal_lower_wick": lower,
            "_local_raw_ret_4h": ret_4h,
            "_local_raw_ret_24h": ret_24h,
            "_local_raw_trend_4h": trend_4h,
            "_local_raw_trend_24h": trend_24h,
            "_local_raw_upper_wick_4h": upper_4h,
            "_local_raw_lower_wick_4h": lower_4h,
            "_local_raw_side_adverse_path_1h": adverse_path_1h,
            "_local_raw_side_adverse_path_4h": adverse_path_4h,
            "_local_raw_side_favorable_path_1h": favorable_path_1h,
            "_local_raw_side_favorable_path_4h": favorable_path_4h,
            "_local_raw_close_location": close_location,
            "_local_raw_range_over_atr": range_over_atr,
            "_local_raw_quote_ratio_24h": quote_ratio_24h,
        }
        cache[cache_key] = cached_values
    out["signal_strength_bin"] = strength_bin(strength_value)
    out["hold_bucket"] = hold_bucket(hold_value)
    return out

def calendar_features_for_entry(entry_time, cache=None):
    cache_key = None
    if isinstance(cache, dict):
        cache_key = ("calendar_features", int(entry_time or 0))
        cached = cache.get(cache_key)
        if cached is not None:
            return cached

    dt_bj = datetime.fromtimestamp(entry_time / 1000, BJ_TZ)
    hour = dt_bj.hour
    weekday = dt_bj.weekday()
    out = {
        "bjt_weekday": weekday,
        "bjt_hour": hour,
        "hour_bucket": hour_bucket(hour),
        "bjt_session": bjt_session(hour),
        "bjt_weekpart": bjt_weekpart(weekday),
        "funding_window": funding_window(hour),
        "session_transition": session_transition(hour),
        "entry_day_utc": datetime.fromtimestamp(entry_time / 1000, UTC).strftime("%Y-%m-%d"),
        "entry_month_utc": datetime.fromtimestamp(entry_time / 1000, UTC).strftime("%Y-%m"),
    }
    if cache_key is not None:
        cache[cache_key] = out
    return out

def apply_local_side_features_from_cache(out, side, timeframe):
    cached_side = str(out.get("_local_cached_side") or "")
    upper = local_cached_raw(out, "_local_raw_signal_upper_wick", "signal_upper_wick")
    lower = local_cached_raw(out, "_local_raw_signal_lower_wick", "signal_lower_wick")
    adverse_signal = side_adverse_wick(upper, lower, side)
    favorable_signal = side_favorable_wick(upper, lower, side)
    out["side_signal_adverse_wick"] = round_float(adverse_signal)
    out["side_signal_favorable_wick"] = round_float(favorable_signal)
    out["side_signal_adverse_wick_bin"] = wick_bin(adverse_signal)
    out["side_signal_favorable_wick_bin"] = wick_bin(favorable_signal)

    trend_4h = local_cached_raw(out, "_local_raw_trend_4h", "local_trend_slope_4h")
    trend_24h = local_cached_raw(out, "_local_raw_trend_24h", "local_trend_slope_24h")
    ret_4h = local_cached_raw(out, "_local_raw_ret_4h", "local_ret_4h")
    ret_24h = local_cached_raw(out, "_local_raw_ret_24h", "local_ret_24h")
    out["side_trend_slope_4h"] = round_float(side_adjusted(trend_4h, side))
    out["side_trend_slope_24h"] = round_float(side_adjusted(trend_24h, side))
    out["side_local_ret_4h_bin"] = return_bin(side_adjusted(ret_4h, side))
    out["side_local_ret_24h_bin"] = return_bin(side_adjusted(ret_24h, side))
    out["side_trend_slope_4h_bin"] = return_bin(side_adjusted(trend_4h, side))
    out["side_trend_slope_24h_bin"] = return_bin(side_adjusted(trend_24h, side))

    old_adverse_1h = local_cached_raw(out, "_local_raw_side_adverse_path_1h", "side_adverse_path_1h")
    old_favorable_1h = local_cached_raw(out, "_local_raw_side_favorable_path_1h", "side_favorable_path_1h")
    old_adverse_4h = local_cached_raw(out, "_local_raw_side_adverse_path_4h", "side_adverse_path_4h")
    old_favorable_4h = local_cached_raw(out, "_local_raw_side_favorable_path_4h", "side_favorable_path_4h")
    if side in {"long", "short"} and cached_side in {"long", "short"} and side != cached_side:
        adverse_path_1h, favorable_path_1h = old_favorable_1h, old_adverse_1h
        adverse_path_4h, favorable_path_4h = old_favorable_4h, old_adverse_4h
    else:
        adverse_path_1h, favorable_path_1h = old_adverse_1h, old_favorable_1h
        adverse_path_4h, favorable_path_4h = old_adverse_4h, old_favorable_4h
    out["side_adverse_path_1h"] = round_float(adverse_path_1h)
    out["side_adverse_path_4h"] = round_float(adverse_path_4h)
    out["side_favorable_path_1h"] = round_float(favorable_path_1h)
    out["side_favorable_path_4h"] = round_float(favorable_path_4h)
    out["side_adverse_path_1h_bin"] = path_excursion_bin(adverse_path_1h)
    out["side_adverse_path_4h_bin"] = path_excursion_bin(adverse_path_4h)
    out["side_favorable_path_1h_bin"] = path_excursion_bin(favorable_path_1h)
    out["side_favorable_path_4h_bin"] = path_excursion_bin(favorable_path_4h)

    upper_4h = local_cached_raw(out, "_local_raw_upper_wick_4h", "local_upper_wick_4h")
    lower_4h = local_cached_raw(out, "_local_raw_lower_wick_4h", "local_lower_wick_4h")
    adverse_wick_4h = side_adverse_wick(upper_4h, lower_4h, side)
    favorable_wick_4h = side_favorable_wick(upper_4h, lower_4h, side)
    pressure_4h = wick_skew(favorable_wick_4h, adverse_wick_4h)
    out["side_adverse_wick_4h"] = round_float(adverse_wick_4h)
    out["side_favorable_wick_4h"] = round_float(favorable_wick_4h)
    out["side_wick_pressure_4h"] = round_float(pressure_4h)
    out["side_adverse_wick_4h_bin"] = wick_bin(adverse_wick_4h)
    out["side_favorable_wick_4h_bin"] = wick_bin(favorable_wick_4h)
    out["side_wick_pressure_4h_bin"] = wick_pressure_bin(pressure_4h)

    minutes = timeframe_minutes(timeframe)
    lag_1h = bars_for_hours(minutes, 1)
    lag_4h = bars_for_hours(minutes, 4)
    if side == "long":
        adverse_count_1h = local_cached_private(out, "_local_upper_wick_count_1h")
        adverse_count_4h = local_cached_private(out, "_local_upper_wick_count_4h")
    elif side == "short":
        adverse_count_1h = local_cached_private(out, "_local_lower_wick_count_1h")
        adverse_count_4h = local_cached_private(out, "_local_lower_wick_count_4h")
    else:
        adverse_count_1h = None
        adverse_count_4h = None
    out["side_adverse_wick_count_1h"] = adverse_count_1h
    out["side_adverse_wick_count_4h"] = adverse_count_4h
    out["side_adverse_wick_cluster_1h_bin"] = wick_cluster_bin(adverse_count_1h, lag_1h)
    out["side_adverse_wick_cluster_4h_bin"] = wick_cluster_bin(adverse_count_4h, lag_4h)
    out["signal_spike_reversal"] = spike_reversal_type(
        side,
        local_cached_raw(out, "_local_raw_close_location", "signal_close_location"),
        upper,
        lower,
        local_cached_raw(out, "_local_raw_range_over_atr", "signal_range_over_atr_4h"),
        local_cached_raw(out, "_local_raw_quote_ratio_24h", "local_quote_volume_ratio_24h"),
    )
    strip_local_cache_private_fields(out)

def local_cached_raw(out, private_key, public_key):
    raw = out.get("_local_raw")
    if isinstance(raw, dict) and private_key in raw:
        return raw.get(private_key)
    value = out.get(private_key)
    return value if value is not None else out.get(public_key)

def local_cached_private(out, private_key):
    raw = out.get("_local_raw")
    if isinstance(raw, dict) and private_key in raw:
        return raw.get(private_key)
    return out.get(private_key)

def local_wick_threshold_count(series, pos, window, which):
    if series is None or pos < window - 1 or pos >= len(series.get("closes") or []):
        return None
    prefix = series["upper_threshold_prefix"] if which == "upper" else series["lower_threshold_prefix"]
    return int(prefix_window(prefix, pos - window + 1, pos))

def strip_local_cache_private_fields(out):
    if "_local_raw" in out:
        out.pop("_local_cached_side", None)
        out.pop("_local_raw", None)
        return
    for key in (
        "_local_cached_side",
        "_local_upper_wick_count_1h",
        "_local_lower_wick_count_1h",
        "_local_upper_wick_count_4h",
        "_local_lower_wick_count_4h",
        "_local_raw_signal_upper_wick",
        "_local_raw_signal_lower_wick",
        "_local_raw_ret_4h",
        "_local_raw_ret_24h",
        "_local_raw_trend_4h",
        "_local_raw_trend_24h",
        "_local_raw_upper_wick_4h",
        "_local_raw_lower_wick_4h",
        "_local_raw_side_adverse_path_1h",
        "_local_raw_side_adverse_path_4h",
        "_local_raw_side_favorable_path_1h",
        "_local_raw_side_favorable_path_4h",
        "_local_raw_close_location",
        "_local_raw_range_over_atr",
        "_local_raw_quote_ratio_24h",
    ):
        out.pop(key, None)
