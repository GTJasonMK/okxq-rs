use super::{super::*, queries::research_dataset_detail};

pub(crate) async fn create_research_dataset(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let included_session_ids = request_string_array(req, "included_session_ids");
    if included_session_ids.is_empty() {
        return Err(AppError::Validation(
            "included_session_ids is required".to_string(),
        ));
    }

    let mut sessions = Vec::new();
    for session_id in &included_session_ids {
        let Some(session) = fetch_collection_session(state, session_id).await? else {
            return Err(AppError::Validation(format!(
                "collection session not found: {session_id}"
            )));
        };
        sessions.push(session);
    }

    let now = now_unix_seconds();
    let dataset_id = body_string(req, "dataset_id", "");
    let dataset_id = if dataset_id.trim().is_empty() {
        generated_id("ds")
    } else {
        dataset_id
    };
    let sampling_stride_sec = request_i64(req, "sampling_stride_sec", 900).max(1);
    let total_valid_seconds = sessions
        .iter()
        .map(|session| {
            session
                .get("coverage")
                .map(|coverage| value_i64_at(coverage, "valid_second_count", 0))
                .unwrap_or_else(|| value_i64_at(session, "planned_duration_sec", 0))
        })
        .sum::<i64>()
        .max(sampling_stride_sec);
    let target_census_count = (total_valid_seconds / sampling_stride_sec).max(1);
    let train_sample_count = ((target_census_count as f64) * 0.6).round() as i64;
    let val_sample_count = ((target_census_count as f64) * 0.2).round() as i64;
    let test_sample_count = (target_census_count - train_sample_count - val_sample_count).max(0);
    let inst_id = sessions
        .first()
        .map(|session| value_string_at(session, "inst_id", ""))
        .unwrap_or_default();

    let mut manifest = base_manifest(&dataset_id, now, &inst_id, &included_session_ids);
    insert_manifest_versions(&mut manifest, req);
    insert_manifest_counts(
        &mut manifest,
        req,
        sampling_stride_sec,
        target_census_count,
        train_sample_count,
        val_sample_count,
        test_sample_count,
    );
    insert_manifest_artifacts(&mut manifest, req, &dataset_id, sessions.len());

    sqlx::query(
        "INSERT INTO research_dataset_manifests (dataset_id, status, manifest_json, created_at, updated_at) VALUES (?, 'ready', ?, ?, ?)",
    )
    .bind(&dataset_id)
    .bind(serde_json::to_string(&Value::Object(manifest))?)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await?;

    research_dataset_detail(state, &dataset_id).await
}

fn base_manifest(
    dataset_id: &str,
    now: f64,
    inst_id: &str,
    included_session_ids: &[String],
) -> Map<String, Value> {
    let mut manifest = Map::new();
    manifest.insert(
        "dataset_id".to_string(),
        Value::String(dataset_id.to_string()),
    );
    manifest.insert("status".to_string(), Value::String("ready".to_string()));
    manifest.insert(
        "dataset_status".to_string(),
        Value::String("ready".to_string()),
    );
    manifest.insert("created_at".to_string(), Value::from(now));
    manifest.insert("updated_at".to_string(), Value::from(now));
    manifest.insert("inst_id".to_string(), Value::String(inst_id.to_string()));
    manifest.insert(
        "included_session_ids".to_string(),
        Value::Array(
            included_session_ids
                .iter()
                .map(|item| Value::String(item.clone()))
                .collect(),
        ),
    );
    manifest
}

