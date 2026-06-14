"""Model artifact loading and scoring helpers for the ML trade selector runtime."""

from __future__ import annotations

import bisect
from concurrent.futures import ProcessPoolExecutor, as_completed
import json
import math
import multiprocessing
import os
from pathlib import Path
import sys

from lib.ml_trade_resource_limits import memory_limited_worker_count, model_score_chunk_rows

UNIVERSE_SYMBOLS = [
    "HBAR-USDT-SWAP",
    "XRP-USDT-SWAP",
    "SUI-USDT-SWAP",
    "CRV-USDT-SWAP",
    "ADA-USDT-SWAP",
    "TRX-USDT-SWAP",
    "ARB-USDT-SWAP",
    "XLM-USDT-SWAP",
    "OP-USDT-SWAP",
    "LDO-USDT-SWAP",
    "DYDX-USDT-SWAP",
    "FIL-USDT-SWAP",
    "TON-USDT-SWAP",
    "NEAR-USDT-SWAP",
    "APT-USDT-SWAP",
    "SOL-USDT-SWAP",
    "DOT-USDT-SWAP",
    "LINK-USDT-SWAP",
    "UNI-USDT-SWAP",
    "ICP-USDT-SWAP",
    "AVAX-USDT-SWAP",
    "ETC-USDT-SWAP",
    "ETH-USDT-SWAP",
    "LTC-USDT-SWAP",
    "BCH-USDT-SWAP",
    "AAVE-USDT-SWAP",
    "BNB-USDT-SWAP",
    "COMP-USDT-SWAP",
    "BTC-USDT-SWAP",
]

FROZEN_CANDIDATE = {
    "research_date": "2026-06-07",
    "experiment_id": "ml_selector_badpenalty20_flatlite_stop450_fullgate_20260607",
    "model": "random_forest_regressor_bad_penalty_18_rf_e160_d8_l100_clip0",
    "score_formula": "predicted net_bps_after_40bps - 18bps * P(very_bad_trade_label=1)",
    "production_selection_rule": "streaming_teacher_gate_v1",
    "teacher_oracle_rule": "daily_top3_rank_weighted_research_label",
    "teacher_oracle_daily_count": 3,
    "rank_profile": "flatlite",
    "rank_weights": [0.35, 0.35, 0.35, 0.35, 0.35],
    "global_weight": 1.0,
    "model_risk_penalty_bps": 18.0,
    "context_penalty_bps": 40.0,
    "context_penalty_mode": "additive",
    "context_terms": [
        {"field": "market_breadth_24h_bin", "value": "strong"},
        {"field": "side_local_ret_24h_bin", "value": "surge"},
        {"field": "side", "value": "short"},
    ],
    "path_stop_loss_bps": 450.0,
    "rf_estimators": 160,
    "rf_max_depth": 8,
    "rf_min_samples_leaf": 100,
    "split_contract": {
        "fit_train": ["2021-09-01", "2024-09-01"],
        "selection_test": ["2024-09-01", "2025-09-01"],
        "audit_validation": ["2025-09-01", "2026-06-06"],
    },
    "audit_hard_gate_pass": True,
    "promotion_note": (
        "Audit-informed forward candidate; 2025-09..2026-06 was "
        "inspected during fastdiag exploration."
    ),
}
FROZEN_CONTEXT_TERMS = tuple(
    (str(term.get("field") or ""), str(term.get("value", "")).strip().lower())
    for term in FROZEN_CANDIDATE["context_terms"]
)

GENERATOR_STATUS_NOT_READY = "strategy_candidate_generator_not_implemented"
EXTERNAL_CONTEXT_STATUS = "external_candidate_context"
RUNTIME_CANDIDATE_GENERATOR = "runtime_candidate_layers_generated"
MODEL_ARTIFACT_MISSING = "model_artifact_missing"
MODEL_CONTRACT_MISSING = "model_contract_missing"
MODEL_DEPENDENCY_MISSING = "model_dependency_missing"
MODEL_LOAD_FAILED = "model_load_failed"
MODEL_FEATURES_MISSING = "candidate_feature_columns_missing"
MODEL_SCORE_FAILED = "model_score_failed"

