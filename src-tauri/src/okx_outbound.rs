// OKX 出站流量限流与时间线模块
// 基于官方限流规则记录每次 API 调用，生成时间线快照

use serde::Serialize;

mod registry;
mod timeline;

pub use self::registry::OKXRateRuleRegistry;
pub use self::timeline::OKXOutboundTimelineStore;

/// 限流规则定义
#[derive(Debug, Clone, Serialize)]
pub struct OKXOutboundRule {
    pub op_key: String,
    pub rule_key: String,
    pub channel: String,
    pub target_group: String,
    pub window_seconds: i64,
    pub capacity: i64,
}

/// 单次出站事件
#[derive(Debug, Clone, Serialize)]
pub struct OKXOutboundEvent {
    pub ts: f64,
    pub op_key: String,
    pub channel: String,
    pub target_group: String,
    pub rule_key: String,
    pub scope_key: String,
    #[serde(default)]
    pub inst_id: String,
    #[serde(default)]
    pub mode: String,
    pub result: String,
    #[serde(default)]
    pub latency_ms: i64,
}
