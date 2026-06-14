use super::{super::*, queries::fetch_research_dataset};

pub(crate) async fn research_dataset_preview(
    state: &AppState,
    dataset_id: &str,
) -> AppResult<Value> {
    let Some(dataset) = fetch_research_dataset(state, dataset_id).await? else {
        return Err(AppError::Validation(
            "dataset manifest not found".to_string(),
        ));
    };
    Ok(json!({ "preview": build_dataset_preview(&dataset) }))
}

fn build_dataset_preview(dataset: &Value) -> Value {
    json!({
        "manifest": dataset,
        "protocol_validation_status": "valid",
        "split_summary": {
            "split_definition_version": value_string_at(dataset, "split_definition_version", ""),
            "train_sample_count": value_i64_at(dataset, "train_sample_count", 0),
            "val_sample_count": value_i64_at(dataset, "val_sample_count", 0),
            "test_sample_count": value_i64_at(dataset, "test_sample_count", 0)
        },
        "weight_summary": {
            "weighting_version": value_string_at(dataset, "weighting_version", ""),
            "weight_definition": value_string_at(dataset, "weight_definition", "")
        },
        "regime_schema": {
            "definition_version": value_string_at(dataset, "regime_definition_version", "boundary_regimes_v1"),
            "regimes": []
        },
        "n_eff_summary": {
            "train_effective_sample_size": value_i64_at(dataset, "train_effective_sample_size", 0),
            "val_effective_sample_size": value_i64_at(dataset, "val_effective_sample_size", 0),
            "test_effective_sample_size": value_i64_at(dataset, "test_effective_sample_size", 0),
            "sequence_definitions": []
        },
        "shift_diagnostic_preview": dataset.get("shift_diagnostic_result").cloned().unwrap_or_else(|| json!({})),
        "shift_diagnostics_bundle": {
            "shift_diagnostic_version": value_string_at(dataset, "shift_diagnostic_version", "")
        },
        "strata_fit_bundle": {
            "artifact_ref": value_string_at(dataset, "strata_fit_ref", ""),
            "strata_definition_version": value_string_at(dataset, "strata_definition_version", "")
        },
        "weight_fit_bundle": {
            "artifact_ref": value_string_at(dataset, "weight_fit_ref", ""),
            "weight_estimator_version": value_string_at(dataset, "weight_estimator_version", "")
        },
        "domain_classifier_fit_bundle": {
            "artifact_ref": value_string_at(dataset, "domain_classifier_fit_ref", ""),
            "domain_classifier_version": value_string_at(dataset, "domain_classifier_version", "")
        }
    })
}
