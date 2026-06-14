use super::{
    super::super::super::*,
    types::{ManualArrivalQuote, ManualCostEvidenceRequest},
};

#[allow(clippy::too_many_arguments)]
pub(in crate::commands::local_api::trading::actions) async fn insert_manual_cost_order_record(
    state: &AppState,
    mode: &str,
    inst_id: &str,
    inst_type: &str,
    side: &str,
    order_type: &str,
    size: Option<f64>,
    price: Option<f64>,
    order_id: &str,
    client_order_id: &str,
    evidence: &ManualCostEvidenceRequest,
    arrival: &ManualArrivalQuote,
) -> AppResult<i64> {
    let now = chrono::Utc::now().to_rfc3339();
    let result = sqlx::query(
        r#"
        INSERT INTO live_order_records (
          strategy_id, strategy_name, symbol, inst_id, inst_type, side, order_type,
          size, price, order_id, client_order_id, status, action,
          error_message, mode, success, run_id, action_timestamp, arrival_ts,
          arrival_mid_px, arrival_bid_px, arrival_ask_px, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'live_submitted', ?, '', ?, 1, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&evidence.strategy_id)
    .bind(&evidence.strategy_name)
    .bind(inst_id)
    .bind(inst_id)
    .bind(inst_type.trim().to_ascii_uppercase())
    .bind(side)
    .bind(order_type)
    .bind(size.unwrap_or(0.0))
    .bind(price)
    .bind(order_id)
    .bind(if client_order_id.is_empty() {
        evidence.client_order_id.as_str()
    } else {
        client_order_id
    })
    .bind(&evidence.action)
    .bind(mode)
    .bind(&evidence.run_id)
    .bind(evidence.action_timestamp)
    .bind(arrival.ts_ms)
    .bind(arrival.mid_px)
    .bind(arrival.bid_px)
    .bind(arrival.ask_px)
    .bind(now)
    .execute(&state.db)
    .await?;
    Ok(result.last_insert_rowid())
}
