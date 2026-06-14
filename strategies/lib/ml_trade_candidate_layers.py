"""Runtime-owned candidate layers for ML trade selector strategies.

These helpers are copied/adapted from promoted research code, but they live
under `strategies/` and do not import from the research platform. They generate
point-in-time candidate rows only; feature construction and model scoring are
handled by other runtime modules.
"""

from __future__ import annotations

from collections import deque
from concurrent.futures import ProcessPoolExecutor
import multiprocessing
import os
import math

from lib.ml_trade_base_layers import (
    base_generation_result,
    base_layer_specs,
    entry_indices_at_timestamps,
    generate_base_candidate_rows,
    generate_base_candidate_rows_for_timestamps,
    generate_layer_for_symbol_at_timestamps,
    rolling_mid_rank_current,
    rolling_mid_rank_current_many,
)
from lib.ml_trade_base_values import (
    asset_key,
    context_candles,
    finite,
    finite_float,
    int_param,
    num_param,
    round_float,
    symbolic_list,
)
from lib.ml_trade_feature_builder import (
    add_btc_context,
    add_cross_sectional_context,
    add_funding_context,
    add_local_and_calendar_features,
    btc_series_from_candles,
    build_funding_context_from_runtime,
    build_market_context_from_runtime,
)
from lib.ml_trade_resource_limits import memory_limited_worker_count


DEFAULT_UNIVERSE_CANDIDATE_PARAMS = {
    "ret_fast_bars": 16,
    "ret_slow_bars": 96,
    "range_bars": 96,
    "vol_bars": 96,
    "volume_bars": 96,
    "rank_lookback": 5000,
    "min_rank_samples": 1000,
    "fast_hi_rank": 0.75,
    "fast_lo_rank": 0.25,
    "slow_hi_rank": 0.50,
    "slow_lo_rank": 0.50,
    "volume_min_rank": 0.20,
    "range_max_rank": 1.0,
    "vol_max_rank": 1.0,
    "leverage": 3.0,
    "round_trip_cost_bps": 16.0,
}


_PARALLEL_UNIVERSE_STATE = None
_PARALLEL_BASE_ENRICH_STATE = None


def runtime_feature_cache(context):
    runtime_cache = context.get("_runtime_cache") if isinstance(context, dict) else None
    if not isinstance(runtime_cache, dict):
        return {}
    cache = runtime_cache.setdefault("feature_context_cache", {})
    cache["_runtime_cache"] = runtime_cache
    return cache


def runtime_cached_rows(feature_cache, symbol, timeframe):
    runtime_cache = feature_cache.get("_runtime_cache") if isinstance(feature_cache, dict) else None
    candles = runtime_cache.get("candles") if isinstance(runtime_cache, dict) else None
    payload = candles.get((symbol, timeframe)) if isinstance(candles, dict) else None
    rows = payload.get("rows") if isinstance(payload, dict) else None
    return rows if isinstance(rows, list) else None


def row_timestamp(row, fallback=0):
    if not isinstance(row, dict):
        return fallback
    try:
        return int(row.get("timestamp") or row.get("funding_time") or fallback)
    except (TypeError, ValueError):
        return fallback


def row_series_signature(rows):
    if not isinstance(rows, list):
        return None
    if not rows:
        return (0, 0, 0)
    return len(rows), row_timestamp(rows[0]), row_timestamp(rows[-1])


def timestamp_signature(timestamps):
    if not timestamps:
        return None
    values = []
    for timestamp in timestamps:
        try:
            parsed = int(timestamp or 0)
        except (TypeError, ValueError):
            continue
        if parsed > 0:
            values.append(parsed)
    if not values:
        return None
    return len(values), min(values), max(values)


def symbol_key(symbols):
    return tuple(sorted(str(symbol).strip() for symbol in symbols if str(symbol).strip()))


def cached_btc_context(context, timestamps, feature_cache):
    cache = cache_section(feature_cache, "context")
    if timestamps:
        rows = runtime_cached_rows(feature_cache, "BTC-USDT-SWAP", "15m")
        key = ("btc_history", "BTC-USDT-SWAP", "15m", id(rows), timestamp_signature(timestamps))
    else:
        rows = context_candles(context, "BTC-USDT-SWAP", "15m")
        key = ("btc_latest", "BTC-USDT-SWAP", "15m", row_series_signature(rows))
    if rows is not None and isinstance(cache, dict):
        cached = cache.get(key)
        if cached is None:
            cached = btc_series_from_candles(rows, timestamps=timestamps)
            cache[key] = cached
        return cached
    return btc_series_from_candles(context_candles(context, "BTC-USDT-SWAP", "15m"), timestamps=timestamps)


