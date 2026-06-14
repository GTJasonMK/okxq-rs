"""Candidate loading, ranking, and model scoring for the ML trade selector runtime."""

from __future__ import annotations

import bisect
import json
import math

from lib.ml_trade_candidate_layers import (
    generate_runtime_candidate_rows,
    generate_runtime_candidate_rows_for_timestamps,
)
from lib.ml_trade_model_scoring import (
    EXTERNAL_CONTEXT_STATUS,
    FROZEN_CANDIDATE,
    FROZEN_CONTEXT_TERMS,
    MODEL_ARTIFACT_MISSING,
    MODEL_FEATURES_MISSING,
    MODEL_SCORE_FAILED,
    RUNTIME_CANDIDATE_GENERATOR,
    UNIVERSE_SYMBOLS,
    bool_param,
    int_param,
    load_model_bundle,
    missing_feature_columns,
    model_input_values_with_missing,
    num,
    score_complete_model_items_parallel,
    score_model_items_serial_chunked_components,
    should_parallel_model_batch_score,
    should_use_fast_model_frame,
)
from lib.ml_trade_runtime_progress import emit_history_progress


def rank_cache_progress(start, span, local_progress):
    assert isinstance(local_progress, (int, float)) and not isinstance(local_progress, bool), (
        "rank cache local progress must be numeric"
    )
    assert math.isfinite(local_progress), "rank cache local progress must be finite"
    assert 0.0 <= local_progress <= 1.0, "rank cache local progress must be between 0 and 1"
    return start + span * local_progress

def candidate_rows_from_context(context):
    if not isinstance(context, dict):
        return []
    rows = context.get("ml_trade_candidates")
    if rows is None:
        rows = context.get("trade_candidates")
    return rows if isinstance(rows, list) else []

def generate_candidate_rows_from_context(context, params, progress_callback=None):
    """Generate strategy-owned candidate rows from raw runtime context."""

    blocker = runtime_candidate_model_blocker(params)
    if blocker is not None:
        return [], model_blocked_source_info(blocker)

    rows, generation = generate_runtime_candidate_rows(
        context,
        UNIVERSE_SYMBOLS,
        params,
        progress_callback=progress_callback,
    )
    if rows:
        return rows, {
            "candidate_source": RUNTIME_CANDIDATE_GENERATOR,
            "blocked_by": [],
            "summary": (
                "Runtime-local candidate generation produced promoted base-layer and "
                "universe_candidate_v1 rows. Research parity tests are still pending."
            ),
            "generation": generation,
        }
    status = generation.get("status", "no_runtime_candidate_signal")
    return [], {
        "candidate_source": "strategy_runtime_generator",
        "blocked_by": [status],
        "summary": (
            "Runtime-local candidate generation is implemented for the promoted base layers "
            "and universe_candidate_v1, but produced no current candidate rows."
        ),
        "generation": generation,
    }

def load_candidate_rows(context, params, progress_callback=None):
    external_rows = candidate_rows_from_context(context)
    if external_rows:
        return external_rows, {
            "candidate_source": EXTERNAL_CONTEXT_STATUS,
            "blocked_by": [],
            "summary": "Using explicit point-in-time candidate rows from context.",
        }
    return generate_candidate_rows_from_context(context, params, progress_callback=progress_callback)


def load_ranked_candidate_rows(context, params, timestamp, progress_callback=None):
    external_rows = candidate_rows_from_context(context)
    if external_rows:
        source_info = {
            "candidate_source": EXTERNAL_CONTEXT_STATUS,
            "blocked_by": [],
            "summary": "Using explicit point-in-time candidate rows from context.",
        }
        ranked, score_report = ranked_candidates(
            external_rows,
            params,
            timestamp,
            progress_callback=progress_callback,
        )
        return external_rows, source_info, ranked, score_report

    cached = cached_backtest_ranked_candidates(context, params, timestamp)
    if cached is not None:
        return cached

    rows, source_info = generate_candidate_rows_from_context(
        context,
        params,
        progress_callback=progress_callback,
    )
    ranked, score_report = ranked_candidates(
        rows,
        params,
        timestamp,
        progress_callback=progress_callback,
    )
    return rows, source_info, ranked, score_report


