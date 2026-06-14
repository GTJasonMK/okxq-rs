"""Self-contained runtime candidate strategy.

This file is the executable strategy contract reference for the app runtime:
it carries tuned/default run settings, visualization metadata, and stable
decision reason codes in the same module as the signal logic.
"""

from __future__ import annotations

import bisect
import math
import sys
from pathlib import Path


_STRATEGIES_ROOT = Path(__file__).resolve().parents[1]
if str(_STRATEGIES_ROOT) not in sys.path:
    sys.path.insert(0, str(_STRATEGIES_ROOT))

from lib.ml_trade_runtime_progress import backtest_progress  # noqa: E402
from lib.runtime_logging import emit_execution_log, execution_log  # noqa: E402


STRATEGY_ID = "runtime_candidate_breakout_v1"
STRATEGY_NAME = "Runtime Candidate Breakout V1"
STRATEGY_DESCRIPTION = (
    "Self-contained candidate strategy using point-in-time MA spread, momentum, "
    "and volume ranks. Runtime defaults are frozen in RUNTIME_CONFIG."
)
STRATEGY_TYPE = "single_symbol_strategy"

RUNTIME_CONFIG = {
    "symbol": "BTC-USDT-SWAP",
    "inst_type": "SWAP",
    "timeframe": "15m",
    "risk_timeframe": "1m",
    "initial_capital": 1000,
    "position_size": 0.15,
    "stop_loss": 0.03,
    "take_profit": 0.06,
    "check_interval": 60,
    "mode": "simulated",
    "params": {
        "contract_mode": True,
        "leverage": 3,
        "fast_window": 48,
        "slow_window": 192,
        "momentum_window": 32,
        "volume_window": 96,
        "rank_lookback": 1200,
        "min_rank_samples": 240,
        "spread_threshold": 0.006,
        "momentum_threshold": 0.012,
        "volume_rank_threshold": 0.55,
        "position_size": 0.15,
    },
}

DATA_REQUIREMENTS = {
    "candles": [
        {
            "role": "primary",
            "symbol": "BTC-USDT-SWAP",
            "inst_type": "SWAP",
            "timeframe": "15m",
            "min_bars": 1200,
        },
        {
            "role": "risk_context",
            "symbol": "BTC-USDT-SWAP",
            "inst_type": "SWAP",
            "timeframe": "1m",
            "min_bars": 240,
        },
    ],
    "funding": [],
    "orderbook": [],
    "positions": {"required": False},
    "account": {"required": False},
    "orders": {"open": False, "recent_fills": False, "recent_rejections": False},
}

VISUALIZATION = {
    "primary_price_series": "close",
    "indicator_series": [
        {
            "key": "ma_spread",
            "label": "MA spread",
            "unit": "ratio",
            "threshold_key": "spread_threshold",
        },
        {
            "key": "momentum_return",
            "label": "Momentum return",
            "unit": "ratio",
            "threshold_key": "momentum_threshold",
        },
        {
            "key": "volume_rank",
            "label": "Volume rank",
            "unit": "rank",
            "threshold_key": "volume_rank_threshold",
        },
    ],
    "diagnostics": ["warmup", "ma_spread", "momentum_return", "volume_rank"],
}

DECISION_CONTRACT = {
    "action_schema_version": 1,
    "actions": ["open_position", "close_position", "hold"],
    "entry_sides": ["buy", "sell"],
    "exit_sides": ["flat"],
    "hold_sides": ["hold"],
    "reason_codes": ["candidate_breakout_long", "candidate_breakout_short"],
}


def _context_runtime(context):
    runtime = context.get("runtime") if isinstance(context, dict) else {}
    return runtime if isinstance(runtime, dict) else {}


def _context_candles(context, symbol, timeframe):
    if not isinstance(context, dict):
        return []
    candles_by_symbol = context.get("candles")
    if not isinstance(candles_by_symbol, dict):
        return []
    timeframe_map = candles_by_symbol.get(symbol)
    if not isinstance(timeframe_map, dict):
        return []
    candles = timeframe_map.get(timeframe)
    return candles if isinstance(candles, list) else []


def _action_from_decision(decision, symbol, inst_type, timeframe, timestamp):
    if not isinstance(decision, dict):
        return None
    side = str(decision.get("side") or "hold").lower()
    if side == "buy":
        action_side = "long"
    elif side == "sell":
        action_side = "short"
    elif side in ("flat", "close", "exit"):
        action_side = "flat"
    else:
        return None
    return {
        "action": "close_position" if action_side == "flat" else "open_position",
        "symbol": symbol,
        "inst_type": inst_type,
        "timeframe": timeframe,
        "side": action_side,
        "order_type": "market",
        "price": decision.get("price"),
        "position_size": decision.get("position_size"),
        "reason": decision.get("reason", ""),
        "strength": decision.get("strength", 0.5),
        "timestamp": timestamp,
    }


