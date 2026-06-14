use serde_json::json;

use crate::commands::local_api::LocalApiRequest;

use super::{
    quote::manual_quote_from_orderbook, types::ManualCostEvidenceRequest,
    values::evidence_client_order_id,
};

#[test]
fn cost_evidence_order_requires_strategy_and_run_scope() {
    let req = LocalApiRequest {
        method: "POST".to_string(),
        path: "/api/trading/order".to_string(),
        params: Default::default(),
        body: json!({
            "record_cost_evidence": true,
            "strategy_id": "spread_velocity_v1",
        }),
    };

    let error = ManualCostEvidenceRequest::from_request(&req, "okxq_test")
        .expect_err("missing run_id must fail")
        .to_string();

    assert!(error.contains("strategy_id"));
    assert!(error.contains("run_id"));
}

#[test]
fn generated_cost_evidence_client_order_id_is_okx_compatible() {
    let generated = evidence_client_order_id("");

    assert!(generated.starts_with("okxq"));
    assert!(generated.len() <= 32);
    assert!(generated.chars().all(|item| item.is_ascii_alphanumeric()));
    assert_eq!(evidence_client_order_id("manual1"), "manual1");
}

#[test]
fn manual_arrival_quote_prefers_orderbook_bid_ask_mid_and_timestamp() {
    let quote = manual_quote_from_orderbook(&json!({
        "best_bid": 99.0,
        "best_ask": 101.0,
        "mid_price": 100.0,
        "ts": "1780000000123"
    }))
    .expect("complete orderbook quote");

    assert_eq!(quote.ts_ms, Some(1780000000123));
    assert_eq!(quote.mid_px, Some(100.0));
    assert_eq!(quote.bid_px, Some(99.0));
    assert_eq!(quote.ask_px, Some(101.0));
}