def cached_backtest_ranked_candidates(context, params, timestamp):
    runtime_cache = context.get("_runtime_cache") if isinstance(context, dict) else None
    if not isinstance(runtime_cache, dict):
        return None
    if not bool_param(params, "backtest_candidate_rank_cache", True):
        return None

    plan = context.get("_backtest_plan") if isinstance(context.get("_backtest_plan"), dict) else {}
    timestamps = normalized_backtest_plan_timestamps(plan)
    if not timestamps:
        return None

    cache = runtime_cache.setdefault("ranked_candidate_cache", {})
    cache_key = backtest_rank_cache_key(runtime_cache, params, timestamps)
    cached = cache.get(cache_key)
    if cached is None:
        cached = empty_backtest_rank_cache(timestamps, rank_cache_chunk_size(params))
        cache[cache_key] = cached

    timestamp = int(timestamp or 0)
    retained = cached.setdefault("retained_timestamps", set())
    if timestamp not in retained:
        chunk_timestamps = backtest_rank_cache_chunk(timestamps, timestamp, cached["chunk_size"])
        if not chunk_timestamps:
            return None
        chunk = build_backtest_rank_cache_chunk(context, runtime_cache, params, timestamps, chunk_timestamps)
        merge_backtest_rank_cache_chunk(cached, chunk, chunk_timestamps)
        prune_backtest_rank_cache(cached, timestamp)

    rows = cached.get("rows_by_timestamp", {}).get(timestamp, [])
    ranked = cached.get("ranked_by_timestamp", {}).get(timestamp, [])
    score_report = cached.get("score_report_by_timestamp", {}).get(timestamp, empty_score_report())
    source_info = dict(cached.get("source_info") or {})
    source_info["rank_cache"] = {
        "enabled": True,
        "timestamp_count": len(timestamps),
        "chunk_size": cached.get("chunk_size"),
        "retained_timestamp_count": len(cached.get("retained_timestamps") or []),
        "built_timestamp_count": cached.get("built_timestamp_count", 0),
        "generated_count": cached.get("generated_count", 0),
        "current_timestamp_rows": len(rows),
    }
    if not rows:
        source_info["blocked_by"] = ["no_runtime_candidate_signal"]
        source_info["summary"] = (
            "Runtime-local candidate generation cache was available, but produced no current "
            "candidate rows for this timestamp."
        )
    return rows, source_info, ranked, score_report


def normalized_backtest_plan_timestamps(plan):
    raw = plan.get("evaluation_timestamps") if isinstance(plan, dict) else None
    if not isinstance(raw, list):
        return []
    timestamps = []
    for timestamp in raw:
        try:
            parsed = int(timestamp or 0)
        except (TypeError, ValueError):
            continue
        if parsed > 0:
            timestamps.append(parsed)
    return sorted(set(timestamps))


def backtest_rank_cache_key(runtime_cache, params, timestamps):
    context_id = str(runtime_cache.get("context_id") or "")
    try:
        params_key = json.dumps(params, sort_keys=True, default=str, separators=(",", ":"))
    except TypeError:
        params_key = str(sorted((str(key), str(value)) for key, value in dict(params or {}).items()))
    timestamp_key = (
        len(timestamps),
        timestamps[0] if timestamps else 0,
        timestamps[-1] if timestamps else 0,
    )
    return ("runtime_rank_cache_v2", context_id, timestamp_key, params_key)


def rank_cache_chunk_size(params):
    try:
        configured = int(params.get("backtest_candidate_rank_cache_chunk_size", 128))
    except (TypeError, ValueError):
        configured = 128
    return max(1, min(configured, 12_288))


def empty_backtest_rank_cache(timestamps, chunk_size):
    return {
        "source_info": {},
        "rows_by_timestamp": {},
        "ranked_by_timestamp": {},
        "score_report_by_timestamp": {},
        "retained_timestamps": set(),
        "built_timestamp_count": 0,
        "generated_count": 0,
        "timestamp_count": len(timestamps),
        "chunk_size": int(chunk_size),
    }


def backtest_rank_cache_chunk(timestamps, timestamp, chunk_size):
    if not timestamps:
        return []
    index = bisect.bisect_left(timestamps, int(timestamp or 0))
    if index >= len(timestamps):
        return []
    if timestamps[index] != int(timestamp or 0):
        return []
    return timestamps[index : index + max(1, int(chunk_size or 1))]