def evaluate(context, params):
    runtime = _context_runtime(context)
    symbol = str(runtime.get("symbol") or params.get("_runtime_symbol") or RUNTIME_CONFIG["symbol"])
    inst_type = str(runtime.get("inst_type") or params.get("_runtime_inst_type") or RUNTIME_CONFIG["inst_type"])
    timeframe = str(runtime.get("timeframe") or params.get("_runtime_timeframe") or RUNTIME_CONFIG["timeframe"])
    candles = _context_candles(context, symbol, timeframe)
    logs = [
        execution_log(
            "strategy_input",
            f"{STRATEGY_NAME}: received {len(candles)} candles for {symbol} {timeframe}",
            "info",
            {"symbol": symbol, "timeframe": timeframe, "candle_count": len(candles)},
        )
    ]
    emit_execution_log(context, "strategy_input", logs[-1]["message"], "info", logs[-1]["details"])

    if not candles:
        progress = backtest_progress(
            context,
            "candidate_signal",
            "No primary candles available.",
            label=STRATEGY_NAME,
        )
        logs.append(
            execution_log(
                "strategy_decision",
                f"{STRATEGY_NAME}: no primary candles, hold",
                "warn",
                {"symbol": symbol, "timeframe": timeframe},
            )
        )
        diagnostics = {
            "strategy_id": STRATEGY_ID,
            "strategy_name": STRATEGY_NAME,
            "symbol": symbol,
            "timeframe": timeframe,
            "action_intent": "hold",
            "action_count": 0,
            "side": "hold",
            "summary": "Runtime Candidate Breakout V1 has no primary candles.",
            "conditions": [],
            "blocked_by": ["empty_candles"],
        }
        if progress is not None:
            diagnostics["backtest_progress"] = progress
        return {
            "actions": [],
            "diagnostics": diagnostics,
            "indicators": {},
            "execution_logs": logs,
        }

    indicators = compute_indicators(candles, params)
    index = len(candles) - 1
    decision = _candidate_decision(index, candles, indicators, params)
    diagnostics = _decision_diagnostics(index, candles, indicators, params, decision)
    progress = backtest_progress(
        context,
        "candidate_signal",
        "Evaluating breakout signal.",
        label=STRATEGY_NAME,
    )
    if progress is not None:
        diagnostics["backtest_progress"] = progress
    timestamp = int(candles[index].get("timestamp", 0) or 0)
    action = _action_from_decision(decision, symbol, inst_type, timeframe, timestamp)
    logs.append(
        execution_log(
            "strategy_decision",
            (
                f"{STRATEGY_NAME}: generated {action['action']} {action['side']}"
                if action
                else f"{STRATEGY_NAME}: no signal, hold"
            ),
            "success" if action else "info",
            {
                "symbol": symbol,
                "timeframe": timeframe,
                "timestamp": timestamp,
                "action": action.get("action") if action else "hold",
                "side": action.get("side") if action else "hold",
                "reason": action.get("reason") if action else diagnostics.get("summary"),
            },
        )
    )
    emit_execution_log(context, "strategy_decision", logs[-1]["message"], logs[-1]["level"], logs[-1]["details"])
    return {
        "actions": [action] if action else [],
        "diagnostics": diagnostics,
        "indicators": indicators,
        "execution_logs": logs,
    }


def _num(params, key, default):
    value = params.get(key, default)
    try:
        parsed = float(value)
        return parsed if math.isfinite(parsed) else float(default)
    except (TypeError, ValueError):
        return float(default)


def _int(params, key, default):
    return max(1, int(_num(params, key, default)))


def _finite(value):
    return isinstance(value, (int, float)) and math.isfinite(float(value))


def _rolling_mean(values, window):
    out = [None] * len(values)
    total = 0.0
    queue = []
    for index, value in enumerate(values):
        queue.append(float(value))
        total += float(value)
        if len(queue) > window:
            total -= queue.pop(0)
        if len(queue) == window:
            out[index] = total / window
    return out


def _lagged_return(closes, lag):
    out = [None] * len(closes)
    for index in range(lag, len(closes)):
        base = closes[index - lag]
        close = closes[index]
        if base > 0.0 and close > 0.0:
            out[index] = close / base - 1.0
    return out


def _rolling_rank(values, lookback, min_samples):
    out = []
    queue = []
    sorted_values = []
    for value in values:
        queue.append(value)
        if _finite(value):
            bisect.insort(sorted_values, float(value))
        if len(queue) > lookback:
            old = queue.pop(0)
            if _finite(old):
                pos = bisect.bisect_left(sorted_values, float(old))
                if pos < len(sorted_values) and sorted_values[pos] == float(old):
                    sorted_values.pop(pos)
        if not _finite(value) or len(sorted_values) < min_samples:
            out.append(None)
            continue
        left = bisect.bisect_left(sorted_values, float(value))
        right = bisect.bisect_right(sorted_values, float(value))
        midpoint = (left + right - 1) / 2.0
        out.append(max(0.0, min(1.0, midpoint / max(1, len(sorted_values) - 1))))
    return out


