"""Runtime-owned base candidate layers for the ML trade selector.

This module is a copied/adapted runtime contract for promoted research base
layers. It intentionally does not import archived external strategies or the
research platform.
"""

from __future__ import annotations

import bisect
from concurrent.futures import ProcessPoolExecutor
import math
import multiprocessing
import os

from lib.ml_trade_base_calendars import (
    BJ_TZ,
    CORE_SYMBOLS,
    DUAL_CALENDAR_SYMBOLS,
    DUAL_LONG_CALENDAR,
    DUAL_SHORT_CALENDAR,
    REV_LONG_CALENDAR,
    REV_LONG_SYMBOLS,
    V20_CAL_15M,
    V20_CAL_5M,
    V20_SYMBOLS,
    V9_CALENDAR,
    V9_COIN_QUANTILES,
    V9_SYMBOLS,
)
from lib.ml_trade_base_series import (
    lagged_return,
    ma_spread,
    momentum_series,
    overextension_series,
    pandas_module,
    prefix_sum_full,
    quantile_sorted,
    rolling_mid_rank_current,
    rolling_mid_rank_current_many,
    rolling_mid_rank_previous,
    rolling_quantile_rank,
    rolling_quantile_rank_current_right,
    rolling_quantile_rank_right,
    rolling_range,
    rolling_right_rank_current,
    rolling_right_rank_current_many,
    rolling_right_rank_previous,
    rolling_right_rank_previous_many,
    rolling_rank_series,
    rolling_mean_full,
    rolling_std,
    sma_at,
    spread_velocity,
    threshold_series,
)
from lib.ml_trade_base_values import (
    asset_key,
    bjt_weekday_hour,
    bool_param,
    close_values,
    context_candles,
    finite,
    finite_float,
    in_calendar_list,
    in_dual_calendar,
    int_param,
    normalized_side,
    num_param,
    round_float,
    runtime_coin,
    runtime_timeframe,
    symbolic_list,
    value_at,
)
from lib.ml_trade_resource_limits import memory_limited_worker_count

_ENTRY_INDICES_CACHE = {}
_PARALLEL_BASE_STATE = None


def generate_base_candidate_rows(context, universe_symbols, params):
    if not isinstance(context, dict):
        return [], {"generated_count": 0, "status": "invalid_context"}

    rows = []
    profiles = []
    for spec in base_layer_specs(universe_symbols, params):
        layer_rows, profile = generate_layer_for_symbol(context=context, **spec)
        rows.extend(layer_rows)
        profiles.append(profile)

    return base_generation_result(rows, profiles)


def generate_base_candidate_rows_for_timestamps(context, universe_symbols, params, timestamps):
    if not isinstance(context, dict):
        return [], {"generated_count": 0, "status": "invalid_context"}

    timestamps = sorted({int(timestamp) for timestamp in timestamps if int(timestamp or 0) > 0})
    specs = base_layer_specs(universe_symbols, params)
    parallel_generation = None
    if should_parallelize_base_generation(params, specs, timestamps):
        rows, profiles, parallel_generation = generate_base_layers_parallel_at_timestamps(
            context=context,
            specs=specs,
            timestamps=timestamps,
            params=params,
        )
    else:
        rows = []
        profiles = []
        parallel_generation = {"enabled": False, "reason": "below_parallel_threshold_or_unsupported"}
        for spec in specs:
            layer_rows, profile = generate_layer_for_symbol_at_timestamps(
                context=context,
                timestamps=timestamps,
                **spec,
            )
            rows.extend(layer_rows)
            profiles.append(profile)

    generation = base_generation_result(rows, profiles)
    generation[1]["parallel_base_generation"] = parallel_generation
    return generation


def should_parallelize_base_generation(params, specs, timestamps):
    if not bool_param(params, "parallel_base_layer_generation", True):
        return False
    if os.name != "posix":
        return False
    if len(specs) < 2:
        return False
    min_timestamps = int_param(params, "parallel_base_layer_min_timestamps", 256)
    if len(timestamps) < min_timestamps:
        return False
    try:
        multiprocessing.get_context("fork")
    except (RuntimeError, ValueError):
        return False
    return parallel_base_worker_count(params, len(specs)) > 1