def merge_backtest_rank_cache_chunk(cache, chunk, chunk_timestamps):
    cache["source_info"] = chunk.get("source_info") or cache.get("source_info") or {}
    for key in ("rows_by_timestamp", "ranked_by_timestamp", "score_report_by_timestamp"):
        cache.setdefault(key, {}).update(chunk.get(key) or {})
    retained = cache.setdefault("retained_timestamps", set())
    retained.update(int(timestamp) for timestamp in chunk_timestamps)
    cache["built_timestamp_count"] = int(cache.get("built_timestamp_count", 0) or 0) + len(chunk_timestamps)
    cache["generated_count"] = int(cache.get("generated_count", 0) or 0) + int(chunk.get("generated_count", 0) or 0)


def prune_backtest_rank_cache(cache, current_timestamp):
    retained = cache.setdefault("retained_timestamps", set())
    stale = [timestamp for timestamp in retained if int(timestamp) < int(current_timestamp or 0)]
    if not stale:
        return
    for timestamp in stale:
        cache.get("rows_by_timestamp", {}).pop(timestamp, None)
        cache.get("ranked_by_timestamp", {}).pop(timestamp, None)
        cache.get("score_report_by_timestamp", {}).pop(timestamp, None)
        retained.discard(timestamp)


def build_backtest_rank_cache_chunk(context, runtime_cache, params, all_timestamps, chunk_timestamps):
    static_context = runtime_cache.get("static_context")
    if isinstance(static_context, dict):
        batch_context = dict(static_context)
        batch_context["_runtime_cache"] = runtime_cache
    else:
        batch_context = context
    emit_history_progress(
        context,
        0.01,
        "candidate_rank_cache",
        (
            "ML selector: building backtest candidate/rank cache "
            f"for {len(chunk_timestamps)}/{len(all_timestamps)} timestamps"
        ),
        0,
        len(chunk_timestamps),
    )

    def report_generation_progress(stage, message, local_progress):
        emit_history_progress(
            context,
            rank_cache_progress(0.02, 0.58, local_progress),
            stage,
            message,
            extra={"rank_cache": True},
        )

    rows, source_info = generate_candidate_rows_from_context_for_timestamps(
        batch_context,
        params,
        chunk_timestamps,
        progress_callback=report_generation_progress,
    )
    rows_by_timestamp = rows_by_candidate_timestamp(rows)

    def report_ranking_progress(stage, message, local_progress):
        emit_history_progress(
            context,
            rank_cache_progress(0.60, 0.38, local_progress),
            stage,
            message,
            extra={"rank_cache": True, "candidate_count": len(rows)},
        )

    ranked_by_timestamp, score_report_by_timestamp = ranked_candidates_by_timestamp(
        rows_by_timestamp,
        params,
        chunk_timestamps,
        progress_callback=report_ranking_progress,
        select_per_timestamp=False,
    )
    emit_history_progress(
        context,
        0.99,
        "candidate_rank_cache",
        f"ML selector: cached {len(rows)} candidate rows for {len(chunk_timestamps)} timestamps",
        len(chunk_timestamps),
        len(chunk_timestamps),
        extra={"rank_cache": True, "candidate_count": len(rows)},
    )
    return {
        "source_info": source_info,
        "rows_by_timestamp": rows_by_timestamp,
        "ranked_by_timestamp": ranked_by_timestamp,
        "score_report_by_timestamp": score_report_by_timestamp,
        "generated_count": len(rows),
    }


def rows_by_candidate_timestamp(rows):
    grouped = {}
    for row in rows if isinstance(rows, list) else []:
        if not isinstance(row, dict):
            continue
        timestamp = num(row.get("timestamp") or row.get("entry_time") or row.get("entry_time_num"), None)
        if timestamp is None:
            continue
        grouped.setdefault(int(timestamp), []).append(row)
    return grouped

def candidate_symbol(row):
    value = row.get("symbol") or row.get("asset") or row.get("inst_id") or row.get("instId")
    return str(value or "").strip().upper()

def candidate_side(row):
    raw = str(row.get("side") or row.get("signal_side") or "").strip().lower()
    if raw in ("long", "buy"):
        return "long"
    if raw in ("short", "sell"):
        return "short"
    return ""