def _constant_series(length, value):
    return [float(value)] * length


def compute_indicators(candles, params):
    closes = [float(item.get("close", 0.0) or 0.0) for item in candles]
    volumes = [float(item.get("volume_ccy", item.get("volume", 0.0)) or 0.0) for item in candles]
    fast_window = _int(params, "fast_window", 48)
    slow_window = _int(params, "slow_window", 192)
    momentum_window = _int(params, "momentum_window", 32)
    volume_window = _int(params, "volume_window", 96)
    rank_lookback = _int(params, "rank_lookback", 1200)
    min_rank_samples = _int(params, "min_rank_samples", 240)

    fast_ma = _rolling_mean(closes, fast_window)
    slow_ma = _rolling_mean(closes, slow_window)
    momentum_return = _lagged_return(closes, momentum_window)
    avg_volume = _rolling_mean(volumes, volume_window)
    volume_ratio = [
        volumes[index] / avg_volume[index]
        if _finite(avg_volume[index]) and avg_volume[index] > 0.0
        else None
        for index in range(len(candles))
    ]
    ma_spread = [
        fast_ma[index] / slow_ma[index] - 1.0
        if _finite(fast_ma[index]) and _finite(slow_ma[index]) and slow_ma[index] > 0.0
        else None
        for index in range(len(candles))
    ]

    return {
        "ma_spread": ma_spread,
        "momentum_return": momentum_return,
        "volume_rank": _rolling_rank(volume_ratio, rank_lookback, min_rank_samples),
        "spread_threshold": _constant_series(len(candles), _num(params, "spread_threshold", 0.006)),
        "momentum_threshold": _constant_series(len(candles), _num(params, "momentum_threshold", 0.012)),
        "volume_rank_threshold": _constant_series(len(candles), _num(params, "volume_rank_threshold", 0.55)),
        "warmup": max(slow_window, momentum_window, volume_window, min_rank_samples),
    }


def _candidate_decision(index, candles, indicators, params):
    if index < int(indicators.get("warmup", 240)) or index >= len(candles):
        return None
    spread = indicators["ma_spread"][index]
    momentum = indicators["momentum_return"][index]
    volume_rank = indicators["volume_rank"][index]
    if not all(_finite(item) for item in [spread, momentum, volume_rank]):
        return None

    spread_threshold = _num(params, "spread_threshold", 0.006)
    momentum_threshold = _num(params, "momentum_threshold", 0.012)
    volume_threshold = _num(params, "volume_rank_threshold", 0.55)
    if volume_rank < volume_threshold:
        return None

    close = float(candles[index].get("close", 0.0) or 0.0)
    strength = min(
        1.0,
        max(
            abs(spread) / max(spread_threshold, 1e-12),
            abs(momentum) / max(momentum_threshold, 1e-12),
        )
        / 2.0,
    )
    if spread >= spread_threshold and momentum >= momentum_threshold:
        return {
            "side": "buy",
            "price": close,
            "reason": "candidate_breakout_long",
            "strength": strength,
            "position_size": _num(params, "position_size", 0.15),
        }
    if spread <= -spread_threshold and momentum <= -momentum_threshold:
        return {
            "side": "sell",
            "price": close,
            "reason": "candidate_breakout_short",
            "strength": strength,
            "position_size": _num(params, "position_size", 0.15),
        }
    return None


def _series_at(indicators, key, index):
    values = indicators.get(key)
    if not isinstance(values, list) or index < 0 or index >= len(values):
        return None
    value = values[index]
    return float(value) if _finite(value) else None


def _progress_ge(current, target):
    if current is None or target is None:
        return 0.0
    if target <= 0.0:
        return 1.0 if current >= target else 0.0
    return max(0.0, min(1.0, current / max(abs(target), 1e-12)))


def _progress_le(current, target):
    if current is None or target is None:
        return 0.0
    if current <= target:
        return 1.0
    if target < 0.0:
        return max(0.0, min(1.0, -current / max(abs(target), 1e-12)))
    return max(0.0, min(1.0, target / max(abs(current), 1e-12)))


def _condition(key, label, operator, current, target, unit, progress):
    missing = current is None or target is None
    if missing:
        return {
            "key": key,
            "label": label,
            "operator": operator,
            "current": current,
            "target": target,
            "gap": None,
            "unit": unit,
            "passed": False,
            "progress": 0.0,
            "missing": True,
        }
    if operator == "<=":
        passed = current <= target
        gap = max(0.0, current - target)
    else:
        passed = current >= target
        gap = max(0.0, target - current)
    return {
        "key": key,
        "label": label,
        "operator": operator,
        "current": round(current, 10),
        "target": round(target, 10),
        "gap": round(gap, 10),
        "unit": unit,
        "passed": passed,
        "progress": round(max(0.0, min(1.0, progress)), 10),
    }