def market_context_signature(context, symbols):
    candles = context.get("candles") if isinstance(context, dict) else {}
    if not isinstance(candles, dict):
        return None
    signature = []
    for symbol in sorted(str(item) for item in symbols):
        timeframe_map = candles.get(symbol)
        rows = timeframe_map.get("15m") if isinstance(timeframe_map, dict) else None
        signature.append((symbol, row_series_signature(rows)))
    return tuple(signature)


def cached_market_context(context, symbols, params, timestamps, feature_cache):
    cache = cache_section(feature_cache, "context")
    runtime_cache = feature_cache.get("_runtime_cache") if isinstance(feature_cache, dict) else None
    timestamp_key = timestamp_signature(timestamps)
    if timestamp_key is None:
        key = (
            "market_latest",
            symbol_key(symbols),
            int_param(params, "min_market_context_count", 20),
            market_context_signature(context, symbols),
        )
    else:
        key = (
            "market_history",
            id(runtime_cache),
            symbol_key(symbols),
            int_param(params, "min_market_context_count", 20),
            timestamp_key,
        )
    if isinstance(cache, dict):
        cached = cache.get(key)
        if cached is None:
            cached = build_market_context_from_runtime(
                context,
                symbols,
                min_symbol_count=int_param(params, "min_market_context_count", 20),
                timestamps=timestamps,
            )
            cache[key] = cached
        return cached
    return build_market_context_from_runtime(
        context,
        symbols,
        min_symbol_count=int_param(params, "min_market_context_count", 20),
        timestamps=timestamps,
    )


def funding_context_signature(context, symbols):
    funding = context.get("funding") if isinstance(context, dict) else {}
    if not isinstance(funding, dict):
        return None
    signature = []
    for symbol in sorted(str(item) for item in symbols):
        payload = funding.get(symbol)
        if not isinstance(payload, dict):
            signature.append((symbol, None))
            continue
        history = payload.get("history")
        history_signature = row_series_signature(history) if isinstance(history, list) else None
        latest = payload.get("latest")
        latest_timestamp = row_timestamp(latest) if isinstance(latest, dict) else 0
        signature.append((symbol, history_signature, latest_timestamp))
    return tuple(signature)


def cached_funding_context(context, symbols, feature_cache):
    cache = cache_section(feature_cache, "context")
    key = (
        "funding",
        symbol_key(symbols),
        funding_context_signature(context, symbols),
    )
    if isinstance(cache, dict):
        cached = cache.get(key)
        if cached is None:
            cached = build_funding_context_from_runtime(context, symbols)
            cache[key] = cached
        return cached
    return build_funding_context_from_runtime(context, symbols)


def same_series_prefix(candles, rows):
    if not isinstance(candles, list) or not isinstance(rows, list):
        return False
    if len(candles) > len(rows):
        return False
    if not candles:
        return True
    first = int(candles[0].get("timestamp", 0) or 0) if isinstance(candles[0], dict) else 0
    expected_first = int(rows[0].get("timestamp", 0) or 0) if rows and isinstance(rows[0], dict) else 0
    last = int(candles[-1].get("timestamp", 0) or 0) if isinstance(candles[-1], dict) else 0
    expected_last = int(rows[len(candles) - 1].get("timestamp", 0) or 0) if isinstance(rows[len(candles) - 1], dict) else 0
    return first == expected_first and last == expected_last


def universe_indicator_signature(params):
    keys = (
        "ret_fast_bars",
        "ret_slow_bars",
        "range_bars",
        "vol_bars",
        "volume_bars",
        "rank_lookback",
        "min_rank_samples",
    )
    return tuple((key, params.get(key)) for key in keys)


def universe_candidate_indicators_for_signal_indices(symbol, candles, signal_indices, params, feature_cache):
    rows = runtime_cached_rows(feature_cache, symbol, "15m")
    if rows is not None and same_series_prefix(candles, rows):
        cache = cache_section(feature_cache, "universe_indicators")
        key = (symbol, id(rows), universe_indicator_signature(params))
        indicators = cache.get(key) if isinstance(cache, dict) else None
        if indicators is None:
            indicators = compute_universe_candidate_indicators(rows, params)
            if isinstance(cache, dict):
                cache[key] = indicators
        return rows, 0, indicators

    indicator_candles, indicator_offset = indicator_slice_for_signal_indices(
        candles,
        signal_indices,
        universe_indicator_padding(params),
    )
    indicators = compute_universe_candidate_indicators(indicator_candles, params)
    return indicator_candles, indicator_offset, indicators