DEFAULT_ARTIFACT_DIR = "artifacts/ml_trade_selector_forward_candidate_v1"
MODEL_FILE_NAME = "random_forest_regressor_bad_penalty_18_rf_e160_d8_l100_clip0.joblib"
CONTRACT_FILE_NAME = "schema_contract.json"

_MODEL_CACHE = {}
_PARALLEL_MODEL_SCORE_STATE = None


def num(value, default=None):
    try:
        if value is None:
            return default
        parsed = float(value)
        return parsed if math.isfinite(parsed) else default
    except (TypeError, ValueError):
        return default


def int_param(params, key, default):
    value = num(params.get(key), default)
    return max(1, int(value))


def bool_param(params, key, default):
    value = params.get(key) if isinstance(params, dict) else None
    if value is None:
        return bool(default)
    if isinstance(value, bool):
        return value
    return str(value).strip().lower() not in {"0", "false", "no", "off", "none"}


def artifact_dir(params):
    configured = params.get("model_artifact_dir") if isinstance(params, dict) else None
    if configured:
        path = Path(str(configured)).expanduser()
        return path if path.is_absolute() else strategies_root() / path
    return strategies_root() / DEFAULT_ARTIFACT_DIR


def strategies_root():
    return Path(__file__).resolve().parents[1]


def load_model_bundle(params):
    directory = artifact_dir(params)
    cache_key = str(directory.resolve() if directory.exists() else directory)
    cached = _MODEL_CACHE.get(cache_key)
    if cached is not None:
        return cached

    model_path = directory / MODEL_FILE_NAME
    contract_path = directory / CONTRACT_FILE_NAME
    if not model_path.is_file():
        bundle = unavailable_model_bundle(MODEL_ARTIFACT_MISSING, model_path, contract_path)
        _MODEL_CACHE[cache_key] = bundle
        return bundle
    if not contract_path.is_file():
        bundle = unavailable_model_bundle(MODEL_CONTRACT_MISSING, model_path, contract_path)
        _MODEL_CACHE[cache_key] = bundle
        return bundle

    try:
        import joblib  # noqa: PLC0415
        import pandas as pd  # noqa: PLC0415
    except Exception as error:  # pragma: no cover - depends on runtime env.
        bundle = unavailable_model_bundle(MODEL_DEPENDENCY_MISSING, model_path, contract_path, str(error))
        _MODEL_CACHE[cache_key] = bundle
        return bundle

    try:
        contract = json.loads(contract_path.read_text(encoding="utf-8"))
        model = joblib.load(model_path)
    except Exception as error:  # pragma: no cover - artifact specific.
        bundle = unavailable_model_bundle(MODEL_LOAD_FAILED, model_path, contract_path, str(error))
        _MODEL_CACHE[cache_key] = bundle
        return bundle

    feature_columns = contract.get("feature_columns")
    if not isinstance(feature_columns, list) or not feature_columns:
        bundle = unavailable_model_bundle(MODEL_CONTRACT_MISSING, model_path, contract_path, "schema_contract.feature_columns missing")
        _MODEL_CACHE[cache_key] = bundle
        return bundle

    feature_columns = [str(item) for item in feature_columns]
    numeric_feature_columns = {str(item) for item in (contract.get("numeric_feature_columns") or [])}
    bundle = {
        "available": True,
        "status": "model_artifact_loaded",
        "model_path": str(model_path),
        "contract_path": str(contract_path),
        "model": model,
        "contract": contract,
        "feature_columns": feature_columns,
        "feature_column_set": set(feature_columns),
        "numeric_feature_columns": numeric_feature_columns,
        "feature_input_specs": [(column, column in numeric_feature_columns) for column in feature_columns],
        "shared_preprocessor": shared_pipeline_preprocessor(model, joblib),
        "pd": pd,
    }
    _MODEL_CACHE[cache_key] = bundle
    return bundle


def unavailable_model_bundle(status, model_path, contract_path, detail=None):
    return {
        "available": False,
        "status": status,
        "model_path": str(model_path),
        "contract_path": str(contract_path),
        "detail": detail,
    }


