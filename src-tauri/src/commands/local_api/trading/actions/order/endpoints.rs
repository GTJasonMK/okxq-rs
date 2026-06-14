use serde_json::{json, Value};

use super::super::{super::*, cost_evidence::*};
use super::params::{
    normalize_order_inst_id, normalize_order_inst_type, resolve_order_pos_side,
    resolve_order_td_mode,
};

/// POST /api/trading/order — 下单（现货/合约）
pub(crate) async fn trading_place_order(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let raw_inst_id = body_string(req, "inst_id", "");
    let requested_inst_type = body_string(req, "inst_type", &infer_inst_type(&raw_inst_id));
    let inst_type = normalize_order_inst_type(&requested_inst_type, &raw_inst_id);
    let inst_id = normalize_order_inst_id(&raw_inst_id, &inst_type);
    let td_mode = resolve_order_td_mode(&inst_type, &body_string(req, "td_mode", ""));
    let side = body_string(req, "side", "");
    let ord_type = body_string(req, "ord_type", "market");
    let sz = body_string(req, "sz", "");
    let px = body_string(req, "px", "");
    let requested_pos_side = body_string(req, "pos_side", "");
    let reduce_only = body_bool(req, "reduce_only", false);
    let record_cost_evidence = body_bool(req, "record_cost_evidence", false);
    let mut client_order_id = body_string(req, "cl_ord_id", "");
    if inst_id.is_empty() || side.is_empty() || sz.is_empty() {
        return Err(AppError::Validation(
            "inst_id/side/sz 为必填参数".to_string(),
        ));
    }
    let cost_evidence = if record_cost_evidence {
        client_order_id = evidence_client_order_id(&client_order_id);
        Some(ManualCostEvidenceRequest::from_request(
            req,
            &client_order_id,
        )?)
    } else {
        None
    };
    let client = okx_private_client(state, &mode).await?;
    let account_config = if inst_type == "SWAP" {
        Some(client.get_account_config().await?)
    } else {
        None
    };
    let pos_side =
        resolve_order_pos_side(&inst_type, &requested_pos_side, account_config.as_ref())?;
    let pre_submit_arrival = if cost_evidence.is_some() {
        Some(fetch_manual_arrival_quote(state, &inst_id).await)
    } else {
        None
    };
    let result = client
        .place_order(
            &inst_id,
            &td_mode,
            &side,
            &ord_type,
            &sz,
            &px,
            &pos_side,
            reduce_only,
            &client_order_id,
        )
        .await?;
    if let Some(evidence) = cost_evidence {
        let arrival = pre_submit_arrival.unwrap_or_default();
        let order_id = value_text(&result, "ordId").unwrap_or_default();
        let response_client_order_id = value_text(&result, "clOrdId").unwrap_or_default();
        let linked_client_order_id = if response_client_order_id.is_empty() {
            client_order_id.clone()
        } else {
            response_client_order_id
        };
        let insert_result = insert_manual_cost_order_record(
            state,
            &mode,
            &inst_id,
            &inst_type,
            &side,
            &ord_type,
            parse_optional_f64(&sz),
            parse_optional_f64(&px).or(arrival.mid_px),
            &order_id,
            &linked_client_order_id,
            &evidence,
            &arrival,
        )
        .await;
        let mut enriched = result;
        if let Some(obj) = enriched.as_object_mut() {
            obj.insert("cost_evidence_requested".to_string(), Value::Bool(true));
            obj.insert(
                "cost_evidence_client_order_id".to_string(),
                Value::String(linked_client_order_id),
            );
            match insert_result {
                Ok(row_id) => {
                    obj.insert("cost_evidence_recorded".to_string(), Value::Bool(true));
                    obj.insert("cost_evidence_order_record_id".to_string(), json!(row_id));
                }
                Err(error) => {
                    obj.insert("cost_evidence_recorded".to_string(), Value::Bool(false));
                    obj.insert(
                        "cost_evidence_error".to_string(),
                        Value::String(error.to_string()),
                    );
                }
            }
        }
        return Ok(code_ok(enriched));
    }
    Ok(code_ok(result))
}

/// POST /api/trading/cancel — 撤单
pub(crate) async fn trading_cancel_order(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let mode = request_trading_mode(state, req).await?;
    let inst_id = body_string(req, "inst_id", "");
    let ord_id = body_string(req, "ord_id", "");
    let client_order_id = body_string(req, "cl_ord_id", "");
    if inst_id.is_empty() || (ord_id.is_empty() && client_order_id.is_empty()) {
        return Err(AppError::Validation(
            "inst_id 和 ord_id/cl_ord_id 至少传一个".to_string(),
        ));
    }
    let client = okx_private_client(state, &mode).await?;
    let result = client
        .cancel_order(&inst_id, &ord_id, &client_order_id)
        .await?;
    Ok(code_ok(result))
}
