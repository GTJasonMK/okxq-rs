use super::super::*;
use super::fills::local_fill_rows;

pub(crate) async fn trade_performance(state: &AppState, req: &LocalApiRequest) -> AppResult<Value> {
    let items = local_fill_rows(state, req).await?;
    Ok(Value::Array(vec![summarize_trade_performance(&items)]))
}

fn summarize_trade_performance(items: &[Value]) -> Value {
    let total_trades = items.len();
    let inst_id = if total_trades > 0 { "ALL" } else { "" };
    json!({
        "inst_id": inst_id,
        "total_trades": total_trades,
        "win_rate": null,
        "total_pnl": null,
        "profit_factor": null,
        "largest_win": null,
        "largest_loss": null
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trade_performance_does_not_fabricate_profitability_metrics_without_pnl_evidence() {
        let summary = summarize_trade_performance(&[
            json!({
                "trade_id": "fill-1",
                "inst_id": "BTC-USDT-SWAP",
                "side": "buy",
                "fill_px": 70000.0,
                "fill_sz": 0.01,
                "fee": 0.1
            }),
            json!({
                "trade_id": "fill-2",
                "inst_id": "BTC-USDT-SWAP",
                "side": "sell",
                "fill_px": 70100.0,
                "fill_sz": 0.01,
                "fee": 0.1
            }),
        ]);

        assert_eq!(summary["inst_id"], "ALL");
        assert_eq!(summary["total_trades"], 2);
        assert!(summary["win_rate"].is_null());
        assert!(summary["total_pnl"].is_null());
        assert!(summary["profit_factor"].is_null());
        assert!(summary["largest_win"].is_null());
        assert!(summary["largest_loss"].is_null());
    }
}