def parallel_base_worker_count(params, spec_count):
    default_workers = min(8, max(1, int(os.cpu_count() or 1)), max(1, int(spec_count or 1)))
    try:
        configured = int(params.get("parallel_base_layer_max_workers", default_workers))
    except (TypeError, ValueError):
        configured = default_workers
    configured = max(1, min(int(configured), max(1, int(spec_count or 1)), max(1, int(os.cpu_count() or 1))))
    return memory_limited_worker_count(
        params,
        configured,
        spec_count,
        worker_memory_key="parallel_base_layer_worker_memory_gb",
        default_worker_memory_gb=1.5,
    )


def generate_base_layers_parallel_at_timestamps(*, context, specs, timestamps, params):
    workers = parallel_base_worker_count(params, len(specs))
    state = ({"candles": context.get("candles")}, list(specs), list(timestamps))
    rows = []
    profiles = []
    process_context = multiprocessing.get_context("fork")
    with ProcessPoolExecutor(
        max_workers=workers,
        mp_context=process_context,
        initializer=init_parallel_base_worker,
        initargs=(state,),
    ) as executor:
        futures = [executor.submit(parallel_base_layer_worker, index) for index in range(len(specs))]
        for future in futures:
            layer_rows, profile = future.result()
            rows.extend(layer_rows)
            profiles.append(profile)
    return rows, profiles, {"enabled": True, "workers": workers, "spec_count": len(specs)}


def init_parallel_base_worker(state):
    global _PARALLEL_BASE_STATE
    _PARALLEL_BASE_STATE = state


def parallel_base_layer_worker(index):
    state = _PARALLEL_BASE_STATE
    if state is None:
        raise RuntimeError("parallel base layer worker was not initialized")
    context, specs, timestamps = state
    return generate_layer_for_symbol_at_timestamps(
        context=context,
        timestamps=timestamps,
        **specs[int(index)],
    )


def base_generation_result(rows, profiles):
    return rows, {
        "generated_count": len(rows),
        "status": "base_layers_generated" if rows else "no_base_layer_signal",
        "profiles": profiles,
        "implemented_layers": ["v20_loose", "spread_velocity", "reversion_long", "dual_calendar", "v9"],
        "missing_layers": [],
    }


def base_layer_specs(universe_symbols, params):
    universe = set(str(symbol) for symbol in universe_symbols)
    specs = []

    v20_overrides = {
        "spread_quantile": 0.80,
        "momentum_quantile": 0.60,
        "spread_quantile_5m": 0.80,
        "momentum_quantile_5m": 0.65,
    }
    for symbol in V20_SYMBOLS:
        if symbol not in universe:
            continue
        for timeframe, hold_bars, profit_target in (("15m", 40, 0.01), ("5m", 120, 0.0)):
            specs.append(
                {
                    "symbol": symbol,
                    "timeframe": timeframe,
                    "family": "v20",
                    "layer_id": f"v20_loose:{asset_key(symbol)}:{timeframe}",
                    "hold_bars": hold_bars,
                    "params": layer_params(params, symbol, timeframe, v20_overrides),
                    "indicator_func": compute_v20_indicators,
                    "signal_func": generate_v20_signal,
                    "profit_target_pct": profit_target,
                    "use_signal_exit": False,
                }
            )

    for symbol in spread_velocity_symbols(universe_symbols, params):
        specs.append(
            {
                "symbol": symbol,
                "timeframe": "15m",
                "family": "spread_velocity",
                "layer_id": f"sv:{asset_key(symbol)}:15m",
                "hold_bars": 72,
                "params": layer_params(params, symbol, "15m", {"leverage": 2.0}),
                "indicator_func": compute_spread_velocity_indicators,
                "signal_func": generate_spread_velocity_signal,
                "profit_target_pct": 0.0,
                "use_signal_exit": False,
            }
        )

    for symbol in REV_LONG_SYMBOLS:
        if symbol not in universe:
            continue
        specs.append(
            {
                "symbol": symbol,
                "timeframe": "15m",
                "family": "reversion_long",
                "layer_id": f"revlong:{asset_key(symbol)}:15m",
                "hold_bars": 32,
                "params": layer_params(params, symbol, "15m", {}),
                "indicator_func": compute_reversion_long_indicators,
                "signal_func": generate_reversion_long_signal,
                "profit_target_pct": 0.0,
                "use_signal_exit": False,
            }
        )

    for symbol in DUAL_CALENDAR_SYMBOLS:
        if symbol not in universe:
            continue
        specs.append(
            {
                "symbol": symbol,
                "timeframe": "15m",
                "family": "dual_calendar",
                "layer_id": f"dual_calendar:{asset_key(symbol)}:15m",
                "hold_bars": 72,
                "params": layer_params(params, symbol, "15m", {"enable_trend_adaptive": False}),
                "indicator_func": compute_dual_calendar_indicators,
                "signal_func": generate_dual_calendar_signal,
                "profit_target_pct": 0.0,
                "use_signal_exit": False,
            }
        )

    for symbol in V9_SYMBOLS:
        if symbol not in universe:
            continue
        specs.append(
            {
                "symbol": symbol,
                "timeframe": "15m",
                "family": "v9",
                "layer_id": f"v9:{asset_key(symbol)}:15m",
                "hold_bars": 192,
                "params": layer_params(params, symbol, "15m", {}),
                "indicator_func": compute_v9_indicators,
                "signal_func": generate_v9_signal,
                "profit_target_pct": 0.0,
                "use_signal_exit": True,
            }
        )
    return specs