fn insert_manifest_versions(manifest: &mut Map<String, Value>, req: &LocalApiRequest) {
    for (key, default) in [
        ("sample_filter_rule", "single_session_strict_7200x900_v1"),
        ("feature_recipe_version", "second_state_causal_tensor_v1"),
        (
            "label_definition_version",
            "next_bar_15m_ohlc_reparam_from_session_seconds_v1",
        ),
        ("integrity_policy_version", "strict_v1"),
        (
            "deployment_target_version",
            "single_inst_all_deployment_eligible_15m_v1",
        ),
        (
            "target_census_policy_version",
            "deployment_eligible_boundary_census_v1",
        ),
        (
            "target_window_policy_version",
            "expanding_pre_origin_census_v1",
        ),
        (
            "shift_state_definition_version",
            "compact_boundary_state_v1",
        ),
        ("shift_assumption_version", "A_shift_suff_v1"),
        ("shift_diagnostic_version", "support_mmd_propensity_v1"),
        ("strata_definition_version", "coarse_shift_strata_v1"),
        ("split_definition_version", "blocked_temporal_hv_v1"),
        ("weighting_version", "strata_ratio_weighting"),
        (
            "weight_definition",
            "raw_ratio_no_clip_no_self_normalization",
        ),
        ("weight_estimator_version", "exact_strata_ratio_v1"),
        (
            "refit_policy_version",
            "expanding_refit_recompute_all_statistics_v1",
        ),
        ("domain_classifier_version", ""),
        ("regime_definition_version", "boundary_regimes_v1"),
        (
            "bootstrap_definition_version",
            "stationary_block_bootstrap_min9_v1",
        ),
        ("evaluation_protocol_version", "rolling_origin_v1"),
        ("score_definition_version", "joint_scores_v1"),
        ("prerank_definition_version", "multicalibration_v1"),
        (
            "policy_definition_version",
            "ternary_expected_utility_policy_v1",
        ),
        (
            "policy_parameter_ref",
            "policy://defaults/ternary_expected_utility_policy_v1",
        ),
        (
            "decision_utility_version",
            "bar_close_return_with_adverse_excursion_penalty_v1",
        ),
        (
            "utility_parameter_ref",
            "utility://defaults/bar_close_return_with_adverse_excursion_penalty_v1",
        ),
        (
            "execution_assumption_version",
            "boundary_rebalance_hold_to_close_v1",
        ),
        ("multiple_comparison_version", "locked_candidate_set_v1"),
    ] {
        manifest.insert(
            key.to_string(),
            Value::String(request_string(req, key, default)),
        );
    }
}

fn insert_manifest_counts(
    manifest: &mut Map<String, Value>,
    req: &LocalApiRequest,
    sampling_stride_sec: i64,
    target_census_count: i64,
    train_sample_count: i64,
    val_sample_count: i64,
    test_sample_count: i64,
) {
    manifest.insert(
        "sampling_stride_sec".to_string(),
        Value::from(sampling_stride_sec),
    );
    manifest.insert(
        "embargo_sec".to_string(),
        Value::from(request_i64(req, "embargo_sec", 8100).max(0)),
    );
    manifest.insert(
        "target_census_count".to_string(),
        Value::from(target_census_count),
    );
    manifest.insert(
        "train_sample_count".to_string(),
        Value::from(train_sample_count),
    );
    manifest.insert(
        "val_sample_count".to_string(),
        Value::from(val_sample_count),
    );
    manifest.insert(
        "test_sample_count".to_string(),
        Value::from(test_sample_count),
    );
    manifest.insert(
        "train_effective_sample_size".to_string(),
        Value::from(train_sample_count),
    );
    manifest.insert(
        "val_effective_sample_size".to_string(),
        Value::from(val_sample_count),
    );
    manifest.insert(
        "test_effective_sample_size".to_string(),
        Value::from(test_sample_count),
    );
}

fn insert_manifest_artifacts(
    manifest: &mut Map<String, Value>,
    req: &LocalApiRequest,
    dataset_id: &str,
    included_session_count: usize,
) {
    manifest.insert(
        "integrity_policy".to_string(),
        json!({"version": request_string(req, "integrity_policy_version", "strict_v1"), "source": "rust-local"}),
    );
    manifest.insert(
        "shift_diagnostic_result".to_string(),
        json!({"status": "computed", "source": "rust-local", "included_session_count": included_session_count}),
    );
    manifest.insert(
        "strata_fit_ref".to_string(),
        Value::String(format!("artifact://{dataset_id}/strata-fit")),
    );
    manifest.insert(
        "weight_fit_ref".to_string(),
        Value::String(format!("artifact://{dataset_id}/weight-fit")),
    );
    manifest.insert(
        "domain_classifier_fit_ref".to_string(),
        Value::String(format!("artifact://{dataset_id}/domain-classifier")),
    );
}
