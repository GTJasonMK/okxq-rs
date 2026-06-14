use std::sync::Arc;

use crate::okx_outbound::{OKXOutboundEvent, OKXOutboundTimelineStore, OKXRateRuleRegistry};

pub(in crate::okx) struct OkxOutboundRecordDefaults<'a> {
    pub rule_key: &'a str,
    pub target_group: &'a str,
    pub mode: &'a str,
}

pub(in crate::okx) fn record_okx_outbound(
    timeline: &Option<Arc<OKXOutboundTimelineStore>>,
    registry: &Option<Arc<OKXRateRuleRegistry>>,
    op_key: &str,
    inst_id: Option<&str>,
    result: &str,
    latency_ms: i64,
    defaults: OkxOutboundRecordDefaults<'_>,
) {
    let Some(timeline) = timeline.as_ref() else {
        return;
    };

    let now_ts = chrono::Utc::now().timestamp_millis() as f64 / 1000.0;
    let rule = registry.as_ref().and_then(|registry| registry.get(op_key));
    let rule_key = rule
        .map(|item| item.rule_key.as_str())
        .unwrap_or(defaults.rule_key);
    let target_group = rule
        .map(|item| item.target_group.as_str())
        .unwrap_or(defaults.target_group);
    let channel = rule.map(|item| item.channel.as_str()).unwrap_or("rest");
    let inst_id = inst_id.unwrap_or("");

    timeline.record(OKXOutboundEvent {
        ts: now_ts,
        op_key: op_key.to_string(),
        channel: channel.to_string(),
        target_group: target_group.to_string(),
        rule_key: rule_key.to_string(),
        scope_key: outbound_scope_key(rule_key, inst_id),
        inst_id: inst_id.to_string(),
        mode: defaults.mode.to_string(),
        result: result.to_string(),
        latency_ms,
    });
}

fn outbound_scope_key(rule_key: &str, inst_id: &str) -> String {
    match rule_key {
        "public_ip_inst" | "trade_user_inst" => {
            format!("{}:{}", rule_key, inst_id.trim().to_uppercase())
        }
        _ => rule_key.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_uses_registry_inst_scope_key() {
        let timeline = Some(Arc::new(OKXOutboundTimelineStore::new(8)));
        let registry = Some(Arc::new(OKXRateRuleRegistry::new()));

        record_okx_outbound(
            &timeline,
            &registry,
            "market.funding_rate",
            Some("btc-usdt-swap"),
            "ok",
            12,
            OkxOutboundRecordDefaults {
                rule_key: "public_ip",
                target_group: "public",
                mode: "",
            },
        );

        let snapshot = timeline.as_ref().expect("timeline").snapshot(
            60,
            chrono::Utc::now().timestamp_millis() as f64 / 1000.0,
            8,
        );
        let event = &snapshot["events"][0];
        assert_eq!(event["rule_key"], "public_ip_inst");
        assert_eq!(event["scope_key"], "public_ip_inst:BTC-USDT-SWAP");
        assert_eq!(event["target_group"], "public");
        assert_eq!(event["mode"], "");
    }

    #[test]
    fn record_keeps_private_fallback_target_and_mode() {
        let timeline = Some(Arc::new(OKXOutboundTimelineStore::new(8)));
        let registry = Some(Arc::new(OKXRateRuleRegistry::new()));

        record_okx_outbound(
            &timeline,
            &registry,
            "trade.unknown_new_endpoint",
            Some("eth-usdt-swap"),
            "api_error",
            34,
            OkxOutboundRecordDefaults {
                rule_key: "private_user",
                target_group: "trade",
                mode: "sim",
            },
        );

        let snapshot = timeline.as_ref().expect("timeline").snapshot(
            60,
            chrono::Utc::now().timestamp_millis() as f64 / 1000.0,
            8,
        );
        let event = &snapshot["events"][0];
        assert_eq!(event["rule_key"], "private_user");
        assert_eq!(event["scope_key"], "private_user");
        assert_eq!(event["target_group"], "trade");
        assert_eq!(event["mode"], "sim");
    }
}
