"""StrategyDecision action, diagnostics, and log normalization."""

from strategy_runner_values import _numeric


_PRICE_REQUIRED_ORDER_TYPES = {
    "limit",
    "post_only",
    "fok",
    "ioc",
    "mmp",
    "mmp_and_post_only",
}

_REMOVED_ACTION_FIELD_ALIASES = {
    "size": "position_size",
    "quantity": "position_size",
    "qty": "position_size",
    "sz": "position_size",
    "exchange_sz": "exchange_size",
    "okx_size": "exchange_size",
    "okx_sz": "exchange_size",
    "order_size": "exchange_size",
    "order_sz": "exchange_size",
    "reference_px": "reference_price",
    "valuation_price": "reference_price",
    "valuation_px": "reference_price",
    "mark_price": "reference_price",
    "mark_px": "reference_price",
    "_price_source": "price_source",
    "exit_time": "planned_exit_time",
    "exit_reason": "planned_exit_reason",
    "action_contract_version": "planned_exit_contract",
    "new_sz": "new_size",
    "newSz": "new_size",
    "target_size": "new_size",
    "amend_size": "new_size",
    "amend_sz": "new_size",
    "new_px": "new_price",
    "newPx": "new_price",
    "target_price": "new_price",
    "amend_price": "new_price",
    "amend_px": "new_price",
    "cxl_on_fail": "cancel_on_fail",
    "cxlOnFail": "cancel_on_fail",
    "cancelOnFail": "cancel_on_fail",
    "req_id": "request_id",
    "reqId": "request_id",
    "ord_id": "order_id",
    "ordId": "order_id",
    "target_order_id": "order_id",
    "target_ord_id": "order_id",
    "cl_ord_id": "client_order_id",
    "clOrdId": "client_order_id",
    "target_client_order_id": "client_order_id",
    "target_cl_ord_id": "client_order_id",
}

_ENGINE_CONTROLLED_ACTION_FIELDS = {
    "leverage": "leverage 由运行参数控制，不允许策略 action 覆盖",
    "lever": "leverage 由运行参数控制，不允许策略 action 覆盖",
    "td_mode": "td_mode 由运行参数控制，不允许策略 action 覆盖",
    "tdMode": "td_mode 由运行参数控制，不允许策略 action 覆盖",
    "mgn_mode": "td_mode 由运行参数控制，不允许策略 action 覆盖",
    "mgnMode": "td_mode 由运行参数控制，不允许策略 action 覆盖",
    "margin_mode": "td_mode 由运行参数控制，不允许策略 action 覆盖",
    "marginMode": "td_mode 由运行参数控制，不允许策略 action 覆盖",
    "pos_side": "posSide 由 OKX 持仓模式和执行层推导，不允许策略 action 覆盖",
    "posSide": "posSide 由 OKX 持仓模式和执行层推导，不允许策略 action 覆盖",
    "reduce_only": "reduceOnly 由 action 类型推导，不允许策略 action 覆盖",
    "reduceOnly": "reduceOnly 由 action 类型推导，不允许策略 action 覆盖",
    "tgt_ccy": "tgtCcy 由执行层按交易类型处理，不允许策略 action 覆盖",
    "tgtCcy": "tgtCcy 由执行层按交易类型处理，不允许策略 action 覆盖",
}

_SUPPORTED_ACTIONS = {
    "open_position",
    "close_position",
    "place_risk_order",
    "cancel_order",
    "modify_order",
    "hold",
}

_SUPPORTED_TARGET_ORDER_KINDS = {
    "any",
    "exchange",
    "algo",
}

_SUPPORTED_TARGET_ORDER_TYPES = {
    "stop_market",
    "stop_loss_market",
    "sl_market",
    "stop_loss",
    "stop",
    "take_profit_market",
    "tp_market",
    "take_profit",
}

_REMOVED_TOP_LEVEL_FIELDS = (
    "orders",
    "risk_orders",
    "signals",
    "portfolio_layers",
)

_REMOVED_EXECUTION_LOG_FIELD_ALIASES = {
    "summary": "message",
    "detail": "details",
    "phase": "stage",
    "severity": "level",
    "data": "details",
}

_ACTION_NUMERIC_FIELDS = (
    "stop_loss_bps",
    "stop_loss_pct",
    "stop_loss",
    "take_profit_bps",
    "take_profit_pct",
    "take_profit",
    "max_slippage_bps",
    "max_slippage_pct",
    "max_slippage",
    "trigger_price",
    "stop_price",
    "sl_trigger_px",
    "slTriggerPx",
    "take_profit_price",
    "tp_trigger_px",
    "tpTriggerPx",
)

