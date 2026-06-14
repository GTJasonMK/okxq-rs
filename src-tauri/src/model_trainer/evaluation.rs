use serde_json::{json, Value};

use super::{round6, EvalMetrics, LinearModel};

/// 用模型预测特征矩阵，返回预测值。
fn predict(model: &LinearModel, x: &[Vec<f64>]) -> Vec<f64> {
    x.iter()
        .map(|row| {
            let mut pred = model.intercept;
            for (j, (&coef, &feat)) in model.coefficients.iter().zip(row.iter()).enumerate() {
                let normalized = (feat - model.feature_means[j]) / model.feature_stds[j];
                pred += coef * normalized;
            }
            pred * model.label_std + model.label_mean
        })
        .collect()
}

/// 评估模型在数据集上的表现。
pub(super) fn evaluate(
    model: &LinearModel,
    x: &[Vec<f64>],
    y: &[f64],
) -> Result<EvalMetrics, String> {
    validate_inputs(model, x, y)?;
    let preds = predict(model, x);
    let n = preds.len();

    let y_mean = y.iter().sum::<f64>() / n as f64;
    let ss_res: f64 = preds
        .iter()
        .zip(y.iter())
        .map(|(p, t)| (t - p).powi(2))
        .sum();
    let ss_tot: f64 = y.iter().map(|t| (t - y_mean).powi(2)).sum();
    let mse = ss_res / n as f64;
    let mae = preds
        .iter()
        .zip(y.iter())
        .map(|(p, t)| (t - p).abs())
        .sum::<f64>()
        / n as f64;
    let r_squared = if ss_tot > 0.0 {
        1.0 - ss_res / ss_tot
    } else {
        0.0
    };
    let direction_accuracy = {
        let correct = preds
            .iter()
            .zip(y.iter())
            .filter(|(&p, &t)| (p > 0.0) == (t > 0.0))
            .count();
        correct as f64 / n as f64
    };

    Ok(EvalMetrics {
        mse: round6(mse),
        mae: round6(mae),
        r_squared: round6(r_squared),
        direction_accuracy: round6(direction_accuracy),
    })
}

pub(super) fn metrics_to_json(metrics: &EvalMetrics) -> Value {
    json!({
        "mse": metrics.mse,
        "mae": metrics.mae,
        "r_squared": metrics.r_squared,
        "direction_accuracy": metrics.direction_accuracy,
    })
}

fn validate_inputs(model: &LinearModel, x: &[Vec<f64>], y: &[f64]) -> Result<(), String> {
    if x.is_empty() {
        return Err("评估数据为空".to_string());
    }
    if x.len() != y.len() {
        return Err(format!(
            "评估特征和标签行数不一致：{} vs {}",
            x.len(),
            y.len()
        ));
    }
    let n_features = model.coefficients.len();
    if model.feature_means.len() != n_features || model.feature_stds.len() != n_features {
        return Err("模型特征统计维度不匹配".to_string());
    }
    if let Some((index, row)) = x
        .iter()
        .enumerate()
        .find(|(_, row)| row.len() != n_features)
    {
        return Err(format!(
            "评估特征维度不匹配：第{index}行期望{n_features}列，实际{}列",
            row.len()
        ));
    }
    Ok(())
}