def generate_layer_for_symbol(
    *,
    context,
    symbol,
    timeframe,
    family,
    layer_id,
    hold_bars,
    params,
    indicator_func,
    signal_func,
    profit_target_pct=0.0,
    use_signal_exit=False,
):
    del use_signal_exit
    candles = context_candles(context, symbol, timeframe)
    profile = {
        "layer_id": layer_id,
        "family": family,
        "symbol": symbol,
        "timeframe": timeframe,
        "rows": len(candles),
        "candidate_rows": 0,
    }
    if len(candles) < 3:
        profile["status"] = "missing_or_short_candles"
        return [], profile

    signal_index = len(candles) - 2
    entry_index = len(candles) - 1
    try:
        indicators = indicator_func(candles, params)
        signal = signal_func(signal_index, candles, indicators, params)
    except Exception as error:  # pragma: no cover - diagnostics path.
        profile["status"] = "layer_signal_failed"
        profile["detail"] = str(error)
        return [], profile

    side = normalized_side(signal)
    if side not in {"long", "short"}:
        profile["status"] = "no_signal"
        return [], profile

    row = base_candidate_row(
        symbol=symbol,
        layer_id=layer_id,
        family=family,
        timeframe=timeframe,
        hold_bars=hold_bars,
        candles=candles,
        signal_index=signal_index,
        entry_index=entry_index,
        signal=signal,
        params=params,
        profit_target_pct=profit_target_pct,
    )
    profile["status"] = "generated"
    profile["candidate_rows"] = 1
    profile["signal_index"] = signal_index
    profile["entry_index"] = entry_index
    return [row], profile


def generate_layer_for_symbol_at_timestamps(
    *,
    context,
    symbol,
    timeframe,
    family,
    layer_id,
    hold_bars,
    params,
    indicator_func,
    signal_func,
    timestamps,
    profit_target_pct=0.0,
    use_signal_exit=False,
):
    candles = context_candles(context, symbol, timeframe)
    profile = {
        "layer_id": layer_id,
        "family": family,
        "symbol": symbol,
        "timeframe": timeframe,
        "rows": len(candles),
        "candidate_rows": 0,
        "evaluated_timestamps": len(timestamps),
    }
    if len(candles) < 3 or not timestamps:
        profile["status"] = "missing_or_short_candles"
        return [], profile

    entry_indices = []
    seen_entry_indices = set()
    for entry_index in entry_indices_at_timestamps(candles, timestamps):
        if entry_index is None or entry_index < 1 or entry_index in seen_entry_indices:
            continue
        seen_entry_indices.add(entry_index)
        entry_indices.append(entry_index)
    if not entry_indices:
        profile["status"] = "no_signal"
        return [], profile

    indicator_candles, indicator_offset = indicator_candles_for_history_layer(
        candles,
        entry_indices,
        family,
        params,
    )
    try:
        indicators = indicator_func(indicator_candles, params)
    except Exception as error:  # pragma: no cover - diagnostics path.
        profile["status"] = "layer_indicator_failed"
        profile["detail"] = str(error)
        return [], profile

    signal_cache = {}

    def signal_at(original_signal_index):
        indicator_signal_index = int(original_signal_index) - indicator_offset
        if indicator_signal_index < 0 or indicator_signal_index >= len(indicator_candles):
            return None
        if original_signal_index not in signal_cache:
            signal_cache[original_signal_index] = signal_func(
                indicator_signal_index,
                indicator_candles,
                indicators,
                params,
            )
        return signal_cache[original_signal_index]

    rows = []
    next_free = 0
    for entry_index in entry_indices:
        signal_index = entry_index - 1
        if signal_index < next_free:
            continue
        try:
            signal = signal_at(signal_index)
        except Exception as error:  # pragma: no cover - diagnostics path.
            profile["status"] = "layer_signal_failed"
            profile["detail"] = str(error)
            return rows, profile
        side = normalized_side(signal)
        if side not in {"long", "short"}:
            continue
        try:
            exit_plan = planned_exit(
                candles=candles,
                signal_at=signal_at,
                entry_index=entry_index,
                side=side,
                hold_bars=hold_bars,
                profit_target_pct=profit_target_pct,
                use_signal_exit=use_signal_exit,
            )
        except Exception as error:  # pragma: no cover - diagnostics path.
            profile["status"] = "layer_exit_plan_failed"
            profile["detail"] = str(error)
            return rows, profile
        if exit_plan is None:
            continue
        exit_index, exit_reason = exit_plan
        rows.append(
            base_candidate_row(
                symbol=symbol,
                layer_id=layer_id,
                family=family,
                timeframe=timeframe,
                hold_bars=exit_index - entry_index,
                candles=candles,
                signal_index=signal_index,
                entry_index=entry_index,
                signal=signal,
                params=params,
                profit_target_pct=profit_target_pct,
                exit_index=exit_index,
                exit_reason=exit_reason,
                planned_hold_bars=hold_bars,
            )
        )
        next_free = exit_index + 1

    profile["status"] = "generated" if rows else "no_signal"
    profile["candidate_rows"] = len(rows)
    if rows:
        profile["last_signal_index"] = rows[-1].get("signal_index")
        profile["last_entry_index"] = rows[-1].get("entry_index")
    return rows, profile


