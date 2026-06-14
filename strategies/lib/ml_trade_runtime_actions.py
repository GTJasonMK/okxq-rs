"""Order, risk action, and lifecycle contract helpers for the ML trade selector runtime."""

from __future__ import annotations

import bisect
from decimal import Decimal, ROUND_CEILING, ROUND_FLOOR, InvalidOperation

from lib.ml_trade_candidate_ranking import selection_weight_for_rank
from lib.ml_trade_model_scoring import FROZEN_CANDIDATE, num

def order_side(side):
    return "buy" if side == "long" else "sell"

def risk_close_side(side):
    return "sell" if side == "long" else "buy"

def stop_price(entry_price, side, stop_bps):
    if entry_price <= 0.0 or stop_bps <= 0.0:
        return entry_price
    ratio = stop_bps / 10000.0
    if side == "long":
        return entry_price * (1.0 - ratio)
    return entry_price * (1.0 + ratio)

def protective_stop_price(entry_price, side, stop_bps, symbol=None, params=None):
    price = stop_price(entry_price, side, stop_bps)
    tick_size = tick_size_for_symbol(symbol, params)
    if tick_size is None:
        return price
    return align_price_to_tick(
        price,
        tick_size,
        "up" if side == "long" else "down",
    )

def tick_size_for_symbol(symbol, params):
    if not symbol or not isinstance(params, dict):
        return None
    key = str(symbol).strip().upper()
    by_symbol = params.get("_backtest_instrument_rules_by_symbol")
    rules = by_symbol.get(key) if isinstance(by_symbol, dict) else None
    if not isinstance(rules, dict):
        inst_id = str(params.get("instId") or params.get("inst_id") or "").strip().upper()
        rules = params if inst_id == key else None
    if not isinstance(rules, dict):
        return None
    tick = num(rules.get("tickSz", rules.get("tick_size", rules.get("tick_sz"))), None)
    return tick if tick is not None and tick > 0.0 else None

def align_price_to_tick(price, tick_size, direction):
    try:
        value = Decimal(str(price))
        tick = Decimal(str(tick_size))
    except (InvalidOperation, ValueError):
        return price
    if value <= 0 or tick <= 0:
        return price
    rounding = ROUND_CEILING if direction == "up" else ROUND_FLOOR
    steps = (value / tick).to_integral_value(rounding=rounding)
    aligned = steps * tick
    return float(aligned)

def protective_stop_reason(stop_bps):
    value = num(stop_bps, FROZEN_CANDIDATE["path_stop_loss_bps"])
    if value is None:
        value = FROZEN_CANDIDATE["path_stop_loss_bps"]
    return f"protective_stop_{int(round(float(value)))}bps"

def price_for_symbol(context, symbol, fallback_price):
    orderbook = context.get("orderbook") if isinstance(context, dict) else {}
    book = orderbook.get(symbol) if isinstance(orderbook, dict) else {}
    if isinstance(book, dict):
        mid = num(book.get("mid_price") or book.get("mid"), None)
        if mid is not None and mid > 0.0:
            return mid
    candles = context.get("candles") if isinstance(context, dict) else {}
    by_timeframe = candles.get(symbol) if isinstance(candles, dict) else {}
    rows = by_timeframe.get("15m") if isinstance(by_timeframe, dict) else []
    if isinstance(rows, list) and rows:
        close = num(rows[-1].get("close"), None)
        if close is not None and close > 0.0:
            return close
    return fallback_price

def build_price_series_cache(context):
    candles = context.get("candles") if isinstance(context, dict) else {}
    if not isinstance(candles, dict):
        return {}
    cache = {}
    for symbol, by_timeframe in candles.items():
        if not isinstance(by_timeframe, dict):
            continue
        rows = by_timeframe.get("15m")
        if not isinstance(rows, list) or not rows:
            continue
        timestamps = []
        closes = []
        for row in rows:
            if not isinstance(row, dict):
                continue
            timestamps.append(int(row.get("timestamp", 0) or 0))
            closes.append(num(row.get("close"), None))
        if timestamps:
            cache[str(symbol)] = {"timestamps": timestamps, "closes": closes}
    return cache

def price_for_symbol_at(context, symbol, timestamp, fallback_price, price_cache=None):
    series = price_cache.get(symbol) if isinstance(price_cache, dict) else None
    if series is not None:
        timestamps = series["timestamps"]
        closes = series["closes"]
    else:
        candles = context.get("candles") if isinstance(context, dict) else {}
        by_timeframe = candles.get(symbol) if isinstance(candles, dict) else {}
        rows = by_timeframe.get("15m") if isinstance(by_timeframe, dict) else []
        if not isinstance(rows, list) or not rows:
            return fallback_price
        timestamps = [int(row.get("timestamp", 0) or 0) for row in rows]
        closes = [num(row.get("close"), None) for row in rows]
    if not timestamps:
        return fallback_price
    index = bisect.bisect_right(timestamps, int(timestamp or 0)) - 1
    if index < 0:
        return fallback_price
    close = closes[index]
    return close if close is not None and close > 0.0 else fallback_price