def generate_runtime_candidate_rows(context, symbols, params, progress_callback=None):
    """Generate current-entry candidate rows from runtime candles.

    This covers the promoted base layers plus the broad `universe_candidate_v1`
    breakout/fade sleeve. It still needs parity tests against frozen research
    rows before being treated as full research-equivalent runtime generation.
    """

    if not isinstance(context, dict):
        return [], {"generated_count": 0, "status": "invalid_context"}

    modes = symbolic_list(params.get("universe_candidate_modes", "breakout,fade"))
    holds = int_list(params.get("universe_candidate_holds", "32,72"))
    rows = []
    profiles = []
    feature_cache = runtime_feature_cache(context)
    report_candidate_progress(
        progress_callback,
        0.05,
        "candidate_context",
        "ML selector: building latest candidate context",
    )
    btc_context = cached_btc_context(context, None, feature_cache)
    market_context = cached_market_context(context, symbols, params, None, feature_cache)
    funding_context = cached_funding_context(context, symbols, feature_cache)
    report_candidate_progress(
        progress_callback,
        0.15,
        "base_layer_generation",
        "ML selector: generating latest base candidate layers",
    )
    base_rows, base_generation = generate_base_candidate_rows(context, symbols, params)
    rows.extend(enrich_candidate_rows(base_rows, context, btc_context, market_context, funding_context, feature_cache, params))
    report_candidate_progress(
        progress_callback,
        0.45,
        "base_layer_generation",
        f"ML selector: generated {len(rows)} latest base candidate rows",
    )
    report_candidate_progress(
        progress_callback,
        0.50,
        "universe_candidate_generation",
        "ML selector: generating latest universe candidate layers",
    )
    for symbol in symbols:
        candles = context_candles(context, symbol, "15m")
        if not candles:
            profiles.append({"symbol": symbol, "status": "missing_15m_candles"})
            continue
        symbol_rows, profile = generate_universe_candidate_rows_for_symbol(
            symbol=symbol,
            candles=candles,
            modes=modes,
            holds=holds,
            params=params,
            btc_candles=btc_context,
            market_context=market_context,
            funding_context=funding_context,
            feature_cache=feature_cache,
        )
        rows.extend(symbol_rows)
        profiles.append(profile)
    report_candidate_progress(
        progress_callback,
        0.95,
        "universe_candidate_generation",
        f"ML selector: generated {len(rows)} latest total candidate rows",
    )
    profiles.extend(base_generation.get("profiles", []))
    implemented_layers = sorted(
        set(["universe_candidate_v1", *base_generation.get("implemented_layers", [])])
    )
    return rows, {
        "generated_count": len(rows),
        "status": "runtime_candidate_layers_generated" if rows else "no_runtime_candidate_signal",
        "profiles": profiles,
        "implemented_layers": implemented_layers,
        "missing_layers": [],
        "base_generation": base_generation,
    }


