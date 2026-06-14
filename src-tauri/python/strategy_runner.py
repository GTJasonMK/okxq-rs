#!/usr/bin/env python3
"""运行策略执行器 — 由 Rust 后端通过子进程调用。

协议：每行一个 JSON 命令，结果以 JSON 行输出到 stdout。
所有日志/错误输出到 stderr（不与 stdout 数据混在一起）。

支持命令：
  {"action":"discover","file_path":"/path/to/strategy.py"}
    → {"ok":true,"strategy_id":"...","strategy_name":"...",
       "runtime_config":{...},"visualization":{...},"decision_contract":{...}}

  {"action":"compute","file_path":"/path/to/strategy.py","strategy_id":"...",
   "config":{...},"candles":[...]}
    → {"ok":true,"actions":[...],"execution_logs":[...],
       "decision":{...},"indicators":{...}}
    如果命令包含 "progress_events": true，stdout 可先输出若干
    {"event":"progress","progress":0.42,"stage":"strategy","message":"..."} 行。
    如果命令包含 "strategy_log_events": true，策略可通过 stdout 先输出若干
    {"event":"strategy_log","stage":"...","level":"info","message":"...","details":{...}} 行。

  {"action":"diagnose","file_path":"/path/to/strategy.py","strategy_id":"...",
   "config":{...},"candles":[...]}
    → {"ok":true,"actions":[...],"execution_logs":[...],
       "indicators":{...},"diagnostics":{...}}

策略文件契约：
  模块级变量: STRATEGY_ID (str), STRATEGY_NAME (str)
  可选: STRATEGY_DESCRIPTION (str)
  必需元数据:
    STRATEGY_TYPE: str
    DATA_REQUIREMENTS: dict
    RUNTIME_CONFIG: dict
      策略研究完成后固化的默认运行配置，包含 symbol/inst_type/timeframe/
      initial_capital/position_size/stop_loss/take_profit/check_interval/params。
    VISUALIZATION: dict
      UI 可视化偏好，例如 primary_price_series、indicator_series、diagnostics。
    DECISION_CONTRACT: dict
      说明策略输出的 actions/diagnostics/execution_logs 语义，供页面和执行层稳定解析。
  优先函数:
    evaluate(context: dict, params: dict) -> dict
      返回 StrategyDecision:
      {"actions": [...], "diagnostics": {...}, "indicators": {...}, "execution_logs": [...]}
      可选在 diagnostics.backtest_progress/task_progress/run_progress 返回
      {"progress": 0.0-1.0, "stage": "...", "message": "..."}，供回测进度条展示。
"""

import json as _json
import sys as _sys
import traceback as _traceback

from strategy_runner_compute import _compute
from strategy_runner_context_cache import cache_context
from strategy_runner_diagnostics import _diagnose
from strategy_runner_metadata import _discover


def main():
    """主循环：逐行读取 stdin 的 JSON 命令，输出 JSON 结果。"""
    for line in _sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            command = _json.loads(line)
        except _json.JSONDecodeError as exc:
            _sys.stdout.write(_json.dumps({"ok": False, "error": f"JSON 解析失败: {exc}"}) + "\n")
            _sys.stdout.flush()
            continue

        action = command.get("action", "")
        try:
            if action == "discover":
                result = _discover(command)
            elif action == "cache_context":
                result = cache_context(command)
            elif action == "compute":
                result = _compute(command)
            elif action == "diagnose":
                result = _diagnose(command)
            elif action == "ping":
                result = {"ok": True, "pong": True}
            else:
                result = {"ok": False, "error": f"未知命令: {action}"}
        except Exception:
            result = {
                "ok": False,
                "error": _traceback.format_exc(),
            }

        _sys.stdout.write(_json.dumps(result, default=str) + "\n")
        _sys.stdout.flush()


if __name__ == "__main__":
    main()