def _warmup_condition(index, warmup):
    passed = index >= warmup
    return {
        "key": "warmup",
        "label": "Warmup",
        "operator": ">=",
        "current": index,
        "target": warmup,
        "gap": max(0, warmup - index),
        "unit": "bars",
        "passed": passed,
        "progress": 1.0 if warmup <= 0 else round(max(0.0, min(1.0, index / warmup)), 10),
    }


def _summary(side, blocked_by, conditions):
    if side == "buy":
        return "Runtime Candidate Breakout V1 long entry triggered."
    if side == "sell":
        return "Runtime Candidate Breakout V1 short entry triggered."
    if "warmup" in blocked_by:
        return "Runtime Candidate Breakout V1 warmup is not complete."
    gaps = []
    for condition in conditions:
        if condition.get("passed") or condition.get("key") == "warmup":
            continue
        gap = condition.get("gap")
        unit = condition.get("unit")
        label = condition.get("label") or condition.get("key")
        if isinstance(gap, (int, float)) and unit == "ratio":
            gaps.append(f"{label} gap {gap * 100:.3f}%")
        elif isinstance(gap, (int, float)):
            gaps.append(f"{label} gap {gap:.4f}")
        else:
            gaps.append(f"{label} not ready")
    return "Runtime Candidate Breakout V1 not triggered" + (": " + ", ".join(gaps) if gaps else ".")


def _decision_diagnostics(index, candles, indicators, params, decision):
    candle = candles[index] if 0 <= index < len(candles) else {}
    side = str((decision or {}).get("side") or "hold").lower()
    spread = _series_at(indicators, "ma_spread", index)
    momentum = _series_at(indicators, "momentum_return", index)
    volume_rank = _series_at(indicators, "volume_rank", index)
    spread_threshold = _num(params, "spread_threshold", 0.006)
    momentum_threshold = _num(params, "momentum_threshold", 0.012)
    volume_threshold = _num(params, "volume_rank_threshold", 0.55)
    warmup = int(indicators.get("warmup", 240) or 240)

    long_progress = min(
        _progress_ge(spread, spread_threshold),
        _progress_ge(momentum, momentum_threshold),
        _progress_ge(volume_rank, volume_threshold),
    )
    short_progress = min(
        _progress_le(spread, -spread_threshold),
        _progress_le(momentum, -momentum_threshold),
        _progress_ge(volume_rank, volume_threshold),
    )
    direction = "short" if side == "sell" or (side == "hold" and short_progress > long_progress) else "long"

    if direction == "short":
        factor_conditions = [
            _condition("ma_spread", "MA spread", "<=", spread, -spread_threshold, "ratio", _progress_le(spread, -spread_threshold)),
            _condition("momentum_return", "Momentum return", "<=", momentum, -momentum_threshold, "ratio", _progress_le(momentum, -momentum_threshold)),
            _condition("volume_rank", "Volume rank", ">=", volume_rank, volume_threshold, "rank", _progress_ge(volume_rank, volume_threshold)),
        ]
    else:
        factor_conditions = [
            _condition("ma_spread", "MA spread", ">=", spread, spread_threshold, "ratio", _progress_ge(spread, spread_threshold)),
            _condition("momentum_return", "Momentum return", ">=", momentum, momentum_threshold, "ratio", _progress_ge(momentum, momentum_threshold)),
            _condition("volume_rank", "Volume rank", ">=", volume_rank, volume_threshold, "rank", _progress_ge(volume_rank, volume_threshold)),
        ]

    conditions = [_warmup_condition(index, warmup), *factor_conditions]
    blocked_by = [item["key"] for item in conditions if not item.get("passed")]
    action_intent = "open_position" if side in ("buy", "sell") else "hold"
    factor_progress = min((item.get("progress", 0.0) for item in factor_conditions), default=0.0)
    progress = min((item.get("progress", 0.0) for item in conditions), default=0.0)
    return {
        "strategy_id": STRATEGY_ID,
        "strategy_name": STRATEGY_NAME,
        "symbol": str(params.get("_runtime_symbol") or ""),
        "timeframe": str(params.get("_runtime_timeframe") or ""),
        "action_intent": action_intent,
        "action_count": 1 if action_intent == "open_position" else 0,
        "side": side,
        "decision": decision,
        "progress": progress,
        "factor_progress": factor_progress,
        "summary": _summary(side, blocked_by, conditions),
        "conditions": conditions,
        "blocked_by": blocked_by,
    }