_ACTION_TEXT_FIELDS = (
    "symbol",
    "side",
    "order_type",
    "reason",
    "order_side",
    "close_side",
    "inst_type",
    "timeframe",
    "price_source",
    "exchange_size",
    "planned_exit_reason",
    "planned_exit_contract",
    "order_id",
    "client_order_id",
    "new_size",
    "new_price",
    "request_id",
    "target_order_kind",
    "target_order_type",
)


def _order_type_requires_explicit_price(order_type) -> bool:
    return _normalize_order_type_text(order_type) in _PRICE_REQUIRED_ORDER_TYPES


def _normalize_order_type_text(value) -> str:
    normalized = str(value or "").strip().lower().replace("-", "_")
    return normalized or "market"


def _action_numeric(value):
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return None
    return _numeric(value)


def _normalize_optional_number(action: dict, normalized: dict, key: str) -> None:
    if key not in action or action.get(key) is None:
        normalized.pop(key, None)
        return
    value = _action_numeric(action.get(key))
    if value is None:
        raise TypeError(
            f"StrategyDecision.actions[].{key} 必须是 JSON number，不能使用字符串数字。"
        )
    normalized[key] = float(value)


def _action_text(action: dict, key: str):
    if key not in action or action.get(key) is None:
        return None
    value = action.get(key)
    if not isinstance(value, str):
        raise TypeError(f"StrategyDecision.actions[].{key} 必须是 JSON string。")
    value = value.strip()
    return value or None


def _normalize_target_order_kind(value):
    if value is None:
        return None
    normalized = str(value).strip().lower()
    if normalized in _SUPPORTED_TARGET_ORDER_KINDS:
        return normalized
    supported = ", ".join(sorted(_SUPPORTED_TARGET_ORDER_KINDS))
    raise TypeError(
        f"StrategyDecision.actions[].target_order_kind={normalized} 不受支持，必须使用: {supported}"
    )


def _normalize_target_order_type(value):
    if value is None:
        return None
    normalized = _normalize_order_type_text(value)
    if normalized in _SUPPORTED_TARGET_ORDER_TYPES:
        return normalized
    supported = ", ".join(sorted(_SUPPORTED_TARGET_ORDER_TYPES))
    raise TypeError(
        f"StrategyDecision.actions[].target_order_type={normalized} 不受支持，必须使用保护单类型: {supported}"
    )


