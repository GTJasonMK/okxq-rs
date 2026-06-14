use serde_json::Value;
use sqlx::{Row, SqlitePool};

use crate::error::{AppError, AppResult};

#[derive(Debug, Default)]
pub(crate) struct ArrivalEvidence {
    pub(crate) strategy_id: String,
    pub(crate) run_id: String,
    pub(crate) arrival_ts: Option<i64>,
    pub(crate) arrival_mid_px: Option<f64>,
    pub(crate) arrival_bid_px: Option<f64>,
    pub(crate) arrival_ask_px: Option<f64>,
}

impl ArrivalEvidence {
    pub(crate) fn has_complete_arrival_quote(&self) -> bool {
        self.arrival_mid_px.is_some_and(valid_positive_price)
            && self.arrival_bid_px.is_some_and(valid_positive_price)
            && self.arrival_ask_px.is_some_and(valid_positive_price)
    }
}

pub(crate) struct UpsertLocalFillRequest<'a> {
    pub(crate) db: &'a SqlitePool,
    pub(crate) mode: &'a str,
    pub(crate) trade_id: &'a str,
    pub(crate) inst_id: &'a str,
    pub(crate) item: &'a Value,
    pub(crate) order_id: &'a str,
    pub(crate) client_order_id: &'a str,
    pub(crate) arrival: &'a ArrivalEvidence,
}

struct ArrivalEvidenceCandidate {
    evidence: ArrivalEvidence,
    created_at: String,
    id: i64,
}

const ARRIVAL_EVIDENCE_SYNCABLE_STATUS_FILTER_SQL: &str = r#"
              AND (
                COALESCE(success, 0) = 1
                OR LOWER(TRIM(COALESCE(status, ''))) IN (
                  'submitting', 'submit_unknown', 'submitted', 'pending', 'open', 'live',
                  'partially_filled', 'partial-filled', 'partially-filled',
                  'cancel_requested', 'modify_requested',
                  'algo_submitting', 'algo_submitted', 'algo_submit_unknown', 'algo_live',
                  'algo_cancel_requested', 'algo_modify_requested',
                  'algo_partially_effective', 'algo_effective'
                )
              )
              AND LOWER(TRIM(COALESCE(status, ''))) NOT IN (
                'blocked', 'risk_blocked', 'submit_failed', 'algo_failed', 'rejected', 'reject'
              )
"#;

#[cfg(test)]
pub(crate) async fn lookup_arrival_evidence(
    db: &SqlitePool,
    mode: &str,
    order_id: &str,
    client_order_id: &str,
) -> AppResult<ArrivalEvidence> {
    lookup_arrival_evidence_scoped(db, mode, None, order_id, client_order_id).await
}

pub(crate) async fn lookup_arrival_evidence_for_symbol(
    db: &SqlitePool,
    mode: &str,
    inst_id: &str,
    order_id: &str,
    client_order_id: &str,
) -> AppResult<ArrivalEvidence> {
    let inst_id = inst_id.trim();
    if inst_id.is_empty() {
        return Ok(ArrivalEvidence::default());
    }
    lookup_arrival_evidence_scoped(db, mode, Some(inst_id), order_id, client_order_id).await
}

async fn lookup_arrival_evidence_scoped(
    db: &SqlitePool,
    mode: &str,
    inst_id: Option<&str>,
    order_id: &str,
    client_order_id: &str,
) -> AppResult<ArrivalEvidence> {
    if order_id.is_empty() && client_order_id.is_empty() {
        return Ok(ArrivalEvidence::default());
    }
    let order_candidate = lookup_arrival_evidence_by_identity(
        db,
        mode,
        inst_id,
        LiveOrderIdentityColumn::OrderId,
        order_id,
    )
    .await?;
    let client_candidate = lookup_arrival_evidence_by_identity(
        db,
        mode,
        inst_id,
        LiveOrderIdentityColumn::ClientOrderId,
        client_order_id,
    )
    .await?;

    if let (Some(order_candidate), Some(client_candidate)) = (&order_candidate, &client_candidate) {
        if order_candidate.id != client_candidate.id {
            return Err(AppError::Validation(format!(
                "OKX fill ordId/clOrdId 在本地命中不同订单记录，已拒绝成交归因: order_id={order_id}, client_order_id={client_order_id}"
            )));
        }
    }

    let Some(mut candidate) = newest_arrival_candidate(order_candidate, client_candidate) else {
        return Ok(ArrivalEvidence::default());
    };
    if !candidate.evidence.has_complete_arrival_quote() {
        candidate.evidence.arrival_ts = None;
        candidate.evidence.arrival_mid_px = None;
        candidate.evidence.arrival_bid_px = None;
        candidate.evidence.arrival_ask_px = None;
    }
    Ok(candidate.evidence)
}