def positive_scores(model, x):
    if hasattr(model, "predict_proba"):
        probabilities = model.predict_proba(x)
        return probabilities[:, 1]
    if hasattr(model, "decision_function"):
        return model.decision_function(x)
    return model.predict(x)


def shared_pipeline_preprocessor(model, joblib_module):
    if not isinstance(model, dict) or "return_model" not in model or "risk_model" not in model:
        return False
    try:
        return_pre = model["return_model"].named_steps.get("preprocessor")
        risk_pre = model["risk_model"].named_steps.get("preprocessor")
        return_estimator = model["return_model"].named_steps.get("model")
        risk_estimator = model["risk_model"].named_steps.get("model")
    except Exception:
        return False
    if return_pre is None or risk_pre is None or return_estimator is None or risk_estimator is None:
        return False
    try:
        return joblib_module.hash(return_pre) == joblib_module.hash(risk_pre)
    except Exception:
        return False


def score_shared_preprocessor_model(model, x, params=None):
    return_pipeline = model["return_model"]
    risk_pipeline = model["risk_model"]
    transformed = return_pipeline.named_steps["preprocessor"].transform(x)
    if should_parallel_model_prediction(params, x):
        return score_shared_preprocessor_model_parallel(model, transformed, params)
    return_scores = positive_scores(return_pipeline.named_steps["model"], transformed)
    risk_scores = positive_scores(risk_pipeline.named_steps["model"], transformed)
    return return_scores, risk_scores


def should_parallel_model_prediction(params, x):
    if not isinstance(params, dict):
        return False
    if not bool_param(params, "parallel_model_prediction", True):
        return False
    try:
        row_count = len(x)
    except TypeError:
        row_count = 0
    if row_count < int_param(params, "parallel_model_min_rows", 10_000):
        return False
    max_rows = int_param(params, "parallel_model_max_rows", 100_000)
    if max_rows > 0 and row_count > max_rows:
        return False
    return parallel_model_outer_jobs(params) > 1 and parallel_model_estimator_jobs(params) > 1


def parallel_model_outer_jobs(params):
    try:
        configured = int(params.get("parallel_model_outer_jobs", 2))
    except (TypeError, ValueError):
        configured = 2
    return max(1, min(2, configured, max(1, int(os.cpu_count() or 1))))


def parallel_model_estimator_jobs(params):
    default_jobs = min(6, max(1, int(os.cpu_count() or 1)))
    try:
        configured = int(params.get("parallel_model_estimator_jobs", default_jobs))
    except (TypeError, ValueError):
        configured = default_jobs
    return max(1, min(configured, max(1, int(os.cpu_count() or 1))))


def score_shared_preprocessor_model_parallel(model, transformed, params):
    try:
        from joblib import Parallel, delayed  # noqa: PLC0415
    except Exception:
        return_pipeline = model["return_model"]
        risk_pipeline = model["risk_model"]
        return (
            positive_scores(return_pipeline.named_steps["model"], transformed),
            positive_scores(risk_pipeline.named_steps["model"], transformed),
        )

    return_pipeline = model["return_model"]
    risk_pipeline = model["risk_model"]
    return_estimator = return_pipeline.named_steps["model"]
    risk_estimator = risk_pipeline.named_steps["model"]
    estimator_jobs = parallel_model_estimator_jobs(params)
    outer_jobs = parallel_model_outer_jobs(params)
    old_return_jobs = getattr(return_estimator, "n_jobs", None)
    old_risk_jobs = getattr(risk_estimator, "n_jobs", None)
    if hasattr(return_estimator, "n_jobs"):
        return_estimator.n_jobs = estimator_jobs
    if hasattr(risk_estimator, "n_jobs"):
        risk_estimator.n_jobs = estimator_jobs
    try:
        return_scores, risk_scores = Parallel(n_jobs=outer_jobs, prefer="threads")(
            [
                delayed(positive_scores)(return_estimator, transformed),
                delayed(positive_scores)(risk_estimator, transformed),
            ]
        )
    finally:
        if hasattr(return_estimator, "n_jobs"):
            return_estimator.n_jobs = old_return_jobs
        if hasattr(risk_estimator, "n_jobs"):
            risk_estimator.n_jobs = old_risk_jobs
    return return_scores, risk_scores


