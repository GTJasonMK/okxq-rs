"""Streaming student selector gate for the ML trade selector runtime."""

from __future__ import annotations

from datetime import datetime, timezone
import json

from lib.ml_trade_model_scoring import artifact_dir, bool_param, num, positive_scores

STUDENT_SELECTOR_MODEL_FILE = "student_selector_v1.joblib"
STUDENT_SELECTOR_CONTRACT_FILE = "student_selector_contract.json"

STUDENT_SELECTOR_ARTIFACT_MISSING = "student_selector_artifact_missing"
STUDENT_SELECTOR_CONTRACT_MISSING = "student_selector_contract_missing"
STUDENT_SELECTOR_DEPENDENCY_MISSING = "student_selector_dependency_missing"
STUDENT_SELECTOR_LOAD_FAILED = "student_selector_load_failed"
STUDENT_SELECTOR_FEATURES_MISSING = "student_selector_features_missing"
STUDENT_SELECTOR_SCORE_FAILED = "student_selector_score_failed"
STUDENT_SELECTOR_CONTRACT_FORBIDDEN_FEATURES = "student_selector_contract_forbidden_features"

FORBIDDEN_STUDENT_LIVE_INPUT_COLUMNS = {
    "teacher_top3_label",
    "net_positive_label",
    "net_bps_ge_label",
    "net_positive_no_very_bad_label",
    "net_bps_after_40bps",
    "very_bad_trade_label",
    "exit_time",
    "exit_reason",
    "mae_bps",
    "mfe_bps",
}

_STUDENT_SELECTOR_CACHE = {}


def load_student_selector_bundle(params):
    directory = artifact_dir(params)
    model_name = str(params.get("student_selector_model_file") or STUDENT_SELECTOR_MODEL_FILE)
    contract_name = str(params.get("student_selector_contract_file") or STUDENT_SELECTOR_CONTRACT_FILE)
    model_path = directory / model_name
    contract_path = directory / contract_name
    cache_key = (str(model_path), str(contract_path))
    cached = _STUDENT_SELECTOR_CACHE.get(cache_key)
    if cached is not None:
        return cached

    if not model_path.is_file():
        bundle = unavailable_student_bundle(STUDENT_SELECTOR_ARTIFACT_MISSING, model_path, contract_path)
        _STUDENT_SELECTOR_CACHE[cache_key] = bundle
        return bundle
    if not contract_path.is_file():
        bundle = unavailable_student_bundle(STUDENT_SELECTOR_CONTRACT_MISSING, model_path, contract_path)
        _STUDENT_SELECTOR_CACHE[cache_key] = bundle
        return bundle

    try:
        import joblib  # noqa: PLC0415
        import pandas as pd  # noqa: PLC0415
    except Exception as error:  # pragma: no cover - runtime dependency specific.
        bundle = unavailable_student_bundle(
            STUDENT_SELECTOR_DEPENDENCY_MISSING,
            model_path,
            contract_path,
            str(error),
        )
        _STUDENT_SELECTOR_CACHE[cache_key] = bundle
        return bundle

    try:
        contract = json.loads(contract_path.read_text(encoding="utf-8"))
        model = joblib.load(model_path)
    except Exception as error:  # pragma: no cover - artifact specific.
        bundle = unavailable_student_bundle(
            STUDENT_SELECTOR_LOAD_FAILED,
            model_path,
            contract_path,
            str(error),
        )
        _STUDENT_SELECTOR_CACHE[cache_key] = bundle
        return bundle

    feature_columns = contract.get("feature_columns")
    if not isinstance(feature_columns, list) or not feature_columns:
        bundle = unavailable_student_bundle(
            STUDENT_SELECTOR_CONTRACT_MISSING,
            model_path,
            contract_path,
            "student_selector_contract.feature_columns missing",
        )
        _STUDENT_SELECTOR_CACHE[cache_key] = bundle
        return bundle

    feature_columns = [str(item) for item in feature_columns]
    forbidden = set(FORBIDDEN_STUDENT_LIVE_INPUT_COLUMNS)
    forbidden.update(str(item) for item in contract.get("forbidden_live_input_columns") or [])
    forbidden_features = sorted(set(feature_columns) & forbidden)
    if forbidden_features:
        bundle = unavailable_student_bundle(
            STUDENT_SELECTOR_CONTRACT_FORBIDDEN_FEATURES,
            model_path,
            contract_path,
            "student selector contract includes forbidden live inputs: " + ",".join(forbidden_features[:20]),
        )
        _STUDENT_SELECTOR_CACHE[cache_key] = bundle
        return bundle

    bundle = {
        "available": True,
        "status": "student_selector_loaded",
        "model_path": str(model_path),
        "contract_path": str(contract_path),
        "model": model,
        "contract": contract,
        "feature_columns": feature_columns,
        "numeric_feature_columns": {str(item) for item in contract.get("numeric_feature_columns") or []},
        "pd": pd,
    }
    _STUDENT_SELECTOR_CACHE[cache_key] = bundle
    return bundle