def generate_runtime_candidate_rows_for_timestamps(context, symbols, params, timestamps, progress_callback=None):
    """Generate candidate rows for a batch of point-in-time entry timestamps."""

    if not isinstance(context, dict):
        return [], {"generated_count": 0, "status": "invalid_context"}

    timestamps = sorted({int(timestamp) for timestamp in timestamps if int(timestamp or 0) > 0})
    if not timestamps:
        return [], {"generated_count": 0, "status": "no_timestamps"}

    modes = symbolic_list(params.get("universe_candidate_modes", "breakout,fade"))
    holds = int_list(params.get("universe_candidate_holds", "32,72"))
    rows = []
    profiles = []
    feature_cache = runtime_feature_cache(context)
    report_candidate_progress(
        progress_callback,
        0.05,
        "candidate_context",
        f"ML selector: building candidate context for {len(timestamps)} timestamps",
    )
    btc_context = cached_btc_context(context, timestamps, feature_cache)
    market_context = cached_market_context(context, symbols, params, timestamps, feature_cache)
    funding_context = cached_funding_context(context, symbols, feature_cache)
    report_candidate_progress(
        progress_callback,
        0.15,
        "base_layer_generation",
        "ML selector: generating base candidate layers",
    )
    if should_parallelize_base_enriched_generation(params, symbols, timestamps):
        base_rows, base_profiles, base_parallel = generate_base_candidate_rows_parallel_enriched_at_timestamps(
            context=context,
            symbols=symbols,
            timestamps=timestamps,
            params=params,
            btc_candles=btc_context,
            market_context=market_context,
            funding_context=funding_context,
        )
        _, base_generation = base_generation_result(base_rows, base_profiles)
        base_generation["parallel_base_generation"] = base_parallel
        base_generation["parallel_base_enrichment"] = base_parallel
        rows.extend(base_rows)
    else:
        base_rows, base_generation = generate_base_candidate_rows_for_timestamps(
            context,
            symbols,
            params,
            timestamps,
        )
        rows.extend(enrich_candidate_rows(base_rows, context, btc_context, market_context, funding_context, feature_cache, params))
    report_candidate_progress(
        progress_callback,
        0.45,
        "base_layer_generation",
        f"ML selector: generated {len(rows)} base candidate rows",
    )
    parallel_generation = None
    report_candidate_progress(
        progress_callback,
        0.50,
        "universe_candidate_generation",
        "ML selector: generating universe candidate layers",
    )
    if should_parallelize_universe_generation(params, symbols, timestamps):
        symbol_rows, symbol_profiles, parallel_generation = generate_universe_candidate_rows_parallel_at_timestamps(
            context=context,
            symbols=symbols,
            timestamps=timestamps,
            modes=modes,
            holds=holds,
            params=params,
            btc_candles=btc_context,
            market_context=market_context,
            funding_context=funding_context,
        )
        rows.extend(symbol_rows)
        profiles.extend(symbol_profiles)
    else:
        parallel_generation = {"enabled": False, "reason": "below_parallel_threshold_or_unsupported"}
        for symbol in symbols:
            candles = context_candles(context, symbol, "15m")
            if not candles:
                profiles.append({"symbol": symbol, "status": "missing_15m_candles"})
                continue
            symbol_rows, profile = generate_universe_candidate_rows_for_symbol_at_timestamps(
                symbol=symbol,
                candles=candles,
                timestamps=timestamps,
                modes=modes,
                holds=holds,
                params=params,
                btc_candles=btc_context,
                market_context=market_context,
                funding_context=funding_context,
                feature_cache=feature_cache,
            )
            rows.extend(symbol_rows)
            profiles.append(profile)
    report_candidate_progress(
        progress_callback,
        0.95,
        "universe_candidate_generation",
        f"ML selector: generated {len(rows)} total candidate rows",
    )
    profiles.extend(base_generation.get("profiles", []))
    implemented_layers = sorted(
        set(["universe_candidate_v1", *base_generation.get("implemented_layers", [])])
    )
    return rows, {
        "generated_count": len(rows),
        "status": "runtime_candidate_layers_generated" if rows else "no_runtime_candidate_signal",
        "profiles": profiles,
        "implemented_layers": implemented_layers,
        "missing_layers": [],
        "base_generation": base_generation,
        "parallel_universe_generation": parallel_generation,
    }


def report_candidate_progress(callback, progress, stage, message):
    if callable(callback):
        callback(stage, message, progress)


def should_parallelize_base_enriched_generation(params, symbols, timestamps):
    if not bool_param(params, "parallel_base_layer_enrichment", True):
        return False
    if os.name != "posix":
        return False
    if len(symbols) < 2:
        return False
    min_timestamps = int_param(params, "parallel_base_layer_min_timestamps", 256)
    if len(timestamps) < min_timestamps:
        return False
    try:
        multiprocessing.get_context("fork")
    except (RuntimeError, ValueError):
        return False
    return parallel_base_enrich_worker_count(params, len(base_layer_specs(symbols, params))) > 1


def parallel_base_enrich_worker_count(params, spec_count):
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


def generate_base_candidate_rows_parallel_enriched_at_timestamps(
    *,
    context,
    symbols,
    timestamps,
    params,
    btc_candles=None,
    market_context=None,
    funding_context=None,
):
    specs = base_layer_specs(symbols, params)
    workers = parallel_base_enrich_worker_count(params, len(specs))
    state = (
        context,
        list(specs),
        list(timestamps),
        dict(params),
        btc_candles,
        market_context,
        funding_context,
    )
    rows = []
    profiles = []
    process_context = multiprocessing.get_context("fork")
    with ProcessPoolExecutor(
        max_workers=workers,
        mp_context=process_context,
        initializer=init_parallel_base_enrich_worker,
        initargs=(state,),
    ) as executor:
        futures = [executor.submit(parallel_base_enrich_worker, index) for index in range(len(specs))]
        for future in futures:
            layer_rows, profile = future.result()
            rows.extend(layer_rows)
            profiles.append(profile)
    return rows, profiles, {
        "enabled": True,
        "workers": workers,
        "spec_count": len(specs),
        "enriched_in_worker": True,
    }


def init_parallel_base_enrich_worker(state):
    global _PARALLEL_BASE_ENRICH_STATE
    _PARALLEL_BASE_ENRICH_STATE = state


def parallel_base_enrich_worker(index):
    state = _PARALLEL_BASE_ENRICH_STATE
    if state is None:
        raise RuntimeError("parallel base enrichment worker was not initialized")
    context, specs, timestamps, params, btc_candles, market_context, funding_context = state
    layer_rows, profile = generate_layer_for_symbol_at_timestamps(
        context=context,
        timestamps=timestamps,
        **specs[int(index)],
    )
    enriched_rows = enrich_candidate_rows(
        layer_rows,
        context,
        btc_candles,
        market_context,
        funding_context,
        {},
        params,
    )
    return enriched_rows, profile