def explicit_candidate_score(row):
    for key in (
        "adjusted_score",
        "score",
        "ml_score",
        "risk_adjusted_score",
        "predicted_score_bps",
    ):
        value = num(row.get(key), None)
        if value is not None:
            return value
    return None

def model_context_blocker(row, params):
    if not bool_param(params, "strict_context_gating", False):
        return None

    if bool_param(params, "require_btc_context", False):
        if row.get("btc_regime") in (None, "", "unknown"):
            return "btc_context_incomplete"

    if bool_param(params, "require_market_context", False):
        market_count = num(row.get("market_context_count"), 0.0) or 0.0
        min_count = int_param(params, "min_market_context_count", 20)
        if market_count < min_count:
            return "market_context_incomplete"

    if bool_param(params, "require_funding_context", False):
        if str(row.get("funding_data_status", "")).strip().lower() != "available":
            return "funding_context_incomplete"

    return None

def term_matches(row, term):
    field = term.get("field")
    value = term.get("value")
    if not field:
        return False
    return str(row.get(field, "")).strip().lower() == str(value).strip().lower()

def context_penalty(row, params):
    penalty_bps = num(params.get("score_context_penalty_bps"), FROZEN_CANDIDATE["context_penalty_bps"])
    matches = context_penalty_match_count(row)
    return penalty_bps * matches

def context_penalty_with_bps(row, penalty_bps):
    return penalty_bps * context_penalty_match_count(row)

def context_penalty_match_count(row):
    matches = 0
    for field, expected in FROZEN_CONTEXT_TERMS:
        raw = row.get(field, "")
        if raw == expected or str(raw).strip().lower() == expected:
            matches += 1
    return matches

def ranked_candidates(rows, params, timestamp, progress_callback=None):
    timestamp = int(timestamp or 0)
    ranked_by_timestamp, score_report_by_timestamp = ranked_candidates_by_timestamp(
        {timestamp: rows},
        params,
        [timestamp],
        progress_callback=progress_callback,
        select_per_timestamp=False,
    )
    return ranked_by_timestamp.get(timestamp, []), score_report_by_timestamp.get(timestamp, empty_score_report())

def ranked_candidates_by_timestamp(
    rows_by_timestamp,
    params,
    timestamps,
    progress_callback=None,
    select_per_timestamp=True,
):
    selected_by_timestamp = {}
    score_report_by_timestamp = {}
    model_items = []
    rank_order = 0
    for timestamp in timestamps:
        timestamp = int(timestamp or 0)
        selected_by_timestamp[timestamp] = []
        score_report_by_timestamp[timestamp] = empty_score_report()
        for row in rows_by_timestamp.get(timestamp, []):
            rank_order += 1
            collect_candidate_for_ranking(
                row,
                params,
                timestamp,
                selected_by_timestamp[timestamp],
                score_report_by_timestamp[timestamp],
                model_items,
                rank_order,
            )

    if model_items:
        model_bundle = load_model_bundle(params)
        for timestamp in sorted({item["timestamp"] for item in model_items}):
            report = score_report_by_timestamp.setdefault(timestamp, empty_score_report())
            report["model_status"] = model_bundle.get("status", "unknown")
            report["model_path"] = model_bundle.get("model_path")
            report["contract_path"] = model_bundle.get("contract_path")
        if callable(progress_callback):
            progress_callback(
                "model_scoring",
                f"ML selector: scoring {len(model_items)} model candidate rows",
                0.05,
            )
        score_model_candidate_batch_by_timestamp(
            model_items,
            model_bundle,
            params,
            score_report_by_timestamp,
            selected_by_timestamp,
            progress_callback=progress_callback,
        )

    ranked_by_timestamp = {}
    for timestamp, selected in selected_by_timestamp.items():
        ranked_by_timestamp[timestamp] = ranked_candidates_for_timestamp(
            selected,
            params,
            select_per_timestamp=select_per_timestamp,
        )
    return ranked_by_timestamp, score_report_by_timestamp


def ranked_candidates_for_timestamp(selected, params, select_per_timestamp=True):
    del params, select_per_timestamp
    ranked = with_timestamp_ranks(sort_ranked_candidates(selected))
    return ranked


def with_timestamp_ranks(ranked):
    count = len(ranked)
    out = []
    for index, item in enumerate(ranked, 1):
        next_item = dict(item)
        next_item["timestamp_rank"] = index
        next_item["timestamp_candidate_count"] = count
        out.append(next_item)
    return out

