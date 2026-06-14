pub(crate) fn infer_spot_swap_inst_type(inst_id: &str) -> &'static str {
    if inst_id.trim().to_uppercase().ends_with("-SWAP") {
        "SWAP"
    } else {
        "SPOT"
    }
}

pub(crate) fn infer_okx_inst_type(inst_id: &str) -> &'static str {
    let inst_id = inst_id.trim().to_uppercase();
    if inst_id.ends_with("-SWAP") {
        "SWAP"
    } else if inst_id.ends_with("-FUTURES") {
        "FUTURES"
    } else {
        "SPOT"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inst_type_inference_keeps_spot_swap_and_tick_collector_contracts() {
        assert_eq!(infer_spot_swap_inst_type("btc-usdt-swap"), "SWAP");
        assert_eq!(infer_spot_swap_inst_type("btc-usdt-futures"), "SPOT");
        assert_eq!(infer_spot_swap_inst_type("btc-usdt"), "SPOT");

        assert_eq!(infer_okx_inst_type("btc-usdt-swap"), "SWAP");
        assert_eq!(infer_okx_inst_type("btc-usdt-futures"), "FUTURES");
        assert_eq!(infer_okx_inst_type("btc-usdt"), "SPOT");
    }
}