def should_parallelize_universe_generation(params, symbols, timestamps):
    if not bool_param(params, "parallel_universe_candidate_generation", True):
        return False
    if os.name != "posix":
        return False
    if len(symbols) < 2:
        return False
    min_timestamps = int_param(params, "parallel_universe_min_timestamps", 256)
    if len(timestamps) < min_timestamps:
        return False
    try:
        multiprocessing.get_context("fork")
    except (RuntimeError, ValueError):
        return False
    return parallel_universe_worker_count(params, len(symbols)) > 1


def parallel_universe_worker_count(params, symbol_count):
    default_workers = min(8, max(1, int(os.cpu_count() or 1)), max(1, int(symbol_count or 1)))
    try:
        configured = int(params.get("parallel_universe_max_workers", default_workers))
    except (TypeError, ValueError):
        configured = default_workers
    configured = max(1, min(int(configured), max(1, int(symbol_count or 1)), max(1, int(os.cpu_count() or 1))))
    return memory_limited_worker_count(
        params,
        configured,
        symbol_count,
        worker_memory_key="parallel_universe_worker_memory_gb",
        default_worker_memory_gb=1.5,
    )


def generate_universe_candidate_rows_parallel_at_timestamps(
    *,
    context,
    symbols,
    timestamps,
    modes,
    holds,
    params,
    btc_candles=None,
    market_context=None,
    funding_context=None,
):
    workers = parallel_universe_worker_count(params, len(symbols))
    candles_by_symbol = {str(symbol): context_candles(context, str(symbol), "15m") for symbol in symbols}
    state = (
        candles_by_symbol,
        list(timestamps),
        list(modes),
        list(holds),
        dict(params),
        btc_candles,
        market_context,
        funding_context,
    )
    rows = []
    profiles = []
    context = multiprocessing.get_context("fork")
    with ProcessPoolExecutor(
        max_workers=workers,
        mp_context=context,
        initializer=init_parallel_universe_worker,
        initargs=(state,),
    ) as executor:
        futures = [executor.submit(parallel_universe_symbol_worker, str(symbol)) for symbol in symbols]
        for future in futures:
            symbol_rows, profile = future.result()
            rows.extend(symbol_rows)
            profiles.append(profile)
    return rows, profiles, {"enabled": True, "workers": workers, "symbol_count": len(symbols)}


def init_parallel_universe_worker(state):
    global _PARALLEL_UNIVERSE_STATE
    _PARALLEL_UNIVERSE_STATE = state


def parallel_universe_symbol_worker(symbol):
    state = _PARALLEL_UNIVERSE_STATE
    if state is None:
        raise RuntimeError("parallel universe candidate worker was not initialized")
    candles_by_symbol, timestamps, modes, holds, params, btc_candles, market_context, funding_context = state
    candles = candles_by_symbol.get(str(symbol)) or []
    return generate_universe_candidate_rows_for_symbol_at_timestamps(
        symbol=str(symbol),
        candles=candles,
        timestamps=timestamps,
        modes=modes,
        holds=holds,
        params=params,
        btc_candles=btc_candles,
        market_context=market_context,
        funding_context=funding_context,
        feature_cache={},
    )


def generate_universe_candidate_rows_for_symbol(
    *,
    symbol,
    candles,
    modes,
    holds,
    params,
    btc_candles=None,
    market_context=None,
    funding_context=None,
    feature_cache=None,
):
    if len(candles) < 3:
        return [], {"symbol": symbol, "status": "not_enough_rows", "rows": len(candles)}

    local_params = dict(DEFAULT_UNIVERSE_CANDIDATE_PARAMS)
    for key in local_params:
        if key in params:
            local_params[key] = params[key]

    signal_index = len(candles) - 2
    entry_index = len(candles) - 1
    indicator_candles, indicator_offset, indicators = universe_candidate_indicators_for_signal_indices(
        symbol,
        candles,
        [signal_index],
        local_params,
        feature_cache,
    )
    indicator_signal_index = signal_index - indicator_offset
    out = []
    for mode in modes:
        mode_params = dict(local_params)
        mode_params["candidate_mode"] = mode
        signal = generate_universe_candidate_signal(indicator_signal_index, indicator_candles, indicators, mode_params, symbol)
        if not is_entry_signal(signal):
            continue
        for hold_bars in holds:
            row = candidate_row_from_signal(
                symbol=symbol,
                mode=mode,
                hold_bars=hold_bars,
                candles=candles,
                signal_index=signal_index,
                entry_index=entry_index,
                signal=signal,
                params=local_params,
            )
            row = enrich_candidate_row(
                row,
                candles,
                btc_candles,
                market_context,
                funding_context,
                feature_cache,
                params,
            )
            out.append(row)
    return out, {
        "symbol": symbol,
        "status": "generated" if out else "no_signal",
        "rows": len(candles),
        "candidate_rows": len(out),
        "signal_index": signal_index,
        "entry_index": entry_index,
    }


