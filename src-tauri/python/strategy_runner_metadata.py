"""Strategy module loading, discovery, and runtime parameter handling."""

import importlib.util as _importlib_util
from pathlib import Path as _Path

from strategy_runner_values import _numeric


def _load_module(file_path: str):
    """Load a Python strategy module from a .py file path."""
    path = _Path(file_path).resolve()
    if not path.exists():
        raise FileNotFoundError(f"策略文件不存在: {file_path}")
    spec = _importlib_util.spec_from_file_location(path.stem, str(path))
    if spec is None or spec.loader is None:
        raise ImportError(f"无法加载策略模块: {file_path}")
    module = _importlib_util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _discover(args: dict) -> dict:
    """Discover strategy metadata and runtime contracts."""
    file_path = args["file_path"]
    module = _load_module(file_path)

    strategy_id = getattr(module, "STRATEGY_ID", None)
    if not strategy_id:
        return {"ok": False, "error": f"策略文件中未找到 STRATEGY_ID: {file_path}"}

    strategy_name = getattr(module, "STRATEGY_NAME", None)
    if not strategy_name:
        return {"ok": False, "error": f"策略文件中未找到 STRATEGY_NAME: {file_path}"}

    description = getattr(module, "STRATEGY_DESCRIPTION", "")
    strategy_type = getattr(module, "STRATEGY_TYPE", "single_symbol_strategy")
    data_requirements = getattr(module, "DATA_REQUIREMENTS", {})
    runtime_config = getattr(module, "RUNTIME_CONFIG", None)
    visualization = getattr(module, "VISUALIZATION", None)
    decision_contract = getattr(module, "DECISION_CONTRACT", None)

    metadata_error = _validate_runtime_metadata(
        file_path,
        runtime_config,
        visualization,
        decision_contract,
        strategy_type,
        data_requirements,
    )
    if metadata_error:
        return {"ok": False, "error": metadata_error}

    return {
        "ok": True,
        "strategy_id": str(strategy_id),
        "strategy_name": str(strategy_name),
        "description": str(description),
        "strategy_type": str(strategy_type),
        "data_requirements": data_requirements if isinstance(data_requirements, dict) else {},
        "runtime_config": runtime_config,
        "visualization": visualization,
        "decision_contract": decision_contract,
    }


def _validate_runtime_metadata(
    file_path: str,
    runtime_config,
    visualization,
    decision_contract,
    strategy_type,
    data_requirements,
) -> str:
    if not isinstance(runtime_config, dict):
        return f"RUNTIME_CONFIG 必须定义为 dict: {file_path}"
    required_runtime_keys = [
        "symbol",
        "inst_type",
        "timeframe",
        "risk_timeframe",
        "initial_capital",
        "position_size",
        "stop_loss",
        "take_profit",
        "check_interval",
        "params",
    ]
    missing_runtime_keys = [
        key for key in required_runtime_keys if key not in runtime_config
    ]
    if missing_runtime_keys:
        return (
            f"RUNTIME_CONFIG 缺少必填字段 {', '.join(missing_runtime_keys)}: "
            f"{file_path}"
        )
    if not isinstance(runtime_config.get("params"), dict):
        return f"RUNTIME_CONFIG.params 必须是 dict: {file_path}"
    for key in ["symbol", "inst_type", "timeframe", "risk_timeframe"]:
        value = runtime_config.get(key)
        if not isinstance(value, str) or not value.strip():
            return f"RUNTIME_CONFIG.{key} 必须是非空字符串: {file_path}"
    for key in [
        "initial_capital",
        "position_size",
        "stop_loss",
        "take_profit",
        "check_interval",
    ]:
        if _numeric(runtime_config.get(key)) is None:
            return f"RUNTIME_CONFIG.{key} 必须是有效数字: {file_path}"
    if not isinstance(visualization, dict) or not visualization:
        return f"VISUALIZATION 必须定义为非空 dict: {file_path}"
    if not isinstance(decision_contract, dict) or not decision_contract:
        return f"DECISION_CONTRACT 必须定义为非空 dict: {file_path}"
    if not isinstance(strategy_type, str) or not strategy_type.strip():
        return f"STRATEGY_TYPE 必须定义为非空字符串: {file_path}"
    if not isinstance(data_requirements, dict) or not data_requirements:
        return f"DATA_REQUIREMENTS 必须定义为非空 dict: {file_path}"
    return ""


def _default_params(module) -> dict:
    """Build strategy params from frozen runtime defaults."""
    params = {}
    runtime_config = getattr(module, "RUNTIME_CONFIG", {})
    if isinstance(runtime_config, dict) and isinstance(runtime_config.get("params"), dict):
        params.update(runtime_config["params"])
    return params


def _inject_runtime_context(params: dict, config: dict) -> None:
    """Inject non-overridable runtime context for strategy execution."""
    params["_runtime_symbol"] = config.get("symbol")
    params["_runtime_inst_type"] = config.get("inst_type")
    params["_runtime_timeframe"] = config.get("timeframe")
    params["_runtime_strategy_id"] = config.get("strategy_id")


def _runtime_params(module, config: dict) -> dict:
    params = _default_params(module)
    raw_params = config.get("params", {})
    if isinstance(raw_params, dict):
        params.update(raw_params)
    _inject_runtime_context(params, config)
    return params
