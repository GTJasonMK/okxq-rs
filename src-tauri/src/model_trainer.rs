//! 模型训练管线 — 从数据集切分加载训练数据，训练线性模型，评估并存储结果。
//!
//! 支持：
//!   - 普通最小二乘法 (OLS) 线性回归，通过正规方程求解
//!   - Z-score 特征标准化
//!   - 训练/验证/测试评估 (MSE / MAE / R² / 方向准确率)
//!   - 模型权重持久化到 research_training_runs

use serde_json::{json, Value};
use sqlx::SqlitePool;

mod data;
mod evaluation;
mod linalg;
mod types;

use self::data::load_split_data;
use self::evaluation::{evaluate, metrics_to_json};
use self::linalg::solve_normal_equation;
pub use self::types::{EvalMetrics, LinearModel};

const FEATURE_NAMES: [&str; 15] = [
    "open",
    "high",
    "low",
    "close",
    "volume",
    "mid_price",
    "ret_1",
    "ret_5",
    "ret_20",
    "ma5_dev",
    "ma20_dev",
    "vol_5",
    "vol_20",
    "vol_ratio",
    "high_low_range",
];

/// 训练 OLS 线性模型。
///
/// 步骤：
///   1. 加载 train + val 数据
///   2. Z-score 标准化特征和标签
///   3. 通过正规方程 w = (X^T X)^{-1} X^T y 求解权重
///   4. 在 train、val、test 上评估
///   5. 返回模型 + 评估结果
pub async fn train_model(
    db: &SqlitePool,
    dataset_id: &str,
    training_seed: i64,
) -> Result<Value, String> {
    let (train_x, train_y, _train_ts) = load_split_data(db, dataset_id, "train").await?;
    let (val_x, val_y, _val_ts) = load_split_data(db, dataset_id, "val").await?;
    let (test_x, test_y, _test_ts) = load_split_data(db, dataset_id, "test").await?;

    if train_x.len() < 10 {
        return Err("训练样本不足（需至少10条）".to_string());
    }

    let n_features = train_x[0].len();
    if n_features != FEATURE_NAMES.len() {
        return Err(format!(
            "训练特征维度不匹配：期望{}列，实际{}列",
            FEATURE_NAMES.len(),
            n_features
        ));
    }
    let (means, stds) = feature_scaling_stats(&train_x, n_features);
    let (label_mean, label_std) = label_scaling_stats(&train_y);
    let (x_scaled, y_scaled) = scale_training_data(
        &train_x, &train_y, &means, &stds, label_mean, label_std, n_features,
    );

    let weights = solve_normal_equation(&x_scaled, &y_scaled, n_features + 1)?;
    let intercept = weights[0];
    let coefficients = weights[1..].to_vec();
    validate_finite_values("模型权重", &weights)?;

    let model = LinearModel {
        coefficients: coefficients.clone(),
        intercept,
        feature_means: means,
        feature_stds: stds,
        label_mean,
        label_std,
    };

    let train_eval = evaluate(&model, &train_x, &train_y)?;
    let val_eval = evaluate(&model, &val_x, &val_y)?;
    let test_eval = evaluate(&model, &test_x, &test_y)?;
    validate_metrics("train", &train_eval)?;
    validate_metrics("val", &val_eval)?;
    validate_metrics("test", &test_eval)?;

    let now = chrono::Utc::now().timestamp_millis() as f64 / 1000.0;
    let run_id = format!("tr_{}", chrono::Utc::now().timestamp_millis());
    let n_train = train_x.len();

    let feature_importance: Vec<Value> = coefficients
        .iter()
        .enumerate()
        .map(|(i, &coef)| {
            json!({
                "feature": FEATURE_NAMES[i],
                "coefficient": round6(coef),
                "abs_importance": round6(coef.abs()),
            })
        })
        .collect();

    let result = json!({
        "run_id": run_id,
        "dataset_id": dataset_id,
        "model_family": "linear_regression_v1",
        "status": "completed",
        "progress_stage": "completed",
        "training_seed": training_seed,
        "n_features": n_features,
        "n_train": n_train,
        "n_val": val_x.len(),
        "n_test": test_x.len(),
        "training": {
            "algorithm": "ordinary_least_squares",
            "normalization": "z_score",
            "label": "label_1m_log_return",
        },
        "model": {
            "intercept": round6(intercept),
            "coefficients": coefficients.iter().map(|c| round6(*c)).collect::<Vec<_>>(),
            "feature_importance": feature_importance,
        },
        "metrics": {
            "train": metrics_to_json(&train_eval),
            "val": metrics_to_json(&val_eval),
            "test": metrics_to_json(&test_eval),
        },
        "best_metric": {
            "val_r_squared": round6(val_eval.r_squared),
            "val_direction_accuracy": round6(val_eval.direction_accuracy),
        },
        "created_at": now,
        "updated_at": now,
    });

    let payload_json =
        serde_json::to_string(&result).map_err(|e| format!("序列化训练记录失败: {e}"))?;
    sqlx::query(
        "INSERT OR REPLACE INTO research_training_runs (run_id, dataset_id, status, progress_stage, payload_json, created_at, updated_at) VALUES (?, ?, 'completed', 'completed', ?, ?, ?)",
    )
    .bind(&run_id)
    .bind(dataset_id)
    .bind(&payload_json)
    .bind(now)
    .bind(now)
    .execute(db)
    .await
    .map_err(|e| format!("写入训练记录失败: {e}"))?;

    Ok(result)
}