def planned_exit(*, candles, signal_at, entry_index, side, hold_bars, profit_target_pct, use_signal_exit):
    hold_exit_index = int(entry_index) + int(hold_bars)
    if hold_exit_index >= len(candles):
        return None

    exit_index = hold_exit_index
    exit_reason = "max_hold_bars"
    if use_signal_exit:
        for index in range(int(entry_index), hold_exit_index):
            exit_signal = signal_at(index)
            if is_exit_signal(exit_signal, side):
                exit_index = min(index + 1, len(candles) - 1)
                exit_reason = str(exit_signal.get("reason", "signal_exit")) if isinstance(exit_signal, dict) else "signal_exit"
                break

    if side == "short" and float(profit_target_pct or 0.0) > 0.0 and exit_reason == "max_hold_bars":
        target = float(profit_target_pct)
        entry_price = finite_float(candles[int(entry_index)].get("open"))
        if entry_price is None or entry_price <= 0.0:
            return None
        for index in range(int(entry_index), exit_index):
            low = finite_float(candles[index].get("low"))
            if low is not None and low > 0.0 and (entry_price - low) / entry_price >= target:
                exit_index = index
                exit_reason = "profit_target"
                break
    return exit_index, exit_reason


def is_exit_signal(signal, open_side):
    if not isinstance(signal, dict):
        return False
    side = str(signal.get("side", "")).lower()
    if side in {"flat", "close", "exit"}:
        return True
    if open_side == "short" and side in {"buy", "long"}:
        return True
    if open_side == "long" and side in {"sell", "short"}:
        return True
    return False


def indicator_candles_for_history_layer(candles, entry_indices, family, params):
    padding = history_layer_indicator_padding(family, params)
    if padding is None:
        return candles, 0
    signal_indices = [int(index) - 1 for index in entry_indices if index is not None and int(index) > 0]
    if not signal_indices:
        return candles, 0
    start = max(0, min(signal_indices) - int(padding))
    return candles[start:], start


def history_layer_indicator_padding(family, params):
    if family == "spread_velocity":
        lookback = int_param(params, "threshold_lookback", 5000)
        source_window = max(
            int_param(params, "fast_window", 120),
            int_param(params, "slow_window", 360),
            int_param(params, "risk_momentum_window", 16),
            int_param(params, "risk_range_window", 96),
            int_param(params, "risk_vol_window", 96),
            4,
        )
        return lookback + source_window + 2
    if family == "dual_calendar":
        lookback = int_param(params, "threshold_lookback", 5000)
        source_window = max(
            int_param(params, "fast_window", 120),
            int_param(params, "slow_window", 360),
            int_param(params, "momentum_window", 144),
        )
        return lookback + source_window + 2
    if family == "reversion_long":
        lookback = int_param(params, "threshold_lookback", 5000)
        source_window = max(
            int_param(params, "zscore_lookback", 480),
            int_param(params, "zscore_window", 4),
            int_param(params, "volume_window", 96),
        )
        return lookback + source_window + 2
    return None