def collect_candidate_for_ranking(row, params, timestamp, selected, score_report, model_items, rank_order):
    if not isinstance(row, dict):
        return
    symbol = candidate_symbol(row)
    side = candidate_side(row)
    if symbol not in UNIVERSE_SYMBOLS or side not in ("long", "short"):
        return
    row_ts = num(row.get("timestamp") or row.get("entry_time_num") or row.get("signal_time_num"), None)
    if row_ts is not None and timestamp > 0 and int(row_ts) != int(timestamp):
        return

    score = explicit_candidate_score(row)
    if score is not None:
        score_report["explicit_score_count"] += 1
        selected.append(
            ranked_item(
                row,
                symbol,
                side,
                score,
                params,
                rank_order=rank_order,
                model_score=explicit_model_score(row, score),
            )
        )
        return

    known_blocker = row.get("_model_context_blocker") if bool_param(params, "strict_context_gating", False) else None
    if known_blocker:
        context_reason = str(known_blocker)
        score_report["skipped_missing_score_count"] += 1
        score_report["skipped_by_reason"][context_reason] = (
            score_report["skipped_by_reason"].get(context_reason, 0) + 1
        )
        return

    context_reason = model_context_blocker(row, params)
    if context_reason:
        score_report["skipped_missing_score_count"] += 1
        score_report["skipped_by_reason"][context_reason] = (
            score_report["skipped_by_reason"].get(context_reason, 0) + 1
        )
        return
    model_items.append(
        {
            "row": row,
            "symbol": symbol,
            "side": side,
            "timestamp": int(timestamp or 0),
            "rank_order": int(rank_order or 0),
        }
    )

def empty_score_report():
    return {
        "explicit_score_count": 0,
        "model_score_count": 0,
        "skipped_missing_score_count": 0,
        "skipped_by_reason": {},
        "model_status": "not_loaded",
    }

def sort_ranked_candidates(selected):
    selected = list(selected)
    selected.sort(
        key=lambda item: (
            -num(item.get("adjusted_score"), float("-inf")),
            int(num(item.get("rank_order"), 0) or 0),
            str(item.get("symbol") or ""),
            str(item.get("side") or ""),
        )
    )
    return selected

def configured_rank_weights(params):
    values = params.get("rank_weights") if isinstance(params, dict) else None
    if isinstance(values, str):
        parsed = []
        for item in values.split(","):
            value = num(item.strip(), None)
            if value is not None:
                parsed.append(value)
        if parsed:
            return parsed
    if isinstance(values, (list, tuple)):
        parsed = [num(item, None) for item in values]
        parsed = [float(item) for item in parsed if item is not None]
        if parsed:
            return parsed
    single = num(params.get("rank_weight"), None) if isinstance(params, dict) else None
    if single is not None:
        return [float(single)]
    return [float(value) for value in FROZEN_CANDIDATE["rank_weights"]]

def selection_weight_for_rank(rank, params):
    weights = configured_rank_weights(params)
    if not weights:
        return 0.0
    profile_index = min(max(1, int(rank or 1)) - 1, len(weights) - 1)
    global_weight = num(params.get("global_weight"), FROZEN_CANDIDATE["global_weight"])
    return max(0.0, min(1.0, float(weights[profile_index]) * float(global_weight)))

def explicit_model_score(row, score):
    risk = first_num(row, ("teacher_risk_prob", "risk_prob", "bad_prob", "very_bad_prob"))
    ret = first_num(row, ("teacher_return_pred", "return_pred", "predicted_return_bps"))
    return {
        "score": score,
        "return_pred": score if ret is None else ret,
        "risk_prob": risk,
        "risk_penalty_bps": first_num(row, ("teacher_risk_penalty_bps", "risk_penalty_bps")),
        "score_source": "explicit_score",
    }


def first_num(row, keys):
    if not isinstance(row, dict):
        return None
    for key in keys:
        value = num(row.get(key), None)
        if value is not None:
            return value
    return None


