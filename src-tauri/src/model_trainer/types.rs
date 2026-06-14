/// OLS 线性回归模型。
#[derive(Clone, Debug)]
pub struct LinearModel {
    pub coefficients: Vec<f64>,
    pub intercept: f64,
    pub feature_means: Vec<f64>,
    pub feature_stds: Vec<f64>,
    pub label_mean: f64,
    pub label_std: f64,
}

/// 评估指标。
#[derive(Clone, Debug)]
pub struct EvalMetrics {
    pub mse: f64,
    pub mae: f64,
    pub r_squared: f64,
    pub direction_accuracy: f64,
}