def entry_indices_at_timestamps(candles, timestamps):
    if not candles or not timestamps:
        return []
    key = (
        id(candles),
        len(candles),
        int(candles[0].get("timestamp", 0) or 0) if isinstance(candles[0], dict) else 0,
        int(candles[-1].get("timestamp", 0) or 0) if isinstance(candles[-1], dict) else 0,
        id(timestamps),
        len(timestamps),
        int(timestamps[0] or 0),
        int(timestamps[-1] or 0),
    )
    cached = _ENTRY_INDICES_CACHE.get(key)
    if cached is not None:
        return list(cached)
    candle_timestamps = [int(candle.get("timestamp", 0) or 0) for candle in candles]
    out = []
    for timestamp in timestamps:
        index = bisect.bisect_right(candle_timestamps, int(timestamp)) - 1
        out.append(index if index >= 0 else None)
    if len(_ENTRY_INDICES_CACHE) > 256:
        _ENTRY_INDICES_CACHE.clear()
    _ENTRY_INDICES_CACHE[key] = tuple(out)
    return out


def base_candidate_row(
    *,
    symbol,
    layer_id,
    family,
    timeframe,
    hold_bars,
    candles,
    signal_index,
    entry_index,
    signal,
    params,
    profit_target_pct=0.0,
    exit_index=None,
    exit_reason=None,
    planned_hold_bars=None,
):
    entry_time = int(candles[entry_index].get("timestamp", 0) or 0)
    signal_time = int(candles[signal_index].get("timestamp", 0) or 0)
    row = {
        "symbol": symbol,
        "asset": symbol,
        "layer_id": layer_id,
        "family": family,
        "timeframe": timeframe,
        "side": normalized_side(signal),
        "signal_index": signal_index,
        "entry_index": entry_index,
        "signal_time": signal_time,
        "entry_time": entry_time,
        "feature_bar_time": signal_time,
        "timestamp": entry_time,
        "hold_bars": int(hold_bars),
        "planned_hold_bars": int(planned_hold_bars if planned_hold_bars is not None else hold_bars),
        "strength": round_float(float(signal.get("strength", 0.0) or 0.0)),
        "reason": str(signal.get("reason", "")),
        "entry_price": finite_float(candles[entry_index].get("open")),
        "leverage": round_float(params.get("leverage", 3.0)),
        "profit_target_pct": round_float(profit_target_pct),
        "round_trip_cost_bps": round_float(params.get("round_trip_cost_bps", 16.0)),
        "candidate_source": "runtime_base_layer",
    }
    if exit_index is not None:
        row["exit_index"] = int(exit_index)
        row["exit_time"] = int(candles[int(exit_index)].get("timestamp", 0) or 0)
        row["exit_reason"] = str(exit_reason or "max_hold_bars")
    return row


def layer_params(params, symbol, timeframe, overrides):
    out = dict(params or {})
    out.update(overrides or {})
    out["_runtime_symbol"] = symbol
    out["_runtime_timeframe"] = timeframe
    out["symbol"] = symbol
    out["timeframe"] = timeframe
    return out


def spread_velocity_symbols(universe_symbols, params):
    value = params.get("spread_velocity_symbols") if isinstance(params, dict) else None
    if value:
        requested = symbolic_list(value)
        available = set(str(symbol) for symbol in universe_symbols)
        return [symbol for symbol in requested if symbol in available]
    return [str(symbol) for symbol in universe_symbols]


def compute_v20_indicators(candles, params):
    timeframe = runtime_timeframe(params)
    if timeframe == "5m":
        return compute_overextension_threshold_indicators(candles, params, "5m", 360, 1080, 432, 3000, 1000, 0.80, 0.65)
    return compute_overextension_threshold_indicators(candles, params, "15m", 120, 360, 144, 5000, 4500, 0.82, 0.63)