fn feature_scaling_stats(train_x: &[Vec<f64>], n_features: usize) -> (Vec<f64>, Vec<f64>) {
    let mut means = vec![0.0; n_features];
    let mut stds = vec![1.0; n_features];
    for j in 0..n_features {
        let sum: f64 = train_x.iter().map(|row| row[j]).sum();
        means[j] = sum / train_x.len() as f64;
        let var: f64 = train_x
            .iter()
            .map(|row| (row[j] - means[j]).powi(2))
            .sum::<f64>()
            / train_x.len() as f64;
        stds[j] = var.sqrt().max(1e-10);
    }
    (means, stds)
}

fn label_scaling_stats(train_y: &[f64]) -> (f64, f64) {
    let label_mean = train_y.iter().sum::<f64>() / train_y.len() as f64;
    let label_var = train_y
        .iter()
        .map(|value| (value - label_mean).powi(2))
        .sum::<f64>()
        / train_y.len() as f64;
    (label_mean, label_var.sqrt().max(1e-10))
}

fn scale_training_data(
    train_x: &[Vec<f64>],
    train_y: &[f64],
    means: &[f64],
    stds: &[f64],
    label_mean: f64,
    label_std: f64,
    n_features: usize,
) -> (Vec<Vec<f64>>, Vec<f64>) {
    let n_train = train_x.len();
    let mut x_scaled = vec![vec![0.0; n_features + 1]; n_train];
    let mut y_scaled = vec![0.0; n_train];
    for i in 0..n_train {
        x_scaled[i][0] = 1.0;
        for j in 0..n_features {
            x_scaled[i][j + 1] = (train_x[i][j] - means[j]) / stds[j];
        }
        y_scaled[i] = (train_y[i] - label_mean) / label_std;
    }
    (x_scaled, y_scaled)
}

fn validate_finite_values(name: &str, values: &[f64]) -> Result<(), String> {
    if values.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        Err(format!("{name} 包含非有限数字"))
    }
}

fn validate_metrics(split: &str, metrics: &EvalMetrics) -> Result<(), String> {
    let values = [
        metrics.mse,
        metrics.mae,
        metrics.r_squared,
        metrics.direction_accuracy,
    ];
    validate_finite_values(&format!("{split} 评估指标"), &values)
}

pub(super) fn round6(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}
