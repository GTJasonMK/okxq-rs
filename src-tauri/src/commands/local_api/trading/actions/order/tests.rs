use serde_json::json;

use super::params::{normalize_order_inst_id, normalize_order_inst_type, resolve_order_pos_side};

#[test]
fn order_pos_side_is_omitted_for_swap_net_mode() {
    let config = json!({"posMode": "net_mode"});

    let pos_side = resolve_order_pos_side("SWAP", "long", Some(&config)).unwrap();

    assert_eq!(pos_side, "");
}

#[test]
fn order_pos_side_is_kept_for_swap_long_short_mode() {
    let config = json!({"posMode": "long_short_mode"});

    let pos_side = resolve_order_pos_side("SWAP", "short", Some(&config)).unwrap();

    assert_eq!(pos_side, "short");
}

#[test]
fn order_pos_side_is_required_for_swap_long_short_mode() {
    let config = json!({"posMode": "long_short_mode"});

    let error = resolve_order_pos_side("SWAP", "", Some(&config))
        .expect_err("missing pos_side must fail in long_short_mode")
        .to_string();

    assert!(error.contains("pos_side=long/short"));
}

#[test]
fn order_pos_side_is_omitted_for_spot() {
    let config = json!({"posMode": "long_short_mode"});

    let pos_side = resolve_order_pos_side("SPOT", "long", Some(&config)).unwrap();

    assert_eq!(pos_side, "");
}

#[test]
fn order_pos_side_requires_known_position_mode_for_swap() {
    let error = resolve_order_pos_side("SWAP", "", Some(&json!({})))
        .expect_err("unknown position mode must fail")
        .to_string();

    assert!(error.contains("posMode"));
}

#[test]
fn order_symbol_uses_requested_swap_type() {
    assert_eq!(normalize_order_inst_type("SWAP", "BTC-USDT"), "SWAP");
    assert_eq!(normalize_order_inst_id("BTC-USDT", "SWAP"), "BTC-USDT-SWAP");
}