def score_candidate_with_model(row, bundle):
    if not bundle.get("available"):
        return None, {
            "status": bundle.get("status", MODEL_ARTIFACT_MISSING),
            "model_path": bundle.get("model_path"),
            "contract_path": bundle.get("contract_path"),
            "detail": bundle.get("detail"),
        }

    feature_columns = bundle["feature_columns"]
    missing = [column for column in feature_columns if column not in row]
    if missing:
        return None, {
            "status": MODEL_FEATURES_MISSING,
            "missing_count": len(missing),
            "missing_sample": missing[:10],
        }

    try:
        pd = bundle["pd"]
        x = pd.DataFrame(
            [model_input_row(row, bundle["contract"], feature_columns, bundle.get("numeric_feature_columns"))],
            columns=feature_columns,
        )
        model = bundle["model"]
        penalty_bps = FROZEN_CANDIDATE["context_penalty_bps"]
        if isinstance(model, dict) and "return_model" in model and "risk_model" in model:
            if bundle.get("shared_preprocessor"):
                return_values, risk_values = score_shared_preprocessor_model(model, x)
            else:
                return_values = positive_scores(model["return_model"], x)
                risk_values = positive_scores(model["risk_model"], x)
            return_score = float(return_values[0])
            risk_score = float(risk_values[0])
            penalty_bps = model_risk_penalty_bps(model)
            return return_score - penalty_bps * risk_score, {
                "status": "model_scored",
                "score_source": "model_risk_adjusted",
            }
        return float(positive_scores(model, x)[0]), {
            "status": "model_scored",
            "score_source": "model",
        }
    except Exception as error:  # pragma: no cover - model/artifact specific.
        return None, {"status": MODEL_SCORE_FAILED, "detail": str(error)}


def model_risk_penalty_bps(model):
    penalty = model.get("risk_penalty_bps") if isinstance(model, dict) else None
    return (
        float(penalty)
        if num(penalty, None) is not None
        else float(FROZEN_CANDIDATE["model_risk_penalty_bps"])
    )


def model_input_row(row, contract, feature_columns=None, numeric=None):
    numeric = numeric if isinstance(numeric, set) else set(contract.get("numeric_feature_columns") or [])
    feature_columns = feature_columns if isinstance(feature_columns, list) else contract.get("feature_columns") or []
    out = {}
    for column in feature_columns:
        value = row.get(column)
        if column in numeric:
            out[column] = num(value, None)
        else:
            out[column] = None if value is None else str(value)
    return out


def model_input_values(row, input_specs):
    out = []
    append = out.append
    row_get = row.get
    isfinite = math.isfinite
    for column, is_numeric in input_specs:
        value = row_get(column)
        if is_numeric:
            if value is None:
                append(None)
            elif isinstance(value, float):
                append(value if isfinite(value) else None)
            elif isinstance(value, int):
                parsed = float(value)
                append(parsed if isfinite(parsed) else None)
            else:
                append(num(value, None))
        else:
            if value is None:
                append(None)
            elif isinstance(value, str):
                append(value)
            else:
                append(str(value))
    return out


_MISSING = object()


def model_input_values_with_missing(row, input_specs):
    out = []
    append = out.append
    missing = None
    row_get = row.get
    isfinite = math.isfinite
    marker = _MISSING
    for column, is_numeric in input_specs:
        value = row_get(column, marker)
        if value is marker:
            if missing is None:
                missing = []
            missing.append(column)
            append(None)
            continue
        if is_numeric:
            if value is None:
                append(None)
            elif isinstance(value, float):
                append(value if isfinite(value) else None)
            elif isinstance(value, int):
                parsed = float(value)
                append(parsed if isfinite(parsed) else None)
            else:
                append(num(value, None))
        else:
            if value is None:
                append(None)
            elif isinstance(value, str):
                append(value)
            else:
                append(str(value))
    return out, missing


def should_use_fast_model_frame(params, bundle):
    return (
        bool_param(params, "fast_model_dataframe_from_records", True)
        and isinstance(bundle.get("feature_column_set"), set)
    )