def entry_action(order):
    item = order.copy()
    item.setdefault("action", "open_position")
    if str(item.get("order_type") or "").strip().lower() == "market":
        reference_price = item.get("reference_price")
        if reference_price is None:
            reference_price = item.get("price")
        if reference_price is not None:
            item["reference_price"] = reference_price
        item.setdefault("price_source", "strategy_reference")
        item.pop("price", None)
    return item

def risk_order_action(order):
    item = order.copy()
    item.setdefault("action", "place_risk_order")
    if item.get("trigger_price") is None and item.get("price") is not None:
        item["trigger_price"] = item.get("price")
    item.pop("price", None)
    return item

def order_from_ranked_item(
    item,
    rank,
    context,
    timestamp,
    fallback_price,
    params,
    stop_bps,
    max_slippage_bps,
    price_cache=None,
):
    symbol = item["symbol"]
    side = item["side"]
    entry_price = (
        price_for_symbol_at(context, symbol, timestamp, fallback_price, price_cache)
        if price_cache is not None
        else price_for_symbol(context, symbol, fallback_price)
    )
    position_size = num(item.get("selection_weight"), None)
    if position_size is None:
        position_size = selection_weight_for_rank(rank, params)
    reason = "ml_ranked_candidate"
    order = {
        "symbol": symbol,
        "side": side,
        "order_side": order_side(side),
        "order_type": "market",
        "price": entry_price,
        "reference_price": entry_price,
        "price_source": "strategy_reference",
        "position_size": position_size,
        "strength": max(0.0, min(1.0, item["adjusted_score"] / 500.0)),
        "reason": f"{reason}:rank={rank}:score={item['adjusted_score']:.4f}",
        "timestamp": timestamp,
        "stop_loss_bps": stop_bps,
        "max_slippage_bps": max_slippage_bps,
    }
    order.update(planned_exit_action_fields(item, context))
    if num(order.get("planned_exit_time"), 0) > 0:
        order["planned_exit_contract"] = "planned_exit_time_v1"
    return order

def action_lifecycle_contract(actions):
    open_actions = [action for action in actions if runtime_open_action(action)]
    open_count = len(open_actions)
    planned_count = sum(1 for action in open_actions if planned_exit_timestamp(action) > 0)
    required_keys = (
        "planned_exit_time",
        "entry_time",
        "planned_hold_bars",
        "hold_bars",
        "layer_id",
        "candidate_source",
    )
    key_counts = {
        key: sum(1 for action in open_actions if action_key_present(action, key))
        for key in required_keys
    }
    status = "no_open_actions"
    if open_count > 0 and planned_count == open_count:
        status = "planned_exit_complete"
    elif open_count > 0 and planned_count > 0:
        status = "planned_exit_partial"
    elif open_count > 0:
        status = "planned_exit_missing"
    missing_samples = []
    for action in open_actions:
        if planned_exit_timestamp(action) > 0:
            continue
        missing_samples.append(
            {
                "symbol": str(action.get("symbol") or ""),
                "timestamp": int(num(action.get("timestamp"), 0) or 0),
                "reason": str(action.get("reason") or ""),
                "row_lifecycle_keys": {
                    key: action_key_present(action, key) for key in required_keys
                },
            }
        )
        if len(missing_samples) >= 3:
            break
    return {
        "version": "ml_trade_selector_planned_exit_v1",
        "status": status,
        "open_action_count": open_count,
        "open_actions_with_planned_exit": planned_count,
        "open_actions_missing_planned_exit": max(0, open_count - planned_count),
        "planned_exit_coverage_pct": round(planned_count / open_count * 100.0, 6) if open_count else 0.0,
        "required_key_counts": key_counts,
        "missing_planned_exit_samples": missing_samples,
    }

def runtime_open_action(action):
    if not isinstance(action, dict):
        return False
    name = str(action.get("action") or "").strip().lower()
    return name == "open_position"

def planned_exit_timestamp(action):
    if not isinstance(action, dict):
        return 0
    return int(num(action.get("planned_exit_time"), 0) or 0)

def action_key_present(action, key):
    if not isinstance(action, dict):
        return False
    value = action.get(key)
    if value is None:
        return False
    if isinstance(value, str):
        return bool(value.strip())
    return True

