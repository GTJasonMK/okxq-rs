use serde_json::Value;

use super::{super::super::super::*, types::ManualArrivalQuote, values::*};

pub(in crate::commands::local_api::trading::actions) async fn fetch_manual_arrival_quote(
    state: &AppState,
    inst_id: &str,
) -> ManualArrivalQuote {
    let Ok(client) = okx_client(state).await else {
        return ManualArrivalQuote::default();
    };
    if let Ok(book) = client.get_orderbook(inst_id, 1).await {
        if let Some(quote) = manual_quote_from_orderbook(&book) {
            return quote;
        }
    }
    if let Ok(ticker) = client.get_ticker(inst_id).await {
        if let Some(quote) = manual_quote_from_ticker(&ticker) {
            return quote;
        }
    }
    ManualArrivalQuote::default()
}

pub(in crate::commands::local_api::trading::actions::cost_evidence) fn manual_quote_from_orderbook(
    book: &Value,
) -> Option<ManualArrivalQuote> {
    let bid = positive_f64(book.get("best_bid"))?;
    let ask = positive_f64(book.get("best_ask"))?;
    Some(ManualArrivalQuote {
        ts_ms: positive_i64(book.get("ts")).or_else(now_ms),
        mid_px: Some(positive_f64(book.get("mid_price")).unwrap_or((bid + ask) / 2.0)),
        bid_px: Some(bid),
        ask_px: Some(ask),
    })
}

fn manual_quote_from_ticker(ticker: &Value) -> Option<ManualArrivalQuote> {
    let bid = positive_f64(ticker.get("bidPx").or_else(|| ticker.get("bid")))?;
    let ask = positive_f64(ticker.get("askPx").or_else(|| ticker.get("ask")))?;
    Some(ManualArrivalQuote {
        ts_ms: positive_i64(ticker.get("ts")).or_else(now_ms),
        mid_px: Some((bid + ask) / 2.0),
        bid_px: Some(bid),
        ask_px: Some(ask),
    })
}