def ranked_item(row, symbol, side, score, params, penalty_bps=None, rank_order=None, model_score=None):
    adjusted = score - (
        context_penalty(row, params)
        if penalty_bps is None
        else context_penalty_with_bps(row, penalty_bps)
    )
    item = {
        "row": row,
        "symbol": symbol,
        "side": side,
        "score": score,
        "adjusted_score": adjusted,
        "teacher_score": score,
        "teacher_adjusted_score": adjusted,
    }
    if isinstance(model_score, dict):
        if model_score.get("return_pred") is not None:
            item["teacher_return_pred"] = model_score.get("return_pred")
        if model_score.get("risk_prob") is not None:
            item["teacher_risk_prob"] = model_score.get("risk_prob")
        if model_score.get("risk_penalty_bps") is not None:
            item["teacher_risk_penalty_bps"] = model_score.get("risk_penalty_bps")
        if model_score.get("score_source"):
            item["teacher_score_source"] = model_score.get("score_source")
    if rank_order is not None:
        item["rank_order"] = int(rank_order or 0)
    return item


def score_report_for_timestamp(score_report_by_timestamp, timestamp):
    report = score_report_by_timestamp.get(timestamp)
    if report is None:
        report = empty_score_report()
        score_report_by_timestamp[timestamp] = report
    return report


def record_score_skip(report, reason, count=1, detail=None):
    report["skipped_missing_score_count"] += count
    report["skipped_by_reason"][reason] = report["skipped_by_reason"].get(reason, 0) + count
    if detail:
        report.setdefault("score_error_detail", detail)


def record_missing_score_features(report, missing):
    record_score_skip(report, MODEL_FEATURES_MISSING)
    report.setdefault("missing_feature_sample", missing[:10])


def prepare_model_batch_items(items, bundle, params, report_for_item):
    feature_columns = bundle["feature_columns"]
    feature_column_set = bundle.get("feature_column_set")
    numeric_columns = bundle.get("numeric_feature_columns") or set()
    input_specs = bundle.get("feature_input_specs") or [
        (column, column in numeric_columns) for column in feature_columns
    ]
    fast_frame = should_use_fast_model_frame(params, bundle)
    complete = []
    model_rows = []
    for item in items:
        row = item["row"]
        if fast_frame:
            missing = missing_feature_columns(row, feature_columns, feature_column_set)
            model_row = row
        else:
            model_row, missing = model_input_values_with_missing(row, input_specs)
        if missing:
            record_missing_score_features(report_for_item(item), missing)
            continue
        complete.append(item)
        model_rows.append(model_row)
    return complete, model_rows, fast_frame


def score_prepared_model_batch(complete, model_rows, bundle, params, fast_frame, progress_callback=None):
    if should_parallel_model_batch_score(params, bundle, len(complete), fast_frame):
        def report_parallel_progress(done, total):
            if callable(progress_callback):
                assert total > 0, "model scoring progress total must be positive"
                progress_callback(
                    "model_scoring",
                    f"ML selector: scored {done}/{total} model candidate rows",
                    0.10 + 0.80 * (done / total),
                )

        return score_complete_model_items_parallel(
            complete,
            bundle,
            params,
            progress_callback=report_parallel_progress if callable(progress_callback) else None,
            include_components=True,
        )

    if callable(progress_callback):
        progress_callback(
            "model_scoring",
            f"ML selector: scoring {len(complete)} model candidate rows",
            0.10,
        )

    def report_serial_progress(done, total):
        if callable(progress_callback):
            assert total > 0, "model scoring progress total must be positive"
            progress_callback(
                "model_scoring",
                f"ML selector: scored {done}/{total} model candidate rows",
                0.10 + 0.80 * (done / total),
            )

    return score_model_items_serial_chunked_components(
        complete,
        model_rows,
        bundle,
        params,
        fast_frame,
        progress_callback=report_serial_progress if callable(progress_callback) else None,
    )


def record_unavailable_model_items(items, bundle, report_for_item):
    reason = str(bundle.get("status") or MODEL_ARTIFACT_MISSING)
    detail = bundle.get("detail")
    for item in items:
        record_score_skip(report_for_item(item), reason, detail=detail)


def record_model_score_failure(complete, error, report_for_item):
    for item in complete:
        record_score_skip(report_for_item(item), MODEL_SCORE_FAILED, detail=str(error))


