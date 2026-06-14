use serde_json::{json, Value};

/// 蒙特卡洛模拟配置
#[derive(Clone, Debug)]
pub struct MonteCarloConfig {
    /// 模拟次数（默认1000）
    pub num_simulations: usize,
    /// 块自助法的连续交易块长度；1 等价于 IID 单笔重采样。
    pub block_size: usize,
    /// 置信水平（默认 [0.05, 0.25, 0.50, 0.75, 0.95]）
    pub confidence_levels: Vec<f64>,
    /// 随机种子（可选）
    pub seed: Option<u64>,
}

impl Default for MonteCarloConfig {
    fn default() -> Self {
        Self {
            num_simulations: 1000,
            block_size: 5,
            confidence_levels: vec![0.05, 0.25, 0.50, 0.75, 0.95],
            seed: None,
        }
    }
}

/// 蒙特卡洛模拟结果
#[derive(Clone, Debug)]
pub struct MonteCarloResult {
    pub num_simulations: usize,
    pub sampling_method: String,
    pub block_size: usize,
    pub original_final_equity: f64,
    pub original_max_drawdown: f64,
    pub equity_percentiles: Vec<(String, f64)>,
    pub drawdown_percentiles: Vec<(String, f64)>,
    pub mean_final_equity: f64,
    pub std_final_equity: f64,
    pub median_final_equity: f64,
    pub prob_profit: f64,
    pub prob_original_beat: f64,
    pub worst_case_equity: f64,
    pub best_case_equity: f64,
}

impl MonteCarloResult {
    pub fn to_value(&self) -> Value {
        let equity_pct: Value = self
            .equity_percentiles
            .iter()
            .map(|(k, v)| json!({k: v}))
            .collect();
        let dd_pct: Value = self
            .drawdown_percentiles
            .iter()
            .map(|(k, v)| json!({k: v}))
            .collect();
        json!({
            "num_simulations": self.num_simulations,
            "sampling_method": self.sampling_method,
            "block_size": self.block_size,
            "original_final_equity": self.original_final_equity,
            "original_max_drawdown": self.original_max_drawdown,
            "equity_percentiles": equity_pct,
            "drawdown_percentiles": dd_pct,
            "mean_final_equity": self.mean_final_equity,
            "std_final_equity": self.std_final_equity,
            "median_final_equity": self.median_final_equity,
            "prob_profit": self.prob_profit,
            "prob_original_beat": self.prob_original_beat,
            "worst_case_equity": self.worst_case_equity,
            "best_case_equity": self.best_case_equity,
        })
    }
}
