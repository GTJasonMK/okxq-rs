"""稳健策略 V1 — 资金费率均值回归 + 动量确认 + 波动率过滤。

经济直觉：
  1. 极端资金费率后价格倾向于回归（空头/多头平仓压力）
  2. 中期动量确认趋势方向
  3. 高波动率时降低仓位以控制风险

本策略文件遵循策略运行器协议（strategies/README.md），
可与 Rust 后端的 strategy_executor 模块通过 JSON-lines 协议通信。
"""

from __future__ import annotations

import math
from typing import Any

# ── 策略元数据 ───────────────────────────────────────────────────────────

STRATEGY_ID = "robust_strategy_v1"
STRATEGY_NAME = "稳健策略 V1 (均值回归反转)"
STRATEGY_DESCRIPTION = (
    "基于 BTC 1H 级别均值回归特征的双反转策略："
    "资金费率极端值回归 + 价格动量反转（IC=-0.45），"
    "高波动率时自动降低仓位。参数通过 Walk-Forward 验证固化。"
)
STRATEGY_TYPE = "single_symbol_strategy"

RUNTIME_CONFIG = {
    "symbol": "BTC-USDT-SWAP",
    "inst_type": "SWAP",
    "timeframe": "1H",
    "risk_timeframe": "1H",
    "initial_capital": 10000,
    "position_size": 0.20,
    "stop_loss": 0.05,
    "take_profit": 0.12,
    "check_interval": 3600,
    "mode": "simulated",
    "params": {
        # 信号阈值
        "entry_threshold": 0.35,
        "exit_threshold": 0.10,
        # 风控
        "stop_loss_pct": 0.05,
        "take_profit_pct": 0.12,
        # 仓位管理
        "base_position_pct": 0.20,
        "max_position_pct": 0.40,
        "vol_target_pct": 0.40,
        # 因子计算参数
        "fr_zscore_window": 168,
        "momentum_long_window": 24,
        "momentum_confirm_window": 8,
        "volatility_window": 24,
        # 复合信号权重
        "signal_fr_weight": 0.50,
        "signal_momentum_weight": 0.30,
        "signal_vol_penalty_weight": 0.20,
    },
}

DATA_REQUIREMENTS = {
    "candles": [
        {
            "role": "primary",
            "symbol": "BTC-USDT-SWAP",
            "inst_type": "SWAP",
            "timeframe": "1H",
            "min_bars": 168,  # 至少 7 天数据用于热身
        },
    ],
    "funding": {"required": True},
    "orderbook": [],
    "positions": {"required": False},
    "account": {"required": False},
    "orders": {"open": False, "recent_fills": False, "recent_rejections": False},
}

VISUALIZATION = {
    "primary_price_series": "close",
    "indicator_series": [
        {
            "key": "composite_signal",
            "label": "综合信号",
            "unit": "signal",
            "threshold_key": "entry_threshold",
        },
        {
            "key": "funding_rate_zscore",
            "label": "资金费率 Z-score",
            "unit": "z",
        },
        {
            "key": "momentum_24h",
            "label": "24H 动量",
            "unit": "log_return",
        },
        {
            "key": "volatility_24h",
            "label": "24H 波动率",
            "unit": "annual_pct",
        },
    ],
    "diagnostics": [
        "warmup", "fr_zscore", "momentum_24h",
        "volatility_filter", "composite_signal",
    ],
}

DECISION_CONTRACT = {
    "action_schema_version": 1,
    "actions": ["open_position", "close_position", "place_risk_order", "hold"],
    "entry_sides": ["long", "short"],
    "exit_sides": ["flat"],
    "hold_sides": ["hold"],
    "reason_codes": [
        "mean_reversion_long",
        "mean_reversion_short",
        "momentum_reversal_confirmed",
        "volatility_scaled",
        "signal_exit",
        "stop_loss_exit",
        "take_profit_exit",
        "insufficient_data",
        "no_signal",
    ],
}


# ── 辅助函数 ─────────────────────────────────────────────────────────────

def _context_candles(context: dict, symbol: str, timeframe: str) -> list[dict]:
    """从上下文中安全提取 K 线数据。"""
    try:
        return context.get("candles", {}).get(symbol, {}).get(timeframe, [])
    except (AttributeError, TypeError):
        return []


def _context_runtime(context: dict) -> dict:
    return context.get("runtime", {})


def _context_funding(context: dict, symbol: str) -> dict | None:
    """获取最近资金费率数据。"""
    try:
        funding_data = context.get("funding", {}).get(symbol, {})
        if not funding_data:
            return None
        latest = funding_data.get("latest", {})
        return latest if latest else None
    except (AttributeError, TypeError):
        return None


def _rolling_mean(values: list[float], window: int) -> list[float]:
    """滚动均值。"""
    result = []
    for i in range(len(values)):
        if i < window - 1:
            result.append(0.0)
        else:
            wv = values[i - window + 1:i + 1]
            result.append(sum(wv) / window)
    return result


