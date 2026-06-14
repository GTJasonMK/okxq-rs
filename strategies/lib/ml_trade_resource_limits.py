"""Runtime resource limits for strategy-owned ML helpers."""

from __future__ import annotations

import math
import os
import resource
import sys


def numeric_param(params, key, default):
    if not isinstance(params, dict):
        return default
    try:
        value = float(params.get(key, default))
    except (TypeError, ValueError):
        return default
    return value if math.isfinite(value) else default


def int_param(params, key, default):
    return max(1, int(numeric_param(params, key, default)))


def current_rss_gb():
    proc_rss_gb = _current_rss_gb_from_proc()
    if proc_rss_gb > 0:
        return proc_rss_gb
    return _current_rss_gb_from_resource()


def _current_rss_gb_from_proc():
    try:
        with open("/proc/self/status", "r", encoding="utf-8") as handle:
            for line in handle:
                if line.startswith("VmRSS:"):
                    parts = line.split()
                    if len(parts) >= 2:
                        return float(parts[1]) / 1024.0 / 1024.0
    except OSError:
        return 0.0
    return 0.0


def _current_rss_gb_from_resource():
    try:
        max_rss = float(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss)
    except (OSError, ValueError):
        return 0.0
    if max_rss <= 0 or not math.isfinite(max_rss):
        return 0.0
    # Linux reports kilobytes; macOS/BSD report bytes. /proc is preferred on Linux
    # above, so this branch mainly provides a conservative non-Linux fallback.
    divisor = 1024.0 ** 3 if sys.platform == "darwin" else 1024.0 ** 2
    return max_rss / divisor


def memory_budget_gb(params):
    budget = numeric_param(params, "memory_budget_gb", 10.0)
    return budget if budget > 0 else 0.0


def memory_limited_worker_count(
    params,
    configured_workers,
    item_count,
    *,
    worker_memory_key,
    default_worker_memory_gb,
    reserve_key="memory_budget_reserve_gb",
    default_reserve_gb=1.5,
):
    cpu_count = max(1, int(os.cpu_count() or 1))
    configured = max(1, int(configured_workers or 1))
    max_workers = max(1, min(configured, max(1, int(item_count or 1)), cpu_count))
    budget = memory_budget_gb(params)
    if budget <= 0:
        return max_workers

    reserve_gb = max(0.0, numeric_param(params, reserve_key, default_reserve_gb))
    worker_gb = max(0.25, numeric_param(params, worker_memory_key, default_worker_memory_gb))
    available_gb = budget - current_rss_gb() - reserve_gb
    if available_gb <= worker_gb:
        return 1
    budget_workers = max(1, int(available_gb // worker_gb))
    return max(1, min(max_workers, budget_workers))


def model_score_chunk_rows(params, row_count):
    configured = int_param(params, "model_score_chunk_rows", 20_000)
    return max(1, min(configured, max(1, int(row_count or 1))))