def generate_universe_candidate_rows_for_symbol_at_timestamps(
    *,
    symbol,
    candles,
    timestamps,
    modes,
    holds,
    params,
    btc_candles=None,
    market_context=None,
    funding_context=None,
    feature_cache=None,
):
    if len(candles) < 3:
        return [], {"symbol": symbol, "status": "not_enough_rows", "rows": len(candles)}

    local_params = dict(DEFAULT_UNIVERSE_CANDIDATE_PARAMS)
    for key in local_params:
        if key in params:
            local_params[key] = params[key]
    entry_indices = entry_indices_at_timestamps(candles, timestamps)
    unique_entry_indices = []
    seen_entry_indices = set()
    for entry_index in entry_indices:
        if entry_index is None or entry_index < 1 or entry_index in seen_entry_indices:
            continue
        seen_entry_indices.add(entry_index)
        unique_entry_indices.append(entry_index)
    if not unique_entry_indices:
        return [], {
            "symbol": symbol,
            "status": "no_signal",
            "rows": len(candles),
            "candidate_rows": 0,
            "evaluated_timestamps": len(timestamps),
        }
    indicator_candles, indicator_offset, indicators = universe_candidate_indicators_for_signal_indices(
        symbol,
        candles,
        [entry_index - 1 for entry_index in unique_entry_indices],
        local_params,
        feature_cache,
    )

    out = []
    for mode in modes:
        mode_params = dict(local_params)
        mode_params["candidate_mode"] = mode
        signal_cache = {}

        def signal_for_entry(entry_index):
            signal_index = int(entry_index) - 1
            indicator_signal_index = signal_index - indicator_offset
            if indicator_signal_index < 0 or indicator_signal_index >= len(indicator_candles):
                return None
            if entry_index not in signal_cache:
                signal_cache[entry_index] = generate_universe_candidate_signal(
                    indicator_signal_index,
                    indicator_candles,
                    indicators,
                    mode_params,
                    symbol,
                )
            return signal_cache[entry_index]

        for hold_bars in holds:
            next_free = 0
            for entry_index in unique_entry_indices:
                signal_index = entry_index - 1
                if signal_index < next_free:
                    continue
                signal = signal_for_entry(entry_index)
                if not is_entry_signal(signal):
                    continue
                exit_index = int(entry_index) + int(hold_bars)
                if exit_index >= len(candles):
                    continue
                row = candidate_row_from_signal(
                    symbol=symbol,
                    mode=mode,
                    hold_bars=hold_bars,
                    candles=candles,
                    signal_index=signal_index,
                    entry_index=entry_index,
                    signal=signal,
                    params=local_params,
                    exit_index=exit_index,
                    exit_reason="max_hold_bars",
                )
                row = enrich_candidate_row(
                    row,
                    candles,
                    btc_candles,
                    market_context,
                    funding_context,
                    feature_cache,
                    params,
                )
                out.append(row)
                next_free = exit_index + 1
    return out, {
        "symbol": symbol,
        "status": "generated" if out else "no_signal",
        "rows": len(candles),
        "candidate_rows": len(out),
        "evaluated_timestamps": len(timestamps),
    }


def enrich_candidate_rows(rows, context, btc_candles, market_context, funding_context, feature_cache=None, params=None):
    out = []
    params = params if isinstance(params, dict) else {}
    for row in rows:
        if not isinstance(row, dict):
            continue
        candles = context_candles(context, str(row.get("asset") or row.get("symbol")), str(row.get("timeframe") or "15m"))
        enriched = enrich_candidate_row(
            row,
            candles,
            btc_candles,
            market_context,
            funding_context,
            feature_cache,
            params,
        )
        out.append(enriched)
    return out


def enrich_candidate_row(row, candles, btc_candles, market_context, funding_context, feature_cache, params):
    enriched = row
    if bool_param(params, "require_funding_context", False) or bool_param(params, "strict_context_gating", False):
        enriched = add_funding_context(enriched, funding_context, cache_section(feature_cache, "funding"))
        if (
            bool_param(params, "strict_context_gating", False)
            and str(enriched.get("funding_data_status", "")).strip().lower() != "available"
        ):
            enriched["_model_context_blocker"] = "funding_context_incomplete"

    enriched = add_local_and_calendar_features(enriched, candles, cache_section(feature_cache, "local"))
    enriched = add_btc_context(enriched, btc_candles, cache_section(feature_cache, "btc"))
    enriched = add_cross_sectional_context(enriched, market_context, cache_section(feature_cache, "market"))
    return add_funding_context(enriched, funding_context, cache_section(feature_cache, "funding"))