def should_parallel_model_batch_score(params, bundle, complete_count, fast_frame):
    if not fast_frame:
        return False
    if not isinstance(params, dict):
        return False
    if not bool_param(params, "parallel_model_batch_scoring", True):
        return False
    if os.name != "posix":
        return False
    if complete_count < int_param(params, "parallel_model_batch_min_rows", 50_000):
        return False
    if not bundle.get("shared_preprocessor"):
        return False
    try:
        multiprocessing.get_context("fork")
    except (RuntimeError, ValueError):
        return False
    return parallel_model_batch_worker_count(params, complete_count) > 1


def parallel_model_batch_worker_count(params, complete_count):
    default_workers = min(6, max(1, int(os.cpu_count() or 1)), max(1, int(complete_count or 1)))
    try:
        configured = int(params.get("parallel_model_batch_max_workers", default_workers))
    except (TypeError, ValueError):
        configured = default_workers
    configured = max(1, min(int(configured), max(1, int(complete_count or 1)), max(1, int(os.cpu_count() or 1))))
    return memory_limited_worker_count(
        params,
        configured,
        complete_count,
        worker_memory_key="parallel_model_batch_worker_memory_gb",
        default_worker_memory_gb=3.0,
    )


def parallel_model_batch_estimator_jobs(params):
    try:
        configured = int(params.get("parallel_model_batch_estimator_jobs", 1))
    except (TypeError, ValueError):
        configured = 1
    return max(1, min(configured, max(1, int(os.cpu_count() or 1))))