def _normalize_action(action, config: dict) -> dict:
    if not isinstance(action, dict):
        raise TypeError("StrategyDecision.actions 中的元素必须是 dict")

    for legacy_key in ("intent_action", "signal_type", "type"):
        if legacy_key in action:
            raise TypeError(
                f"StrategyDecision.actions[].{legacy_key} 旧动作别名已删除，请使用 action。"
            )
    for alias_key, canonical_key in _REMOVED_ACTION_FIELD_ALIASES.items():
        if alias_key in action:
            raise TypeError(
                f"StrategyDecision.actions[].{alias_key} 字段别名已删除，请使用 {canonical_key}。"
            )
    for field_key, reason in _ENGINE_CONTROLLED_ACTION_FIELDS.items():
        if field_key in action:
            raise TypeError(
                f"StrategyDecision.actions[].{field_key} 是交易引擎控制字段，{reason}。"
            )
    explicit_action = _action_text(action, "action")
    action_name = _canonical_action_name(explicit_action)
    text_fields = {key: _action_text(action, key) for key in _ACTION_TEXT_FIELDS}
    side = str(text_fields.get("side") or "").strip().lower()
    if not side:
        if action_name == "hold":
            side = "hold"
        elif action_name == "close_position":
            side = "flat"
    timestamp_value = _action_numeric(action.get("timestamp"))
    if timestamp_value is None or timestamp_value <= 0:
        raise TypeError(
            "StrategyDecision.actions[].timestamp 必须显式返回有效 JSON number 毫秒时间戳。"
        )
    timestamp = int(timestamp_value)
    order_type = _normalize_order_type_text(text_fields.get("order_type"))
    has_explicit_price = "price" in action and action.get("price") is not None
    price = _action_numeric(action.get("price")) if has_explicit_price else None
    invalid_explicit_price = has_explicit_price and price is None
    has_explicit_reference_price = (
        "reference_price" in action and action.get("reference_price") is not None
    )
    reference_price = (
        _action_numeric(action.get("reference_price"))
        if has_explicit_reference_price
        else None
    )
    invalid_reference_price = has_explicit_reference_price and reference_price is None
    missing_required_price = (
        not has_explicit_price
        and not invalid_explicit_price
        and _order_type_requires_explicit_price(order_type)
    )
    price_source = text_fields.get("price_source")
    if price is not None:
        price_source = price_source or "explicit"
        if reference_price is None:
            reference_price = price
    elif reference_price is not None:
        price_source = price_source or "reference_price"
    has_explicit_strength = "strength" in action and action.get("strength") is not None
    strength = _action_numeric(action.get("strength")) if has_explicit_strength else None
    if has_explicit_strength and strength is None:
        raise TypeError(
            "StrategyDecision.actions[].strength 必须是 JSON number，不能使用字符串数字。"
        )
    has_explicit_position_size = (
        "position_size" in action and action.get("position_size") is not None
    )
    position_size = (
        _action_numeric(action.get("position_size"))
        if has_explicit_position_size
        else None
    )
    if has_explicit_position_size and position_size is None:
        raise TypeError(
            "StrategyDecision.actions[].position_size 必须是 JSON number，不能使用字符串数字。"
        )
    exchange_size = text_fields.get("exchange_size")
    explicit_symbol = text_fields.get("symbol")

    normalized = dict(action)
    normalized.update(
        {
            "action": action_name,
            "symbol": str(explicit_symbol or config.get("symbol") or ""),
            "_symbol_explicit": explicit_symbol is not None,
            "side": side or "hold",
            "order_type": order_type,
            "reason": text_fields.get("reason") or "",
            "strength": float(strength if strength is not None else 0.5),
            "timestamp": timestamp,
        }
    )
    if price is not None:
        normalized["price"] = float(price)
        normalized.pop("_invalid_price", None)
        normalized.pop("_missing_required_price", None)
    elif invalid_explicit_price:
        normalized.pop("price", None)
        normalized["_invalid_price"] = True
        normalized.pop("_missing_required_price", None)
        price_source = "invalid_explicit"
    elif missing_required_price:
        normalized.pop("price", None)
        normalized.pop("_invalid_price", None)
        normalized["_missing_required_price"] = True
        price_source = "missing_required"
    else:
        normalized.pop("price", None)
        normalized.pop("_invalid_price", None)
        normalized.pop("_missing_required_price", None)
    if reference_price is not None:
        normalized["reference_price"] = float(reference_price)
        normalized.pop("_invalid_reference_price", None)
    elif invalid_reference_price:
        normalized.pop("reference_price", None)
        normalized["_invalid_reference_price"] = True
        price_source = price_source or "invalid_reference"
    else:
        normalized.pop("reference_price", None)
        normalized.pop("_invalid_reference_price", None)
    if price_source:
        normalized["price_source"] = price_source
    else:
        normalized.pop("price_source", None)
    if position_size is not None:
        normalized["position_size"] = float(position_size)
    else:
        normalized.pop("position_size", None)
    if exchange_size:
        normalized["exchange_size"] = exchange_size
    else:
        normalized.pop("exchange_size", None)
    for key in (
        "order_side",
        "close_side",
        "inst_type",
        "timeframe",
        "planned_exit_reason",
        "planned_exit_contract",
        "order_id",
        "client_order_id",
        "new_size",
        "new_price",
        "request_id",
    ):
        value = text_fields.get(key)
        if value:
            normalized[key] = value
        else:
            normalized.pop(key, None)
    target_order_kind = _normalize_target_order_kind(text_fields.get("target_order_kind"))
    if target_order_kind:
        normalized["target_order_kind"] = target_order_kind
    else:
        normalized.pop("target_order_kind", None)
    target_order_type = _normalize_target_order_type(text_fields.get("target_order_type"))
    if target_order_type:
        normalized["target_order_type"] = target_order_type
    else:
        normalized.pop("target_order_type", None)
    if "planned_exit_time" in action and action.get("planned_exit_time") is not None:
        planned_exit_time = _action_numeric(action.get("planned_exit_time"))
        if planned_exit_time is None or planned_exit_time <= 0:
            raise TypeError(
                "StrategyDecision.actions[].planned_exit_time 必须是有效 JSON number 毫秒时间戳。"
            )
        normalized["planned_exit_time"] = int(planned_exit_time)
    else:
        normalized.pop("planned_exit_time", None)
    for key in _ACTION_NUMERIC_FIELDS:
        _normalize_optional_number(action, normalized, key)
    if "cancel_on_fail" in action and action.get("cancel_on_fail") is not None:
        if not isinstance(action.get("cancel_on_fail"), bool):
            raise TypeError("StrategyDecision.actions[].cancel_on_fail 必须是 JSON boolean。")
        normalized["cancel_on_fail"] = action["cancel_on_fail"]
    else:
        normalized.pop("cancel_on_fail", None)
    return normalized