def cache_section(feature_cache, key):
    if not isinstance(feature_cache, dict):
        return None
    return feature_cache.setdefault(key, {})


def candidate_row_from_signal(
    *,
    symbol,
    mode,
    hold_bars,
    candles,
    signal_index,
    entry_index,
    signal,
    params,
    exit_index=None,
    exit_reason=None,
):
    entry_time = int(candles[entry_index].get("timestamp", 0) or 0)
    signal_time = int(candles[signal_index].get("timestamp", 0) or 0)
    side = normalized_side(signal)
    layer_id = f"ucand_{mode}:{asset_key(symbol)}:15m:h{int(hold_bars)}"
    row = {
        "symbol": symbol,
        "asset": symbol,
        "layer_id": layer_id,
        "family": f"universe_{mode}",
        "timeframe": "15m",
        "side": side,
        "signal_index": signal_index,
        "entry_index": entry_index,
        "signal_time": signal_time,
        "entry_time": entry_time,
        "feature_bar_time": signal_time,
        "timestamp": entry_time,
        "hold_bars": int(hold_bars),
        "planned_hold_bars": int(hold_bars),
        "strength": round_float(float(signal.get("strength", 0.0) or 0.0)),
        "reason": str(signal.get("reason", "")),
        "entry_price": finite_float(candles[entry_index].get("open")),
        "leverage": round_float(params.get("leverage", 3.0)),
        "round_trip_cost_bps": round_float(params.get("round_trip_cost_bps", 16.0)),
        "candidate_source": "runtime_universe_candidate_v1",
    }
    if exit_index is not None:
        row["exit_index"] = int(exit_index)
        row["exit_time"] = int(candles[int(exit_index)].get("timestamp", 0) or 0)
        row["exit_reason"] = str(exit_reason or "max_hold_bars")
    return row


def universe_indicator_padding(params):
    lookback = int_param(params, "rank_lookback", 5000)
    source_window = max(
        int_param(params, "ret_fast_bars", 16),
        int_param(params, "ret_slow_bars", 96),
        int_param(params, "range_bars", 96),
        int_param(params, "vol_bars", 96) + 1,
        int_param(params, "volume_bars", 96),
    )
    return max(0, lookback + source_window + 2)


def indicator_slice_for_signal_indices(candles, signal_indices, padding):
    valid_indices = [int(index) for index in signal_indices if index is not None and int(index) >= 0]
    if not valid_indices:
        return candles, 0
    start = max(0, min(valid_indices) - max(0, int(padding or 0)))
    return candles[start:], start


def compute_universe_candidate_indicators(candles, params):
    closes = [finite_float(item.get("close")) or 0.0 for item in candles]
    highs = [finite_float(item.get("high")) or 0.0 for item in candles]
    lows = [finite_float(item.get("low")) or 0.0 for item in candles]
    volumes = [finite_float(item.get("volume_ccy", item.get("volume"))) or 0.0 for item in candles]

    ret_fast = lagged_return(closes, int_param(params, "ret_fast_bars", 16))
    ret_slow = lagged_return(closes, int_param(params, "ret_slow_bars", 96))
    range_values = rolling_range(highs, lows, int_param(params, "range_bars", 96))
    true_range_values = true_range(highs, lows, closes)
    vol_values = rolling_std(true_range_values, int_param(params, "vol_bars", 96))
    volume_avg = rolling_mean(volumes, int_param(params, "volume_bars", 96))
    volume_ratio = [
        volumes[index] / volume_avg[index]
        if finite(volume_avg[index]) and volume_avg[index] > 0.0
        else float("nan")
        for index in range(len(volumes))
    ]

    lookback = int_param(params, "rank_lookback", 5000)
    min_samples = max(10, int_param(params, "min_rank_samples", 1000))
    ranks = rolling_mid_rank_current_many(
        {
            "ret_fast": ret_fast,
            "ret_slow": ret_slow,
            "range": range_values,
            "vol": vol_values,
            "volume": volume_ratio,
        },
        lookback,
        min_samples,
    )
    return {
        "ret_fast": ret_fast,
        "ret_slow": ret_slow,
        "range": range_values,
        "vol": vol_values,
        "volume_ratio": volume_ratio,
        "ret_fast_rank": ranks["ret_fast"],
        "ret_slow_rank": ranks["ret_slow"],
        "range_rank": ranks["range"],
        "vol_rank": ranks["vol"],
        "volume_rank": ranks["volume"],
        "warmup": min_samples,
    }