def generate_v20_signal(index, candles, indicators, params):
    timeframe = runtime_timeframe(params)
    calendar = V20_CAL_5M if timeframe == "5m" else V20_CAL_15M
    if index < int(indicators.get("warmup", 0) or 0):
        return None
    ma_spread = value_at(indicators, "ma_spread", index)
    momentum = value_at(indicators, "momentum_return", index)
    spread_threshold = value_at(indicators, "spread_threshold", index)
    momentum_threshold = value_at(indicators, "mom_threshold", index)
    if not all(finite(item) for item in [ma_spread, momentum, spread_threshold, momentum_threshold]):
        return None
    coin = runtime_coin(params)
    wd, hour = bjt_weekday_hour(candles[index])
    if not coin or not in_calendar_list(calendar, coin, wd, hour):
        return None
    if ma_spread >= spread_threshold and momentum >= momentum_threshold:
        return {"side": "short", "price": finite_float(candles[index].get("close")), "reason": f"{timeframe}:{coin} wd{wd}h{hour}", "strength": 0.5}
    return None


def compute_v9_indicators(candles, params):
    fast = int_param(params, "fast_window", 120)
    slow = int_param(params, "slow_window", 360)
    mom_window = int_param(params, "momentum_window", 144)
    lookback = int_param(params, "threshold_lookback", 5000)
    min_samples = int_param(params, "min_threshold_samples", 4500)
    exit_q = num_param(params, "exit_quantile", 0.50)
    closes = close_values(candles)
    ma_spread, momentum = overextension_series(closes, fast, slow, mom_window)
    indicators = {
        "ma_spread": ma_spread,
        "momentum_return": momentum,
        "exit_threshold": threshold_series(ma_spread, exit_q, min_samples, max(slow, mom_window, min_samples), lookback),
        "warmup": max(slow, mom_window, min_samples),
    }
    runtime = runtime_coin(params)
    quantile_items = (
        [(runtime, V9_COIN_QUANTILES[runtime])]
        if runtime in V9_COIN_QUANTILES
        else list(V9_COIN_QUANTILES.items())
    )
    for coin, quantiles in quantile_items:
        spread_q, mom_q = quantiles
        indicators[f"{coin}_spread_th"] = threshold_series(ma_spread, spread_q, min_samples, indicators["warmup"], lookback)
        indicators[f"{coin}_mom_th"] = threshold_series([max(0.0, item) for item in momentum], mom_q, min_samples, indicators["warmup"], lookback, floor_zero=True)
    return indicators


def generate_v9_signal(index, candles, indicators, params):
    if index < int(indicators.get("warmup", 0) or 0):
        return None
    ma_spread = value_at(indicators, "ma_spread", index)
    momentum = value_at(indicators, "momentum_return", index)
    exit_threshold = value_at(indicators, "exit_threshold", index)
    if not all(finite(item) for item in [ma_spread, momentum, exit_threshold]):
        return None
    if ma_spread < exit_threshold:
        return None
    coin = runtime_coin(params)
    wd, hour = bjt_weekday_hour(candles[index])
    if not coin or not in_calendar_list(V9_CALENDAR, coin, wd, hour):
        return None
    spread_threshold = value_at(indicators, f"{coin}_spread_th", index)
    momentum_threshold = value_at(indicators, f"{coin}_mom_th", index)
    if spread_threshold is None or momentum_threshold is None:
        return None
    if ma_spread >= spread_threshold and momentum >= momentum_threshold:
        headroom = ma_spread - spread_threshold
        strength = min(1.0, 0.3 + 0.7 * headroom / max(0.001, abs(spread_threshold) * 0.3 + 0.001))
        return {"side": "short", "price": finite_float(candles[index].get("close")), "reason": f"v9:{coin} wd{wd}h{hour}", "strength": strength}
    return None


def compute_spread_velocity_indicators(candles, params):
    closes = close_values(candles)
    highs = [finite_float(item.get("high")) or 0.0 for item in candles]
    lows = [finite_float(item.get("low")) or 0.0 for item in candles]
    fast = int_param(params, "fast_window", 120)
    slow = int_param(params, "slow_window", 360)
    lookback = int_param(params, "threshold_lookback", 5000)
    min_samples = int_param(params, "min_threshold_samples", 1000)
    spread = ma_spread(closes, fast, slow)
    velocity = spread_velocity(spread, 4)
    one_bar_return = lagged_return(closes, 1)
    ret_4h = lagged_return(closes, int_param(params, "risk_momentum_window", 16))
    range_24h = rolling_range(highs, lows, int_param(params, "risk_range_window", 96))
    vol_24h = rolling_std(one_bar_return, int_param(params, "risk_vol_window", 96))
    ranks = rolling_right_rank_previous_many(
        {
            "spread": spread,
            "velocity": velocity,
            "risk_return": ret_4h,
            "risk_range": range_24h,
            "risk_vol": vol_24h,
        },
        lookback,
        min_samples,
    )
    return {
        "spread_rank": ranks["spread"],
        "velocity_rank": ranks["velocity"],
        "risk_return_rank": ranks["risk_return"],
        "risk_range_rank": ranks["risk_range"],
        "risk_vol_rank": ranks["risk_vol"],
        "warmup": min_samples,
    }