pub(crate) async fn upsert_local_fill(request: UpsertLocalFillRequest<'_>) -> AppResult<()> {
    let UpsertLocalFillRequest {
        db,
        mode,
        trade_id,
        inst_id,
        item,
        order_id,
        client_order_id,
        arrival,
    } = request;
    let fill_px = okx_text(item, &["fillPx", "fill_px"]);
    let fill_sz = okx_text(item, &["fillSz", "fill_sz"]);
    let fee = okx_text(item, &["fee"]);
    let fee_ccy = okx_text(item, &["feeCcy", "fee_ccy"]);
    let side = normalize_fill_side(&okx_text(item, &["side"]));
    let ts = okx_i64(item, &["ts"], 0);
    let source = match okx_text(item, &["source"]).trim() {
        "" => "okx_fills_history".to_string(),
        value => value.to_string(),
    };
    let ccy = if !fee_ccy.is_empty() {
        fee_ccy.clone()
    } else {
        fill_ccy_from_inst_id(inst_id)
    };
    sqlx::query(
        r#"
        INSERT INTO local_fills (
          trade_id, inst_id, ccy, side, fill_px, fill_sz, fee, fee_ccy, ts,
          mode, source, order_id, client_order_id, strategy_id, run_id,
          arrival_ts, arrival_mid_px, arrival_bid_px, arrival_ask_px
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
                  ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(trade_id, mode) DO UPDATE SET
          inst_id = excluded.inst_id,
          ccy = excluded.ccy,
          side = excluded.side,
          fill_px = excluded.fill_px,
          fill_sz = excluded.fill_sz,
          fee = excluded.fee,
          fee_ccy = excluded.fee_ccy,
          ts = excluded.ts,
          source = excluded.source,
          order_id = excluded.order_id,
          client_order_id = excluded.client_order_id,
          strategy_id = CASE
            WHEN excluded.strategy_id != '' THEN excluded.strategy_id
            ELSE local_fills.strategy_id
          END,
          run_id = CASE
            WHEN excluded.run_id != '' THEN excluded.run_id
            ELSE local_fills.run_id
          END,
          arrival_ts = COALESCE(excluded.arrival_ts, local_fills.arrival_ts),
          arrival_mid_px = COALESCE(excluded.arrival_mid_px, local_fills.arrival_mid_px),
          arrival_bid_px = COALESCE(excluded.arrival_bid_px, local_fills.arrival_bid_px),
          arrival_ask_px = COALESCE(excluded.arrival_ask_px, local_fills.arrival_ask_px)
        "#,
    )
    .bind(trade_id)
    .bind(inst_id)
    .bind(ccy)
    .bind(side)
    .bind(fill_px)
    .bind(fill_sz)
    .bind(fee)
    .bind(fee_ccy)
    .bind(ts)
    .bind(mode)
    .bind(source)
    .bind(order_id)
    .bind(client_order_id)
    .bind(&arrival.strategy_id)
    .bind(&arrival.run_id)
    .bind(arrival.arrival_ts)
    .bind(arrival.arrival_mid_px)
    .bind(arrival.arrival_bid_px)
    .bind(arrival.arrival_ask_px)
    .execute(db)
    .await?;
    Ok(())
}

pub(crate) fn okx_text(item: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| item.get(*key).and_then(value_as_string))
        .unwrap_or_default()
        .trim()
        .to_string()
}

pub(crate) fn okx_i64(item: &Value, keys: &[&str], default: i64) -> i64 {
    keys.iter()
        .find_map(|key| item.get(*key).and_then(value_as_i64))
        .unwrap_or(default)
}