def score_complete_model_items_parallel(complete, bundle, params, progress_callback=None, include_components=False):
    workers = parallel_model_batch_worker_count(params, len(complete))
    estimator_jobs = parallel_model_batch_estimator_jobs(params)
    chunk_size = max(1, (len(complete) + workers - 1) // workers)
    chunks = [
        (start, min(len(complete), start + chunk_size), estimator_jobs)
        for start in range(0, len(complete), chunk_size)
    ]
    scores = [None] * len(complete)
    state = (complete, bundle, dict(params), bool(include_components))
    process_context = multiprocessing.get_context("fork")
    with ProcessPoolExecutor(
        max_workers=workers,
        mp_context=process_context,
        initializer=init_parallel_model_score_worker,
        initargs=(state,),
    ) as executor:
        futures = [executor.submit(parallel_model_score_worker, chunk) for chunk in chunks]
        completed = 0
        for future in as_completed(futures):
            start, chunk_scores = future.result()
            scores[start : start + len(chunk_scores)] = chunk_scores
            completed += len(chunk_scores)
            if callable(progress_callback):
                progress_callback(completed, len(complete))
    return scores


def init_parallel_model_score_worker(state):
    global _PARALLEL_MODEL_SCORE_STATE
    _PARALLEL_MODEL_SCORE_STATE = state


def parallel_model_score_worker(chunk):
    state = _PARALLEL_MODEL_SCORE_STATE
    if state is None:
        raise RuntimeError("parallel model score worker was not initialized")
    if len(state) == 3:
        complete, bundle, params = state
        include_components = False
    else:
        complete, bundle, params, include_components = state
    start, end, estimator_jobs = chunk
    pd = bundle["pd"]
    feature_columns = bundle["feature_columns"]
    model = bundle["model"]
    rows = [complete[index]["row"] for index in range(int(start), int(end))]
    x = pd.DataFrame.from_records(rows, columns=feature_columns)
    old_jobs = set_model_estimator_jobs(model, estimator_jobs)
    try:
        local_params = dict(params)
        local_params["parallel_model_prediction"] = False
        if include_components:
            scores = score_model_frame_components(bundle, x, local_params)
        elif isinstance(model, dict) and "return_model" in model and "risk_model" in model:
            if bundle.get("shared_preprocessor"):
                return_scores, risk_scores = score_shared_preprocessor_model(model, x, local_params)
            else:
                return_scores = positive_scores(model["return_model"], x)
                risk_scores = positive_scores(model["risk_model"], x)
            penalty_bps = model_risk_penalty_bps(model)
            scores = [float(ret) - penalty_bps * float(risk) for ret, risk in zip(return_scores, risk_scores)]
        else:
            scores = [float(value) for value in positive_scores(model, x)]
    finally:
        restore_model_estimator_jobs(old_jobs)
    return int(start), scores


def set_model_estimator_jobs(model, jobs):
    old_jobs = []
    if isinstance(model, dict):
        for key in ("return_model", "risk_model"):
            pipeline = model.get(key)
            estimator = pipeline.named_steps.get("model") if hasattr(pipeline, "named_steps") else None
            if estimator is not None and hasattr(estimator, "n_jobs"):
                old_jobs.append((estimator, estimator.n_jobs))
                estimator.n_jobs = jobs
    elif hasattr(model, "n_jobs"):
        old_jobs.append((model, model.n_jobs))
        model.n_jobs = jobs
    return old_jobs


def restore_model_estimator_jobs(old_jobs):
    for estimator, jobs in old_jobs:
        estimator.n_jobs = jobs


def score_model_frame(bundle, x, params):
    return [item["score"] for item in score_model_frame_components(bundle, x, params)]


def score_model_frame_components(bundle, x, params):
    model = bundle["model"]
    local_params = dict(params)
    if bool_param(params, "memory_limited_model_prediction", True):
        local_params["parallel_model_prediction"] = False
    old_jobs = []
    if bool_param(params, "memory_limited_model_estimator_jobs", True):
        old_jobs = set_model_estimator_jobs(
            model,
            int_param(params, "model_estimator_jobs", parallel_model_batch_estimator_jobs(params)),
        )
    try:
        if isinstance(model, dict) and "return_model" in model and "risk_model" in model:
            if bundle.get("shared_preprocessor"):
                return_scores, risk_scores = score_shared_preprocessor_model(model, x, local_params)
            else:
                return_scores = positive_scores(model["return_model"], x)
                risk_scores = positive_scores(model["risk_model"], x)
            penalty_bps = model_risk_penalty_bps(model)
            return [
                {
                    "score": float(ret) - penalty_bps * float(risk),
                    "return_pred": float(ret),
                    "risk_prob": float(risk),
                    "risk_penalty_bps": float(penalty_bps),
                    "score_source": "model_risk_adjusted",
                }
                for ret, risk in zip(return_scores, risk_scores)
            ]
        return [
            {
                "score": float(value),
                "return_pred": float(value),
                "risk_prob": None,
                "risk_penalty_bps": 0.0,
                "score_source": "model",
            }
            for value in positive_scores(model, x)
        ]
    finally:
        restore_model_estimator_jobs(old_jobs)


def score_model_items_serial_chunked(
    complete,
    model_rows,
    bundle,
    params,
    fast_frame,
    progress_callback=None,
    ):
    pd = bundle["pd"]
    feature_columns = bundle["feature_columns"]
    chunk_rows = model_score_chunk_rows(params, len(complete))
    scores = []
    for start in range(0, len(complete), chunk_rows):
        end = min(len(complete), start + chunk_rows)
        rows_chunk = model_rows[start:end]
        x = (
            pd.DataFrame.from_records(rows_chunk, columns=feature_columns)
            if fast_frame
            else pd.DataFrame(rows_chunk, columns=feature_columns)
        )
        scores.extend(score_model_frame(bundle, x, params))
        if callable(progress_callback):
            progress_callback(end, len(complete))
    return scores


def score_model_items_serial_chunked_components(
    complete,
    model_rows,
    bundle,
    params,
    fast_frame,
    progress_callback=None,
):
    pd = bundle["pd"]
    feature_columns = bundle["feature_columns"]
    chunk_rows = model_score_chunk_rows(params, len(complete))
    scores = []
    for start in range(0, len(complete), chunk_rows):
        end = min(len(complete), start + chunk_rows)
        rows_chunk = model_rows[start:end]
        x = (
            pd.DataFrame.from_records(rows_chunk, columns=feature_columns)
            if fast_frame
            else pd.DataFrame(rows_chunk, columns=feature_columns)
        )
        scores.extend(score_model_frame_components(bundle, x, params))
        if callable(progress_callback):
            progress_callback(end, len(complete))
    return scores


def missing_feature_columns(row, feature_columns, feature_column_set=None):
    if isinstance(feature_column_set, set) and feature_column_set.issubset(row.keys()):
        return []
    return [column for column in feature_columns if column not in row]
