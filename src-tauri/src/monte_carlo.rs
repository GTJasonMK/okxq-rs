//! 蒙特卡洛模拟 — 基于交易盈亏重采样的策略稳健性评估。
//!
//! 通过对历史交易盈亏序列进行块自助法（block bootstrap）重采样，生成大量
//! 可能的权益曲线，计算分位数和概率分布。`block_size=1` 时退化为 IID 单笔重采样。

mod rng;
mod simulation;
mod types;

pub use simulation::run_monte_carlo;
pub use types::MonteCarloConfig;