async fn lookup_arrival_evidence_by_identity(
    db: &SqlitePool,
    mode: &str,
    inst_id: Option<&str>,
    column: LiveOrderIdentityColumn,
    value: &str,
) -> AppResult<Option<ArrivalEvidenceCandidate>> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let identity_predicate = column.predicate();
    let inst_id_filter_sql = if inst_id.is_some() {
        "AND UPPER(TRIM(COALESCE(inst_id, ''))) = UPPER(TRIM(?))"
    } else {
        ""
    };
    let sql = format!(
        r#"
            SELECT id, created_at, strategy_id, run_id, COALESCE(arrival_ts, action_timestamp) AS arrival_ts,
                   arrival_mid_px, arrival_bid_px, arrival_ask_px
            FROM live_order_records
            WHERE mode = ?
              {inst_id_filter_sql}
              AND {identity_predicate}
{ARRIVAL_EVIDENCE_SYNCABLE_STATUS_FILTER_SQL}
            ORDER BY created_at DESC, id DESC
            LIMIT 2
            "#
    );
    let mut query = sqlx::query(&sql).bind(mode);
    if let Some(inst_id) = inst_id {
        query = query.bind(inst_id.trim());
    }
    let rows = query.bind(value).bind(value).fetch_all(db).await?;

    if rows.is_empty() {
        return Ok(None);
    }
    if rows.len() > 1 {
        return Err(AppError::Validation(format!(
            "OKX fill {}={} 在本地命中多条订单记录，已拒绝成交归因",
            column.label(),
            value
        )));
    }
    let row = &rows[0];
    Ok(Some(ArrivalEvidenceCandidate {
        created_at: row.try_get::<String, _>("created_at").unwrap_or_default(),
        id: row.try_get::<i64, _>("id").unwrap_or_default(),
        evidence: ArrivalEvidence {
            strategy_id: row.try_get::<String, _>("strategy_id").unwrap_or_default(),
            run_id: row.try_get::<String, _>("run_id").unwrap_or_default(),
            arrival_ts: row.try_get::<Option<i64>, _>("arrival_ts").ok().flatten(),
            arrival_mid_px: row
                .try_get::<Option<f64>, _>("arrival_mid_px")
                .ok()
                .flatten(),
            arrival_bid_px: row
                .try_get::<Option<f64>, _>("arrival_bid_px")
                .ok()
                .flatten(),
            arrival_ask_px: row
                .try_get::<Option<f64>, _>("arrival_ask_px")
                .ok()
                .flatten(),
        },
    }))
}

enum LiveOrderIdentityColumn {
    OrderId,
    ClientOrderId,
}

impl LiveOrderIdentityColumn {
    fn predicate(&self) -> &'static str {
        match self {
            Self::OrderId => "(order_id = ? OR actual_order_id = ?)",
            Self::ClientOrderId => "(client_order_id = ? OR actual_client_order_id = ?)",
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::OrderId => "ordId",
            Self::ClientOrderId => "clOrdId",
        }
    }
}

fn newest_arrival_candidate(
    left: Option<ArrivalEvidenceCandidate>,
    right: Option<ArrivalEvidenceCandidate>,
) -> Option<ArrivalEvidenceCandidate> {
    match (left, right) {
        (Some(left), Some(right)) => {
            if arrival_candidate_newer(&right, &left) {
                Some(right)
            } else {
                Some(left)
            }
        }
        (Some(candidate), None) | (None, Some(candidate)) => Some(candidate),
        (None, None) => None,
    }
}

fn arrival_candidate_newer(
    candidate: &ArrivalEvidenceCandidate,
    current: &ArrivalEvidenceCandidate,
) -> bool {
    candidate.created_at > current.created_at
        || (candidate.created_at == current.created_at && candidate.id > current.id)
}

fn value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(item) => Some(item.clone()),
        Value::Number(item) => Some(item.to_string()),
        Value::Bool(item) => Some(item.to_string()),
        _ => None,
    }
}

fn value_as_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(item) => item.as_i64(),
        Value::String(item) => item.parse::<i64>().ok(),
        _ => None,
    }
}

fn normalize_fill_side(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "buy" => "buy".to_string(),
        "sell" => "sell".to_string(),
        other => other.to_string(),
    }
}

fn fill_ccy_from_inst_id(inst_id: &str) -> String {
    inst_id.split('-').next().unwrap_or(inst_id).to_string()
}

fn valid_positive_price(value: f64) -> bool {
    value.is_finite() && value > 0.0
}