def planned_exit_action_fields(item, context=None):
    row = item.get("row") if isinstance(item, dict) else None
    if not isinstance(row, dict):
        return {}

    fields = {}
    exit_time = planned_exit_time_from_row(row, context, item)
    if exit_time > 0:
        fields["planned_exit_time"] = exit_time

    exit_reason = str(row.get("exit_reason") or "").strip()
    if not exit_reason and exit_time > 0:
        exit_reason = "max_hold_bars"
    if exit_reason:
        fields["planned_exit_reason"] = exit_reason

    for key in ("exit_index", "planned_hold_bars", "hold_bars", "entry_index"):
        value = num(row.get(key), None)
        if value is not None:
            fields[key] = int(value)

    source_index = num(row.get("source_index"), None)
    if source_index is None:
        source_index = num(row.get("signal_index"), None)
    if source_index is not None:
        fields["source_index"] = int(source_index)

    for key in ("entry_time", "feature_bar_time"):
        value = num(row.get(key), None)
        if value is not None:
            fields[key] = int(value)

    source_time = num(row.get("source_time"), None)
    if source_time is None:
        source_time = num(row.get("signal_time"), None)
    if source_time is not None:
        fields["source_time"] = int(source_time)

    for key in ("layer_id", "family", "timeframe", "candidate_source"):
        value = row.get(key)
        if value is not None and str(value).strip():
            fields[key] = str(value)

    entry_price = num(row.get("entry_price"), None)
    if entry_price is not None and entry_price > 0.0:
        fields["candidate_entry_price"] = entry_price

    return fields

def planned_exit_time_from_row(row, context, item=None):
    explicit = int(num(row.get("exit_time") or row.get("planned_exit_time"), 0) or 0)
    if explicit > 0:
        return explicit

    entry_index = int_or_default(row.get("entry_index"), -1)
    exit_index = int_or_default(row.get("exit_index"), -1)
    hold_bars = planned_hold_bars_from_row(row)
    if exit_index < 0 and entry_index >= 0 and hold_bars > 0:
        exit_index = entry_index + hold_bars

    candles = candidate_context_candles(context, row, item)
    if exit_index >= 0 and exit_index < len(candles):
        timestamp = int(num(candles[exit_index].get("timestamp"), 0) or 0)
        if timestamp > 0:
            return timestamp

    entry_time = int(num(row.get("entry_time") or row.get("timestamp"), 0) or 0)
    interval_ms = timeframe_ms(str(row.get("timeframe") or "15m"))
    if entry_time > 0 and hold_bars > 0 and interval_ms > 0:
        return entry_time + hold_bars * interval_ms
    return 0

def planned_hold_bars_from_row(row):
    for key in ("planned_hold_bars", "hold_bars"):
        value = num(row.get(key), None)
        if value is not None and value > 0:
            return int(value)
    return 0

def int_or_default(value, default):
    parsed = num(value, None)
    return int(parsed) if parsed is not None else int(default)

def candidate_context_candles(context, row, item=None):
    if not isinstance(context, dict):
        return []
    symbol = str(
        row.get("symbol")
        or row.get("asset")
        or (item or {}).get("symbol")
        or ""
    ).strip()
    timeframe = str(row.get("timeframe") or "15m").strip()
    candles = context.get("candles")
    by_timeframe = candles.get(symbol) if isinstance(candles, dict) else None
    rows = by_timeframe.get(timeframe) if isinstance(by_timeframe, dict) else None
    return rows if isinstance(rows, list) else []

def timeframe_ms(timeframe):
    value = str(timeframe or "").strip().lower()
    if value.endswith("m"):
        minutes = num(value[:-1], None)
        return int(minutes * 60_000) if minutes is not None and minutes > 0 else 0
    if value.endswith("h"):
        hours = num(value[:-1], None)
        return int(hours * 3_600_000) if hours is not None and hours > 0 else 0
    if value.endswith("d"):
        days = num(value[:-1], None)
        return int(days * 86_400_000) if days is not None and days > 0 else 0
    return 0

def hold_decision(strategy_id, blocked_by, summary, extra):
    diagnostics = {
        "strategy_id": strategy_id,
        "action_intent": "hold",
        "action_count": 0,
        "open_action_count": 0,
        "risk_action_count": 0,
        "side": "hold",
        "summary": summary,
        "blocked_by": blocked_by,
        "frozen_candidate": FROZEN_CANDIDATE,
    }
    diagnostics.update(extra)
    return {"actions": [], "diagnostics": diagnostics, "indicators": {}, "execution_logs": []}

def empty_indicators(length):
    return {
        "candidate_score": [None] * length,
        "adjusted_score": [None] * length,
        "rank_weight": [0.0] * length,
    }