def generate_universe_candidate_signal(index, candles, indicators, params, symbol):
    if index < int(indicators.get("warmup", 1000)) or index >= len(candles):
        return None

    fast_rank = indicators["ret_fast_rank"][index]
    slow_rank = indicators["ret_slow_rank"][index]
    volume_rank = indicators["volume_rank"][index]
    range_rank = indicators["range_rank"][index]
    vol_rank = indicators["vol_rank"][index]
    if not all(finite(item) for item in [fast_rank, slow_rank, volume_rank, range_rank, vol_rank]):
        return None

    if volume_rank < num_param(params, "volume_min_rank", 0.50):
        return None
    if range_rank > num_param(params, "range_max_rank", 0.98):
        return None
    if vol_rank > num_param(params, "vol_max_rank", 0.98):
        return None

    fast_hi = num_param(params, "fast_hi_rank", 0.88)
    fast_lo = num_param(params, "fast_lo_rank", 0.12)
    slow_hi = num_param(params, "slow_hi_rank", 0.60)
    slow_lo = num_param(params, "slow_lo_rank", 0.40)
    mode = str(params.get("candidate_mode", "breakout")).strip().lower()
    close = finite_float(candles[index].get("close")) or 0.0

    high_momentum = fast_rank >= fast_hi and slow_rank >= slow_hi
    low_momentum = fast_rank <= fast_lo and slow_rank <= slow_lo
    if mode == "breakout":
        if high_momentum:
            strength = min(1.0, (fast_rank - fast_hi) + (slow_rank - slow_hi))
            return {"side": "buy", "price": close, "reason": f"{symbol} breakout long", "strength": strength}
        if low_momentum:
            strength = min(1.0, (fast_lo - fast_rank) + (slow_lo - slow_rank))
            return {"side": "sell", "price": close, "reason": f"{symbol} breakdown short", "strength": strength}
    elif mode == "fade":
        if high_momentum:
            strength = min(1.0, (fast_rank - fast_hi) + (slow_rank - slow_hi))
            return {"side": "sell", "price": close, "reason": f"{symbol} overextension fade short", "strength": strength}
        if low_momentum:
            strength = min(1.0, (fast_lo - fast_rank) + (slow_lo - slow_rank))
            return {"side": "buy", "price": close, "reason": f"{symbol} washout fade long", "strength": strength}
    return None


def int_list(value):
    if isinstance(value, (list, tuple)):
        raw = value
    else:
        raw = str(value or "").split(",")
    out = []
    for item in raw:
        try:
            parsed = int(item)
        except (TypeError, ValueError):
            continue
        if parsed > 0:
            out.append(parsed)
    return out or [32, 72]


def lagged_return(closes, lag):
    lag = max(1, int(lag))
    out = [float("nan")] * len(closes)
    for index in range(lag, len(closes)):
        base = closes[index - lag]
        close = closes[index]
        if base > 0.0 and close > 0.0:
            out[index] = close / base - 1.0
    return out


def true_range(highs, lows, closes):
    out = [float("nan")] * len(closes)
    for index in range(1, len(closes)):
        prev = closes[index - 1]
        if prev <= 0.0:
            continue
        value = max(highs[index] - lows[index], abs(highs[index] - prev), abs(lows[index] - prev))
        out[index] = max(0.0, value) / prev
    return out


def rolling_mean(values, window):
    window = max(1, int(window))
    out = [float("nan")] * len(values)
    total = 0.0
    count = 0
    queue = deque()
    for index, value in enumerate(values):
        queue.append(value)
        if finite(value):
            total += float(value)
            count += 1
        if len(queue) > window:
            old = queue.popleft()
            if finite(old):
                total -= float(old)
                count -= 1
        if count >= max(3, int(window * 0.5)):
            out[index] = total / count
    return out


def rolling_range(highs, lows, window):
    window = max(1, int(window))
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
    window = max(1, int(window))
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


def rolling_rank(values, lookback, min_samples):
    return rolling_mid_rank_current(values, lookback, min_samples)


def normalized_side(signal):
    side = str(signal.get("side", "") if isinstance(signal, dict) else "").lower()
    if side in {"sell", "short"}:
        return "short"
    if side in {"buy", "long"}:
        return "long"
    return ""


def is_entry_signal(signal):
    return normalized_side(signal) in {"short", "long"}


def bool_param(params, key, default):
    value = params.get(key, default) if isinstance(params, dict) else default
    if isinstance(value, bool):
        return value
    return str(value).strip().lower() not in {"0", "false", "no", "off", "none"}
