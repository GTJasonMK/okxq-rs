use super::super::super::super::*;
use crate::trading_fills::{
    lookup_arrival_evidence_for_symbol, okx_text, upsert_local_fill, UpsertLocalFillRequest,
};

pub(crate) async fn sync_local_fills(state: &AppState, req: &LocalApiRequest) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let inst_type = request_string(req, "inst_type", "SWAP");
    let inst_id = request_string(req, "inst_id", "");
    let limit = request_i64(req, "limit", 100).clamp(1, 100) as u32;
    let after = request_string(req, "after", "");
    let before = request_string(req, "before", "");
    let client = okx_private_client(state, &mode).await?;
    let items = client
        .get_fills_history(&inst_type, &inst_id, limit, &after, &before)
        .await?;

    let mut stored = 0_i64;
    let mut skipped_missing_trade_id = 0_i64;
    let mut arrival_matched = 0_i64;
    for item in &items {
        let trade_id = okx_text(item, &["tradeId", "trade_id"]);
        if trade_id.is_empty() {
            skipped_missing_trade_id += 1;
            continue;
        }
        let inst_id = okx_text(item, &["instId", "inst_id"]);
        if inst_id.is_empty() {
            skipped_missing_trade_id += 1;
            continue;
        }
        let order_id = okx_text(item, &["ordId", "ord_id"]);
        let client_order_id = okx_text(item, &["clOrdId", "cl_ord_id", "client_order_id"]);
        let arrival = lookup_arrival_evidence_for_symbol(
            &state.db,
            &mode,
            &inst_id,
            &order_id,
            &client_order_id,
        )
        .await?;
        if arrival.has_complete_arrival_quote() {
            arrival_matched += 1;
        }
        upsert_local_fill(UpsertLocalFillRequest {
            db: &state.db,
            mode: &mode,
            trade_id: &trade_id,
            inst_id: &inst_id,
            item,
            order_id: &order_id,
            client_order_id: &client_order_id,
            arrival: &arrival,
        })
        .await?;
        stored += 1;
    }

    Ok(json!({
        "mode": mode,
        "inst_type": inst_type,
        "inst_id": inst_id,
        "fetched": items.len(),
        "stored": stored,
        "skipped_missing_trade_id": skipped_missing_trade_id,
        "arrival_matched": arrival_matched,
        "note": "Synced OKX fills into local_fills. Slippage evidence is ready only for rows with arrival_mid_px/arrival_bid_px/arrival_ask_px."
    }))
}