def _canonical_action_name(value) -> str:
    name = str(value or "").strip().lower()
    if name in _SUPPORTED_ACTIONS:
        return name
    supported = ", ".join(sorted(_SUPPORTED_ACTIONS))
    if not name:
        raise TypeError(f"StrategyDecision.actions[] 缺少 action，必须显式使用: {supported}")
    raise TypeError(f"StrategyDecision.actions[].action={name} 不受支持，必须使用: {supported}")


def _decision_actions(decision: dict) -> list:
    removed_keys = [key for key in _REMOVED_TOP_LEVEL_FIELDS if key in decision]
    if removed_keys:
        names = "/".join(removed_keys)
        raise TypeError(
            f"StrategyDecision.{names} 旧合约已删除，请显式返回 actions。"
        )
    if "actions" not in decision:
        raise TypeError("StrategyDecision.actions 是必填 list；无交易时请返回 []，或返回带 timestamp 的 hold。")
    actions = decision.get("actions")
    if isinstance(actions, list):
        return actions
    if actions is not None:
        raise TypeError("StrategyDecision.actions 必须是 list")
    raise TypeError("StrategyDecision.actions 是必填 list；无交易时请返回 []，或返回带 timestamp 的 hold。")


def _normalize_execution_log(entry, index: int) -> dict:
    if not isinstance(entry, dict):
        raise TypeError(f"StrategyDecision.execution_logs[{index}] 必须是 dict")
    for alias_key, canonical_key in _REMOVED_EXECUTION_LOG_FIELD_ALIASES.items():
        if alias_key in entry:
            raise TypeError(
                f"StrategyDecision.execution_logs[{index}].{alias_key} 字段别名已删除，请使用 {canonical_key}。"
            )
    message = entry.get("message")
    if not isinstance(message, str) or not message.strip():
        raise TypeError(f"StrategyDecision.execution_logs[{index}].message 必须是非空字符串")
    stage = entry.get("stage")
    if not isinstance(stage, str) or not stage.strip():
        raise TypeError(f"StrategyDecision.execution_logs[{index}].stage 必须是非空字符串")
    level = entry.get("level")
    if level not in {"info", "warn", "error", "success"}:
        raise TypeError(
            f"StrategyDecision.execution_logs[{index}].level 必须是 info/warn/error/success"
        )
    if "details" not in entry:
        raise TypeError(f"StrategyDecision.execution_logs[{index}].details 必须显式返回 dict")
    details = entry.get("details")
    if not isinstance(details, dict):
        raise TypeError(f"StrategyDecision.execution_logs[{index}].details 必须是 dict")
    return {
        "stage": stage,
        "level": level,
        "message": message,
        "details": details,
    }


def _decision_execution_logs(decision: dict) -> list[dict]:
    if "execution_logs" not in decision:
        raise TypeError("StrategyDecision.execution_logs 是必填 list；无日志时请显式返回 []。")
    logs = decision.get("execution_logs")
    if not isinstance(logs, list):
        raise TypeError("StrategyDecision.execution_logs 必须是 list")
    return [_normalize_execution_log(log, index) for index, log in enumerate(logs)]


def _decision_from_evaluate(module, context: dict, params: dict, config: dict) -> dict:
    evaluate = getattr(module, "evaluate", None)
    if not callable(evaluate):
        raise TypeError("策略文件中未找到 evaluate 函数")
    decision = evaluate(context, params)
    return _decision_from_strategy_decision(decision, config)


def _decision_from_strategy_decision(decision: dict, config: dict) -> dict:
    if not isinstance(decision, dict):
        raise TypeError("evaluate 必须返回 dict")

    raw_actions = _decision_actions(decision)
    actions = [_normalize_action(action, config) for action in raw_actions]
    execution_logs = _decision_execution_logs(decision)

    if "diagnostics" not in decision:
        raise TypeError("StrategyDecision.diagnostics 是必填 dict；无诊断信息时请显式返回 {}。")
    if "indicators" not in decision:
        raise TypeError("StrategyDecision.indicators 是必填 dict；无指标信息时请显式返回 {}。")
    indicators = decision.get("indicators")
    diagnostics = decision.get("diagnostics")
    if not isinstance(indicators, dict):
        raise TypeError("StrategyDecision.indicators 必须是 dict")
    if not isinstance(diagnostics, dict):
        raise TypeError("StrategyDecision.diagnostics 必须是 dict")
    return {
        "actions": actions,
        "indicators": indicators,
        "diagnostics": diagnostics,
        "execution_logs": execution_logs,
    }