def unavailable_student_bundle(status, model_path, contract_path, detail=None):
    return {
        "available": False,
        "status": status,
        "model_path": str(model_path),
        "contract_path": str(contract_path),
        "detail": detail,
    }


def apply_streaming_teacher_gate(context, params, ranked, timestamp):
    timestamp = int(timestamp or 0)
    daily_cap = non_negative_int_param(params, "daily_open_cap", 3)
    per_eval_cap = non_negative_int_param(params, "per_eval_open_cap", 3)
    daily_count = daily_open_count(context, timestamp)
    remaining = max(0, daily_cap - daily_count) if daily_cap > 0 else 0
    report = {
        "enabled": True,
        "selection_rule": "streaming_teacher_gate_v1",
        "timestamp": timestamp,
        "day_bucket": utc_day_bucket(timestamp),
        "daily_open_cap": daily_cap,
        "daily_open_count": daily_count,
        "daily_open_remaining": remaining,
        "per_eval_open_cap": per_eval_cap,
        "input_count": len(ranked or []),
        "student_threshold": student_probability_threshold(params),
        "risk_probability_cap": risk_probability_cap(params),
        "parameter_precision_warnings": probability_parameter_precision_warnings(params),
        "blocked_by_reason": {},
        "student_model_status": "pending_artifact_scoring",
    }
    if not ranked:
        return [], report
    if remaining <= 0 or per_eval_cap <= 0:
        report["blocked_by_reason"]["daily_open_cap_reached"] = len(ranked)
        report["selected_count"] = 0
        return [], report

    candidates = [dict(item) for item in ranked if isinstance(item, dict)]
    clear_runtime_student_probabilities(candidates)
    bundle = load_student_selector_bundle(params)
    report["student_model_status"] = bundle.get("status", "unknown")
    report["student_model_path"] = bundle.get("model_path")
    report["student_contract_path"] = bundle.get("contract_path")
    if not bundle.get("available"):
        reason = str(bundle.get("status") or STUDENT_SELECTOR_ARTIFACT_MISSING)
        report["blocked_by_reason"][reason] = len(candidates)
        if bundle.get("detail"):
            report["student_model_detail"] = bundle.get("detail")
        report["selected_count"] = 0
        return [], report

    score_student_selector_items(bundle, candidates, report)

    active_symbols = current_active_symbols(context) if bool_param(params, "block_existing_symbol_exposure", True) else set()
    passed = []
    threshold = report["student_threshold"]
    risk_cap = report["risk_probability_cap"]
    seen_symbols = set()
    for item in candidates:
        reasons = gate_block_reasons(item, threshold, risk_cap, active_symbols, seen_symbols, params)
        if reasons:
            item["student_gate_pass"] = False
            item["gate_blocked_by"] = reasons
            for reason in reasons:
                report["blocked_by_reason"][reason] = report["blocked_by_reason"].get(reason, 0) + 1
            continue
        item["student_gate_pass"] = True
        item["gate_blocked_by"] = []
        passed.append(item)
        if bool_param(params, "unique_symbol_selection", True):
            seen_symbols.add(str(item.get("symbol") or "").strip().upper())

    passed.sort(
        key=lambda item: (
            -float(student_execution_probability(item) or 0.0),
            -float(num(item.get("adjusted_score"), float("-inf")) or float("-inf")),
            int(num(item.get("rank_order"), 0) or 0),
            str(item.get("symbol") or ""),
            str(item.get("side") or ""),
        )
    )
    limit = max(0, min(int(remaining), int(per_eval_cap)))
    selected = passed[:limit]
    for index, item in enumerate(selected, 1):
        item["selection_rank"] = index
    report["scored_count"] = sum(1 for item in candidates if student_execution_probability(item) is not None)
    report["passed_count"] = len(passed)
    report["selected_count"] = len(selected)
    report["selected_symbols"] = [str(item.get("symbol") or "") for item in selected]
    return selected, report


def clear_runtime_student_probabilities(items):
    for item in items:
        item.pop("student_execution_prob", None)
        item.pop("student_top3_prob", None)
        item.pop("student_score_source", None)