def generate_spread_velocity_signal(index, candles, indicators, params):
    if index < int(indicators.get("warmup", 1000)):
        return None
    spread_rank = value_at(indicators, "spread_rank", index)
    velocity_rank = value_at(indicators, "velocity_rank", index)
    if not all(finite(item) for item in [spread_rank, velocity_rank]):
        return None
    if bool_param(params, "enable_risk_filter", True):
        ceiling = num_param(params, "risk_filter_quantile", 0.90)
        for key in ("risk_return_rank", "risk_range_rank", "risk_vol_rank"):
            value = value_at(indicators, key, index)
            if value is None or not finite(value) or value > ceiling:
                return None
    spread_q = num_param(params, "spread_quantile", 0.90)
    velocity_q = num_param(params, "velocity_quantile", 0.30)
    if spread_rank >= spread_q and velocity_rank <= velocity_q:
        coin = runtime_coin(params) or asset_key(params.get("_runtime_symbol", "")).upper()
        wd, hour = bjt_weekday_hour(candles[index])
        strength = min(1.0, (spread_rank - spread_q) + (velocity_q - velocity_rank))
        return {"side": "short", "price": finite_float(candles[index].get("close")), "reason": f"{coin} spread+velocity SHORT wd{wd}h{hour}", "strength": strength}
    return None


def compute_reversion_long_indicators(candles, params):
    z_window = int_param(params, "zscore_window", 4)
    z_lookback = int_param(params, "zscore_lookback", 480)
    volume_window = int_param(params, "volume_window", 96)
    rev_q = num_param(params, "reversion_quantile", 0.90)
    vol_q = num_param(params, "volume_quantile", 0.65)
    lookback = int_param(params, "threshold_lookback", 5000)
    min_samples = int_param(params, "min_threshold_samples", 500)
    closes = close_values(candles)
    volumes = [finite_float(item.get("volume")) or 0.0 for item in candles]
    bar_returns = [0.0]
    for index in range(1, len(closes)):
        bar_returns.append(closes[index] / closes[index - 1] - 1.0 if closes[index - 1] > 0.0 else 0.0)
    return_prefix = prefix_sum_full(bar_returns)
    return_sq_prefix = prefix_sum_full([value * value for value in bar_returns])
    reversion = []
    for index in range(len(closes)):
        if index < z_window or index < z_lookback - 1:
            reversion.append(0.0)
            continue
        start = max(0, index - z_lookback + 1)
        count = index - start + 1
        total = return_prefix[index + 1] - return_prefix[start]
        total_sq = return_sq_prefix[index + 1] - return_sq_prefix[start]
        mean = total / count
        variance = total_sq / count - mean * mean
        stdev = math.sqrt(variance) if variance > 0.0 else 0.001
        short_ret = closes[index] / closes[index - z_window] - 1.0 if closes[index - z_window] > 0.0 else 0.0
        reversion.append(max(-5.0, min(5.0, -(short_ret - mean) / stdev)))
    volume_baseline = rolling_mean_full(volumes, volume_window)
    volume_factor = [
        value / volume_baseline[index] if volume_baseline[index] > 0.0 else 1.0
        for index, value in enumerate(volumes)
    ]
    warmup = max(z_lookback, volume_window, min_samples)
    return {
        "reversion_factor": reversion,
        "volume_factor": volume_factor,
        "rev_threshold": threshold_series(reversion, rev_q, min_samples, 0, lookback),
        "vol_threshold": threshold_series(volume_factor, vol_q, min_samples, 0, lookback),
        "warmup": warmup,
    }


