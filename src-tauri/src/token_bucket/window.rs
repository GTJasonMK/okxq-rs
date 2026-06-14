use std::{collections::VecDeque, time::Instant};

use crate::okx_outbound::OKXOutboundRule;

#[derive(Clone)]
pub(super) struct RateWindowRule {
    pub(super) op_key: String,
    pub(super) rule_key: String,
    pub(super) channel: String,
    pub(super) target_group: String,
    pub(super) window_seconds: i64,
    pub(super) capacity: i64,
}

impl From<&OKXOutboundRule> for RateWindowRule {
    fn from(rule: &OKXOutboundRule) -> Self {
        Self {
            op_key: rule.op_key.clone(),
            rule_key: rule.rule_key.clone(),
            channel: rule.channel.clone(),
            target_group: rule.target_group.clone(),
            window_seconds: rule.window_seconds.max(1),
            capacity: rule.capacity.max(1),
        }
    }
}

#[derive(Default)]
pub(super) struct RateWindowState {
    pub(super) calls: VecDeque<Instant>,
}

pub(super) fn bucket_key(rule: &RateWindowRule, inst_id: Option<&str>) -> String {
    if rule.rule_key == "public_ip_inst" || rule.rule_key == "trade_user_inst" {
        let normalized_inst_id = inst_id.unwrap_or("").trim().to_uppercase();
        return format!("{}|{}:{}", rule.op_key, rule.rule_key, normalized_inst_id);
    }
    format!("{}|{}", rule.op_key, rule.rule_key)
}