def score_student_selector_items(bundle, items, report):
    rows = []
    complete = []
    missing_count = 0
    missing_sample = None
    feature_columns = bundle["feature_columns"]
    numeric_columns = bundle.get("numeric_feature_columns") or set()
    for item in items:
        source = student_feature_source(item)
        row = {}
        missing = []
        for column in feature_columns:
            if column not in source:
                missing.append(column)
                row[column] = None
                continue
            value = source.get(column)
            row[column] = num(value, None) if column in numeric_columns else (None if value is None else str(value))
        if missing:
            missing_count += 1
            if missing_sample is None:
                missing_sample = missing[:10]
            item["student_selector_missing_features"] = missing[:10]
            continue
        rows.append(row)
        complete.append(item)

    if missing_count:
        report["blocked_by_reason"][STUDENT_SELECTOR_FEATURES_MISSING] = (
            report["blocked_by_reason"].get(STUDENT_SELECTOR_FEATURES_MISSING, 0) + missing_count
        )
        report["student_missing_feature_sample"] = missing_sample or []
    if not complete:
        return

    try:
        pd = bundle["pd"]
        x = pd.DataFrame.from_records(rows, columns=feature_columns)
        model = bundle["model"]
        estimator = model.get("selector_model") if isinstance(model, dict) else model
        probabilities = positive_scores(estimator, x)
    except Exception as error:  # pragma: no cover - artifact specific.
        report["blocked_by_reason"][STUDENT_SELECTOR_SCORE_FAILED] = (
            report["blocked_by_reason"].get(STUDENT_SELECTOR_SCORE_FAILED, 0) + len(complete)
        )
        report["student_score_error_detail"] = str(error)
        return

    for item, probability in zip(complete, probabilities):
        set_student_execution_probability(item, probability)
        item["student_score_source"] = "student_selector_v1"


def student_feature_source(item):
    row = item.get("row") if isinstance(item.get("row"), dict) else {}
    source = dict(row)
    source.update(
        {
            "teacher_return_pred": item.get("teacher_return_pred"),
            "teacher_risk_prob": item.get("teacher_risk_prob"),
            "teacher_score": item.get("teacher_score", item.get("score")),
            "teacher_adjusted_score": item.get("teacher_adjusted_score", item.get("adjusted_score")),
            "timestamp_rank": item.get("timestamp_rank"),
            "timestamp_candidate_count": item.get("timestamp_candidate_count"),
        }
    )
    return source


def gate_block_reasons(item, threshold, risk_cap, active_symbols, seen_symbols, params):
    reasons = []
    probability = student_execution_probability(item)
    if probability is None:
        reasons.append("student_selector_score_missing")
    elif probability < threshold:
        reasons.append("student_probability_below_threshold")

    risk = teacher_risk_probability(item)
    if risk_cap is not None:
        if risk is None:
            reasons.append("teacher_risk_probability_missing")
        elif risk > risk_cap:
            reasons.append("teacher_risk_probability_above_cap")

    symbol = str(item.get("symbol") or "").strip().upper()
    if symbol and symbol in active_symbols:
        reasons.append("symbol_already_active")
    if bool_param(params, "unique_symbol_selection", True) and symbol and symbol in seen_symbols:
        reasons.append("duplicate_symbol_candidate")
    return reasons


def student_execution_probability(item):
    value = num(item.get("student_execution_prob"), None)
    if value is not None:
        return clamp_probability(value)
    return None


def set_student_execution_probability(item, probability):
    value = clamp_probability(probability)
    item["student_execution_prob"] = value


def teacher_risk_probability(item):
    for key in ("teacher_risk_prob", "risk_prob", "bad_prob", "very_bad_prob"):
        value = num(item.get(key), None)
        if value is not None:
            return clamp_probability(value)
    row = item.get("row") if isinstance(item.get("row"), dict) else {}
    for key in ("teacher_risk_prob", "risk_prob", "bad_prob", "very_bad_prob"):
        value = num(row.get(key), None)
        if value is not None:
            return clamp_probability(value)
    return None


def current_active_symbols(context):
    symbols = set()
    positions = context.get("positions") if isinstance(context, dict) else {}
    if isinstance(positions, dict):
        open_positions = positions.get("open")
        if isinstance(open_positions, list):
            for row in open_positions:
                add_symbol_if_present(symbols, row)
        else:
            for row in positions.values():
                if isinstance(row, dict):
                    quantity = num(row.get("quantity", row.get("pos")), 0.0) or 0.0
                    if abs(quantity) > 0.0:
                        add_symbol_if_present(symbols, row)

    orders = normalize_orders_context(context.get("orders") if isinstance(context, dict) else {})
    for row in orders.get("open", []):
        if runtime_open_order_row(row):
            add_symbol_if_present(symbols, row)
    return symbols


def add_symbol_if_present(symbols, row):
    if not isinstance(row, dict):
        return
    symbol = str(row.get("symbol") or row.get("inst_id") or row.get("instId") or "").strip().upper()
    if symbol:
        symbols.add(symbol)