def generate_reversion_long_signal(index, candles, indicators, params):
    if index < int(indicators.get("warmup", 500)):
        return None
    coin = runtime_coin(params)
    wd, hour = bjt_weekday_hour(candles[index])
    if not coin or not in_calendar_list(REV_LONG_CALENDAR, coin, wd, hour):
        return None
    rev = value_at(indicators, "reversion_factor", index)
    vol = value_at(indicators, "volume_factor", index)
    rev_threshold = value_at(indicators, "rev_threshold", index)
    vol_threshold = value_at(indicators, "vol_threshold", index)
    if not all(finite(item) for item in [rev, vol, rev_threshold, vol_threshold]):
        return None
    if rev >= rev_threshold and vol >= vol_threshold:
        strength = min(1.0, (rev - rev_threshold) / max(0.001, abs(rev_threshold)))
        return {"side": "long", "price": finite_float(candles[index].get("close")), "reason": f"LONG:{coin} wd{wd}h{hour}", "strength": strength}
    return None


def compute_dual_calendar_indicators(candles, params):
    closes = close_values(candles)
    fast = int_param(params, "fast_window", 120)
    slow = int_param(params, "slow_window", 360)
    mom_window = int_param(params, "momentum_window", 144)
    lookback = int_param(params, "threshold_lookback", 5000)
    min_samples = int_param(params, "min_threshold_samples", 1000)
    spread = ma_spread(closes, fast, slow)
    momentum = momentum_series(closes, mom_window)
    ranks = rolling_right_rank_current_many(
        {
            "spread": spread,
            "momentum": momentum,
        },
        lookback,
        min_samples,
    )
    return {
        "spread_rank": ranks["spread"],
        "mom_rank": ranks["momentum"],
        "warmup": max(slow, mom_window, min_samples),
    }


def generate_dual_calendar_signal(index, candles, indicators, params):
    if index < int(params.get("threshold_lookback", indicators.get("warmup", 5000))):
        return None
    coin = runtime_coin(params)
    wd, hour = bjt_weekday_hour(candles[index])
    spread_rank = value_at(indicators, "spread_rank", index)
    mom_rank = value_at(indicators, "mom_rank", index)
    if coin is None or not all(finite(item) for item in [spread_rank, mom_rank]):
        return None
    short_spread_q = num_param(params, "short_spread_quantile", 0.90)
    short_mom_q = num_param(params, "short_mom_quantile", 0.70)
    long_spread_q = num_param(params, "long_spread_quantile", 0.10)
    long_mom_q = num_param(params, "long_mom_quantile", 0.30)
    if spread_rank >= short_spread_q and mom_rank >= short_mom_q and in_dual_calendar(DUAL_SHORT_CALENDAR, coin, wd, hour):
        strength = min(1.0, (spread_rank - short_spread_q) + (mom_rank - short_mom_q))
        return {"side": "short", "price": finite_float(candles[index].get("close")), "reason": f"{coin} overextension SHORT wd{wd}h{hour}", "strength": strength}
    if spread_rank <= long_spread_q and mom_rank <= long_mom_q and in_dual_calendar(DUAL_LONG_CALENDAR, coin, wd, hour):
        strength = min(1.0, (long_spread_q - spread_rank) + (long_mom_q - mom_rank))
        return {"side": "long", "price": finite_float(candles[index].get("close")), "reason": f"{coin} underextension LONG wd{wd}h{hour}", "strength": strength}
    return None


def compute_overextension_threshold_indicators(candles, params, timeframe, fast_default, slow_default, mom_default, lookback_default, min_default, spread_q_default, mom_q_default):
    fast = int_param(params, f"fast_window_{timeframe}", int_param(params, "fast_window", fast_default))
    slow = int_param(params, f"slow_window_{timeframe}", int_param(params, "slow_window", slow_default))
    mom_window = int_param(params, f"momentum_window_{timeframe}", int_param(params, "momentum_window", mom_default))
    lookback = int_param(params, f"threshold_lookback_{timeframe}", int_param(params, "threshold_lookback", lookback_default))
    min_samples = int_param(params, f"min_threshold_samples_{timeframe}", int_param(params, "min_threshold_samples", min_default))
    spread_q = num_param(params, f"spread_quantile_{timeframe}", num_param(params, "spread_quantile", spread_q_default))
    momentum_q = num_param(params, f"momentum_quantile_{timeframe}", num_param(params, "momentum_quantile", mom_q_default))
    closes = close_values(candles)
    spread, momentum = overextension_series(closes, fast, slow, mom_window)
    warmup = max(slow, mom_window, min_samples)
    return {
        "ma_spread": spread,
        "momentum_return": momentum,
        "spread_threshold": threshold_series(spread, spread_q, min_samples, warmup, lookback),
        "mom_threshold": threshold_series([max(0.0, item) for item in momentum], momentum_q, min_samples, warmup, lookback, floor_zero=True),
        "warmup": warmup,
    }
