"""
XAU Risk-Off Regime Filter

Statistical regime classifier that gates ETH/BTC trading entries.
Computed definition: risk_off = XAU+ / BTC- / ETH- on the prior completed day.

Derived from statistical analysis (see data/research/xau_regime_final_report.md).
Edge: -3.43% ETH daily return on risk-off days vs +0.70% on normal days.
NW t-stat = 11.25 (significant at 95% confidence).

Usage from strategy files:
    from regime_filter import is_risk_off, check_entry_gate

    # Before entering a trade:
    if not check_entry_gate(db_path, current_timestamp_ms):
        return None  # skip this entry
"""

import sqlite3
import json
from pathlib import Path

DAY_MS = 86_400_000

_CACHE = {}

def _get_daily_return_table(db_path, inst_id):
    """Lazy-load daily return lookup table. Cached per db_path."""
    cache_key = f"{db_path}:{inst_id}"
    if cache_key in _CACHE:
        return _CACHE[cache_key]

    try:
        db = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True, timeout=10)
        db.row_factory = sqlite3.Row
        rows = db.execute(
            "SELECT timestamp, close FROM candles WHERE inst_id=? AND timeframe='1H' ORDER BY timestamp",
            (inst_id,)
        ).fetchall()
        db.close()
    except Exception:
        return {}

    daily = {}
    last_close = None
    current_day = None
    for r in rows:
        day = r["timestamp"] // DAY_MS * DAY_MS
        if current_day is None:
            current_day = day
        if day != current_day:
            if last_close is not None:
                daily[current_day] = last_close
            current_day = day
        last_close = r["close"]
    if last_close is not None and current_day is not None:
        daily[current_day] = last_close

    _CACHE[cache_key] = daily
    return daily


def is_risk_off(db_path, timestamp_ms):
    """Check if the completed day before timestamp_ms was a risk-off day.

    Risk-off definition: XAU 24h return > 0, BTC 24h return < 0, ETH 24h return < 0.
    All three conditions must be true simultaneously.

    Args:
        db_path: Path to market.db
        timestamp_ms: Current decision timestamp (milliseconds)

    Returns:
        True if the prior completed day was risk-off (entry should be skipped)
    """
    current_day_ms = timestamp_ms // DAY_MS * DAY_MS
    prior_day_ms = current_day_ms - DAY_MS

    xau = _get_daily_return_table(db_path, "XAU-USDT-SWAP")
    btc = _get_daily_return_table(db_path, "BTC-USDT-SWAP")
    eth = _get_daily_return_table(db_path, "ETH-USDT-SWAP")

    days = sorted(set(xau.keys()) & set(btc.keys()) & set(eth.keys()))

    # Find the index of prior_day_ms in the day list
    try:
        idx = days.index(prior_day_ms)
    except ValueError:
        # Find closest completed day
        completed_days = [d for d in days if d <= prior_day_ms]
        if not completed_days:
            return False
        prior_day_ms = max(completed_days)
        idx = days.index(prior_day_ms)

    if idx == 0:
        return False  # No prior day to compare

    prev_day = days[idx - 1]

    xau_ret = xau[prior_day_ms] / xau[prev_day] - 1
    btc_ret = btc[prior_day_ms] / btc[prev_day] - 1
    eth_ret = eth[prior_day_ms] / eth[prev_day] - 1

    return xau_ret > 0 and btc_ret < 0 and eth_ret < 0


def check_entry_gate(db_path, timestamp_ms):
    """Gate check for strategy entry.

    Returns:
        (allowed: bool, reason: str)
        - If allowed is False, the strategy should skip this entry.
    """
    if is_risk_off(db_path, timestamp_ms):
        return False, "XAU risk-off regime active: skip ETH/BTC entry"

    return True, "regime clear"


def get_regime_stats(db_path):
    """Return summary statistics about the risk-off regime for reporting."""
    xau = _get_daily_return_table(db_path, "XAU-USDT-SWAP")
    btc = _get_daily_return_table(db_path, "BTC-USDT-SWAP")
    eth = _get_daily_return_table(db_path, "ETH-USDT-SWAP")

    days = sorted(set(xau.keys()) & set(btc.keys()) & set(eth.keys()))

    ro_count = 0
    total = 0
    ro_eth_rets = []

    for i in range(1, len(days)):
        d, p = days[i], days[i-1]
        xr = xau[d] / xau[p] - 1
        br = btc[d] / btc[p] - 1
        er = eth[d] / eth[p] - 1

        if xr > 0 and br < 0 and er < 0:
            ro_count += 1
            ro_eth_rets.append(er)
        total += 1

    ro_avg = sum(ro_eth_rets) / len(ro_eth_rets) * 100 if ro_eth_rets else 0

    return {
        "total_days": total,
        "risk_off_days": ro_count,
        "risk_off_pct": round(ro_count / max(total, 1) * 100, 1),
        "risk_off_eth_avg_pct": round(ro_avg, 2),
    }