def daily_open_count(context, timestamp):
    orders = normalize_orders_context(context.get("orders") if isinstance(context, dict) else {})
    target_day = utc_day_bucket(timestamp)
    if target_day is None:
        return 0
    seen = set()
    for bucket in ("open", "recent_fills"):
        for row in orders.get(bucket, []):
            if not runtime_open_order_row(row):
                continue
            row_ts = order_timestamp(row)
            if utc_day_bucket(row_ts) != target_day:
                continue
            seen.add(order_identity(row))
    return len(seen)


def normalize_orders_context(orders):
    if not isinstance(orders, dict):
        orders = {}
    return {
        "open": orders.get("open") if isinstance(orders.get("open"), list) else [],
        "recent_fills": orders.get("recent_fills") if isinstance(orders.get("recent_fills"), list) else [],
        "recent_rejections": orders.get("recent_rejections") if isinstance(orders.get("recent_rejections"), list) else [],
    }


def runtime_open_order_row(row):
    if not isinstance(row, dict):
        return False
    action = str(row.get("action") or "").strip().lower()
    if action not in {"open_position", "open"}:
        return False
    success = row.get("success")
    if success is False:
        return False
    status = str(row.get("status") or "").strip().lower()
    if not status:
        return True
    return status in {
        "submitting",
        "submit_unknown",
        "algo_submitted",
        "algo_submit_unknown",
        "algo_live",
        "open",
        "submitted",
        "pending",
        "live",
        "partially_filled",
        "partial-filled",
        "partially-filled",
        "filled",
        "fully_filled",
        "fully-filled",
        "algo_effective",
        "algo_partially_effective",
    }


def order_timestamp(row):
    for key in (
        "timestamp",
        "action_timestamp",
        "last_fill_ts",
        "first_fill_ts",
        "created_ts",
        "submitted_ts",
        "local_timestamp",
    ):
        value = num(row.get(key), None) if isinstance(row, dict) else None
        if value is not None and value > 0:
            return int(value)
    created_at = str(row.get("created_at") or row.get("local_created_at") or "").strip() if isinstance(row, dict) else ""
    if created_at:
        parsed = parse_datetime_ms(created_at)
        if parsed > 0:
            return parsed
    return 0


def parse_datetime_ms(value):
    try:
        text = value.replace("Z", "+00:00")
        parsed = datetime.fromisoformat(text)
        if parsed.tzinfo is None:
            parsed = parsed.replace(tzinfo=timezone.utc)
        return int(parsed.timestamp() * 1000)
    except Exception:
        return 0


def order_identity(row):
    for key in (
        "order_id",
        "actual_order_id",
        "client_order_id",
        "actual_client_order_id",
        "trade_id",
        "id",
    ):
        value = str(row.get(key) or "").strip() if isinstance(row, dict) else ""
        if value:
            return f"{key}:{value}"
    symbol = str(row.get("symbol") or row.get("inst_id") or row.get("instId") or "").strip().upper()
    timestamp = order_timestamp(row)
    return f"fallback:{symbol}:{timestamp}:{str(row.get('action') or '')}"


def utc_day_bucket(timestamp):
    try:
        timestamp = int(timestamp or 0)
    except (TypeError, ValueError):
        return None
    if timestamp <= 0:
        return None
    return timestamp // 86_400_000


def student_probability_threshold(params):
    value = num(params.get("student_execution_probability_threshold"), None) if isinstance(params, dict) else None
    if value is None:
        value = num(params.get("student_probability_threshold"), 0.5) if isinstance(params, dict) else 0.5
    return clamp_probability(value)


def risk_probability_cap(params):
    if not isinstance(params, dict):
        return None
    for key in ("risk_probability_cap", "teacher_risk_probability_cap", "bad_probability_cap"):
        value = num(params.get(key), None)
        if value is not None:
            return clamp_probability(value)
    return None


def probability_parameter_precision_warnings(params, max_decimal_places=2):
    if not isinstance(params, dict):
        return []
    warnings = []
    for key in ("student_execution_probability_threshold", "risk_probability_cap"):
        value = num(params.get(key), None)
        if value is None:
            continue
        places = decimal_places(value)
        if places <= int(max_decimal_places):
            continue
        warnings.append(
            {
                "key": key,
                "value": float(value),
                "decimal_places": places,
                "review_rounding": round(float(value), int(max_decimal_places)),
                "reason": "over_precise_probability_parameter_requires_oos_review",
            }
        )
    return warnings


def decimal_places(value):
    try:
        text = f"{float(value):.12f}".rstrip("0").rstrip(".")
    except (TypeError, ValueError):
        return 0
    if "." not in text:
        return 0
    return len(text.rsplit(".", 1)[1])


def clamp_probability(value):
    parsed = num(value, 0.0)
    return max(0.0, min(1.0, float(parsed or 0.0)))


def non_negative_int_param(params, key, default):
    value = num(params.get(key), default) if isinstance(params, dict) else default
    try:
        return max(0, int(value))
    except (TypeError, ValueError):
        return max(0, int(default))