def _rolling_std(values: list[float], window: int) -> list[float]:
    """滚动标准差。"""
    result = []
    for i in range(len(values)):
        if i < window - 1 or window < 2:
            result.append(0.0)
            continue
        wv = values[i - window + 1:i + 1]
        n = len(wv)
        mean = sum(wv) / n
        var = sum((v - mean) ** 2 for v in wv) / (n - 1)
        result.append(var ** 0.5)
    return result


# ── 核心评估函数 ─────────────────────────────────────────────────────────

def evaluate(context: dict, params: dict) -> dict:
    """评估当前市场状态并返回交易决策。

    Args:
        context: 运行时上下文（包含 candles, funding, positions 等）
        params: 运行时参数（合并了 RUNTIME_CONFIG.params 和用户覆盖）

    Returns:
        {"actions": [...], "diagnostics": {...}, "indicators": {...}, "execution_logs": [...]}
    """
    runtime = _context_runtime(context)
    symbol = runtime.get("symbol") or RUNTIME_CONFIG["symbol"]
    timeframe = runtime.get("timeframe") or RUNTIME_CONFIG["timeframe"]
    candles = _context_candles(context, symbol, timeframe)

    logs = []
    diagnostics: dict[str, Any] = {
        "strategy_id": STRATEGY_ID,
        "strategy_name": STRATEGY_NAME,
        "symbol": symbol,
        "timeframe": timeframe,
        "action_intent": "hold",
        "action_count": 0,
        "conditions": [],
    }

    # 空数据保护
    if not candles:
        logs.append({
            "stage": "strategy_input",
            "level": "warn",
            "message": "无可用 K 线数据，保持观望",
            "details": {"symbol": symbol, "timeframe": timeframe},
        })
        diagnostics["action_intent"] = "hold"
        diagnostics["summary"] = "无可用数据，保持观望"
        return {
            "actions": [],
            "diagnostics": diagnostics,
            "indicators": {},
            "execution_logs": logs,
        }

    # 提取参数
    entry_threshold = float(params.get("entry_threshold", 0.35))
    exit_threshold = float(params.get("exit_threshold", 0.10))
    stop_loss_pct = float(params.get("stop_loss_pct", 0.05))
    take_profit_pct = float(params.get("take_profit_pct", 0.12))
    base_position_pct = float(params.get("base_position_pct", 0.20))
    max_position_pct = float(params.get("max_position_pct", 0.40))
    vol_target_pct = float(params.get("vol_target_pct", 0.40))

    fr_zscore_window = int(params.get("fr_zscore_window", 168))
    momentum_long_window = int(params.get("momentum_long_window", 24))
    momentum_confirm_window = int(params.get("momentum_confirm_window", 8))
    volatility_window = int(params.get("volatility_window", 24))

    signal_fr_weight = float(params.get("signal_fr_weight", 0.50))
    signal_momentum_weight = float(params.get("signal_momentum_weight", 0.30))
    signal_vol_penalty_weight = float(params.get("signal_vol_penalty_weight", 0.20))

    n = len(candles)
    closes = [float(item.get("close", 0.0) or 0.0) for item in candles]
    volumes = [float(item.get("volume_ccy", 0.0) or float(item.get("volume", 0.0) or 0.0))
               for item in candles]

    # ── 资金费率 ──
    funding = _context_funding(context, symbol)
    # 从 funding 数据构建资金费率序列（简化：仅使用最新值）
    # 完整实现中应从 funding.history 提取时间序列
    funding_rate = float(funding.get("funding_rate", 0.0)) if funding else 0.0

    # ── 计算指标 ──
    # 24H 动量
    momentum_24h = 0.0
    if n > momentum_long_window and closes[-1] > 0 and closes[-1 - momentum_long_window] > 0:
        momentum_24h = math.log(closes[-1] / closes[-1 - momentum_long_window])

    # 8H 动量
    momentum_8h = 0.0
    if n > momentum_confirm_window and closes[-1] > 0 and closes[-1 - momentum_confirm_window] > 0:
        momentum_8h = math.log(closes[-1] / closes[-1 - momentum_confirm_window])

    # 24H 波动率
    log_rets = []
    for i in range(1, n):
        if closes[i - 1] > 0 and closes[i] > 0:
            log_rets.append(math.log(closes[i] / closes[i - 1]))
        else:
            log_rets.append(0.0)
    vol_window_rets = log_rets[-volatility_window:] if len(log_rets) >= volatility_window else log_rets
    if len(vol_window_rets) >= 2:
        mean_ret = sum(vol_window_rets) / len(vol_window_rets)
        vol_std = (sum((r - mean_ret) ** 2 for r in vol_window_rets) / (len(vol_window_rets) - 1)) ** 0.5
        volatility_24h = vol_std * math.sqrt(365 * 24)
    else:
        volatility_24h = 0.0

    # 资金费率 Z-score（简化：使用绝对水平代替）
    # 完整实现需要历史资金费率序列来计算滚动 Z-score
    fr_level = funding_rate * 100.0  # 转百分比
    # 以 0.01%（年化约 4.4%）为中心，0.1% 为 1 个标准差的经验估计
    fr_z_simple = (fr_level - 0.01) / 0.10 if abs(funding_rate) > 1e-12 else 0.0

    # ── 综合信号计算 ──
    # 两条反转流 + 波动率过滤：
    # 1. 资金费率均值回归：极端负费率 → 做多
    # 2. 动量反转（IC=-0.45）：近期大涨 → 做空（预期回落）
    # 3. 波动率过滤：高波动率 → 削弱信号
    fr_signal = -max(-1.0, min(1.0, fr_z_simple / 3.0))
    mom_signal = -max(-1.0, min(1.0, momentum_24h / 0.15))  # 反转！
    vol_penalty = max(0.0, min(1.0, (volatility_24h - 0.40) / 0.80)) if volatility_24h > 0 else 0.0

    composite_signal = (
        signal_fr_weight * fr_signal
        + signal_momentum_weight * mom_signal
        - signal_vol_penalty_weight * vol_penalty * abs(fr_signal + mom_signal) * 0.5
    )
    composite_signal = max(-1.0, min(1.0, composite_signal))

    # ── 指标输出 ──
    indicators = {
        "composite_signal": composite_signal,
        "funding_rate_zscore": fr_z_simple,
        "momentum_24h": momentum_24h,
        "momentum_8h": momentum_8h,
        "volatility_24h": volatility_24h,
        "warmup": 168,
    }

    # ── 热身检查 ──
    warmup = 168
    is_warm = n >= warmup

    # 诊断条件
    warmup_condition = {
        "key": "warmup",
        "label": "热身期",
        "operator": ">=",
        "current": n,
        "target": warmup,
        "gap": max(0, warmup - n),
        "unit": "bars",
        "passed": is_warm,
        "progress": min(1.0, n / warmup),
    }

    signal_condition = {
        "key": "composite_signal",
        "label": "综合信号强度",
        "operator": ">=" if abs(composite_signal) > entry_threshold else "<",
        "current": round(abs(composite_signal), 4),
        "target": entry_threshold,
        "gap": max(0.0, entry_threshold - abs(composite_signal)),
        "unit": "signal",
        "passed": abs(composite_signal) > entry_threshold,
        "progress": min(1.0, abs(composite_signal) / entry_threshold),
    }

    diagnostics["conditions"] = [warmup_condition, signal_condition]

    # ── 决策 ──
    actions = []
    timestamp = context.get("time", {}).get("timestamp", 0)
    current_price = closes[-1] if closes else 0.0

    if not is_warm:
        diagnostics["action_intent"] = "hold"
        diagnostics["summary"] = f"热身期未完成 ({n}/{warmup} bar)"
        logs.append({
            "stage": "strategy_decision",
            "level": "info",
            "message": f"热身期: {n}/{warmup}",
            "details": {"warmup_bars": n, "required": warmup},
        })
    elif abs(composite_signal) > entry_threshold:
        # 有信号
        if composite_signal > 0:
            side = "long"
            reason = "mean_reversion_long"
        else:
            side = "short"
            reason = "mean_reversion_short"

        # 波动率仓位缩放
        vol_scale = min(1.0, vol_target_pct / volatility_24h) if volatility_24h > 0 else 1.0
        position_size = base_position_pct * vol_scale
        position_size = min(position_size, max_position_pct)

        action = {
            "action": "open_position",
            "symbol": symbol,
            "side": side,
            "order_type": "market",
            "timestamp": timestamp,
            "reason": reason,
            "strength": abs(composite_signal),
            "position_size": position_size,
            "reference_price": current_price,
        }

        # 风控单
        risk_order = {
            "action": "place_risk_order",
            "symbol": symbol,
            "side": side,
            "order_type": "market",
            "timestamp": timestamp,
            "reason": f"protective_stop_{int(stop_loss_pct * 10000)}bps",
            "strength": 0.8,
            "stop_loss_pct": stop_loss_pct,
            "take_profit_pct": take_profit_pct,
            "target_order_kind": "algo",
            "target_order_type": "stop_loss_market",
        }

        actions = [action, risk_order]
        diagnostics["action_intent"] = "open_position"
        diagnostics["side"] = side
        diagnostics["action_count"] = 2

        logs.append({
            "stage": "strategy_decision",
            "level": "success",
            "message": f"入场信号: {side}, 强度={abs(composite_signal):.3f}, 仓位={position_size:.2%}",
            "details": {
                "side": side,
                "composite_signal": composite_signal,
                "fr_zscore": fr_z_simple,
                "momentum_24h": momentum_24h,
                "volatility_24h": volatility_24h,
                "position_size": position_size,
            },
        })
    else:
        # 无信号
        diagnostics["action_intent"] = "hold"
        diagnostics["summary"] = f"无信号 (|signal|={abs(composite_signal):.3f} < {entry_threshold})"

        logs.append({
            "stage": "strategy_decision",
            "level": "info",
            "message": f"无信号，保持观望 |signal|={abs(composite_signal):.3f}",
            "details": {"composite_signal": composite_signal},
        })

    return {
        "actions": actions,
        "diagnostics": diagnostics,
        "indicators": indicators,
        "execution_logs": logs,
    }