def score_model_candidate_batch_items(
    items,
    bundle,
    params,
    report_for_item,
    append_ranked_item,
    progress_callback=None,
):
    if not bundle.get("available"):
        record_unavailable_model_items(items, bundle, report_for_item)
        return 0

    complete, model_rows, fast_frame = prepare_model_batch_items(
        items,
        bundle,
        params,
        report_for_item,
    )

    if not complete:
        return 0

    try:
        scores = score_prepared_model_batch(
            complete,
            model_rows,
            bundle,
            params,
            fast_frame,
            progress_callback=progress_callback,
        )
    except Exception as error:  # pragma: no cover - model/artifact specific.
        record_model_score_failure(complete, error, report_for_item)
        return 0

    context_penalty_bps = num(params.get("score_context_penalty_bps"), FROZEN_CANDIDATE["context_penalty_bps"])
    for item, model_score in zip(complete, scores):
        report = report_for_item(item)
        report["model_score_count"] += 1
        score = model_score.get("score") if isinstance(model_score, dict) else model_score
        append_ranked_item(
            item,
            ranked_item(
                item["row"],
                item["symbol"],
                item["side"],
                score,
                params,
                context_penalty_bps,
                rank_order=item.get("rank_order"),
                model_score=model_score,
            )
        )
    return len(complete)


def score_model_candidate_batch(items, bundle, params, score_report):
    selected = []
    score_model_candidate_batch_items(
        items,
        bundle,
        params,
        lambda _item: score_report,
        lambda _item, ranked: selected.append(ranked),
    )
    return selected


def score_model_candidate_batch_by_timestamp(
    items,
    bundle,
    params,
    score_report_by_timestamp,
    selected_by_timestamp,
    progress_callback=None,
):
    def append_for_timestamp(item, ranked):
        timestamp = item["timestamp"]
        selected = selected_by_timestamp.get(timestamp)
        if selected is None:
            selected = []
            selected_by_timestamp[timestamp] = selected
        selected.append(ranked)

    scored_count = score_model_candidate_batch_items(
        items,
        bundle,
        params,
        lambda item: score_report_for_timestamp(score_report_by_timestamp, item["timestamp"]),
        append_for_timestamp,
        progress_callback=progress_callback,
    )
    if callable(progress_callback) and scored_count:
        progress_callback(
            "model_scoring",
            f"ML selector: scored {scored_count}/{scored_count} model candidate rows",
            1.0,
        )

def generate_candidate_rows_from_context_for_timestamps(context, params, timestamps, progress_callback=None):
    external_rows = candidate_rows_from_context(context)
    if external_rows:
        return external_rows, {
            "candidate_source": EXTERNAL_CONTEXT_STATUS,
            "blocked_by": [],
            "summary": "Using explicit point-in-time candidate rows from context.",
        }
    blocker = runtime_candidate_model_blocker(params)
    if blocker is not None:
        return [], model_blocked_source_info(blocker)
    rows, generation = generate_runtime_candidate_rows_for_timestamps(
        context,
        UNIVERSE_SYMBOLS,
        params,
        timestamps,
        progress_callback=progress_callback,
    )
    if rows:
        return rows, {
            "candidate_source": RUNTIME_CANDIDATE_GENERATOR,
            "blocked_by": [],
            "summary": (
                "Runtime-local candidate generation produced promoted base-layer and "
                "universe_candidate_v1 rows. Research parity tests are still pending."
            ),
            "generation": generation,
        }
    status = generation.get("status", "no_runtime_candidate_signal")
    return [], {
        "candidate_source": "strategy_runtime_generator",
        "blocked_by": [status],
        "summary": (
            "Runtime-local candidate generation is implemented for the promoted base layers "
            "and universe_candidate_v1, but produced no current candidate rows."
        ),
        "generation": generation,
    }

def runtime_candidate_model_blocker(params):
    """Runtime-generated candidate rows do not carry explicit scores."""

    bundle = load_model_bundle(params)
    return None if bundle.get("available") else bundle

def model_blocked_source_info(bundle):
    status = str(bundle.get("status") or MODEL_ARTIFACT_MISSING)
    return {
        "candidate_source": "strategy_runtime_generator",
        "blocked_by": [status],
        "summary": "Runtime candidate generation skipped because generated rows require a loadable ML model.",
        "model_status": status,
        "model_path": bundle.get("model_path"),
        "contract_path": bundle.get("contract_path"),
        "detail": bundle.get("detail"),
    }
