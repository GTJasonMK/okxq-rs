use super::super::super::*;

#[derive(Clone, Debug, Default)]
pub(in crate::commands::local_api::trading::actions) struct ManualArrivalQuote {
    pub(in crate::commands::local_api::trading::actions) ts_ms: Option<i64>,
    pub(in crate::commands::local_api::trading::actions) mid_px: Option<f64>,
    pub(in crate::commands::local_api::trading::actions) bid_px: Option<f64>,
    pub(in crate::commands::local_api::trading::actions) ask_px: Option<f64>,
}

#[derive(Clone, Debug)]
pub(in crate::commands::local_api::trading::actions) struct ManualCostEvidenceRequest {
    pub(in crate::commands::local_api::trading::actions::cost_evidence) strategy_id: String,
    pub(in crate::commands::local_api::trading::actions::cost_evidence) strategy_name: String,
    pub(in crate::commands::local_api::trading::actions::cost_evidence) run_id: String,
    pub(in crate::commands::local_api::trading::actions::cost_evidence) action: String,
    pub(in crate::commands::local_api::trading::actions::cost_evidence) action_timestamp: i64,
    pub(in crate::commands::local_api::trading::actions::cost_evidence) client_order_id: String,
}

impl ManualCostEvidenceRequest {
    pub(in crate::commands::local_api::trading::actions) fn from_request(
        req: &LocalApiRequest,
        client_order_id: &str,
    ) -> AppResult<Self> {
        let strategy_id = body_string(req, "strategy_id", "");
        let run_id = body_string(req, "run_id", "");
        if strategy_id.trim().is_empty() || run_id.trim().is_empty() {
            return Err(AppError::Validation(
                "record_cost_evidence=true 时必须提供 strategy_id 和 run_id".to_string(),
            ));
        }
        Ok(Self {
            strategy_name: body_string(req, "strategy_name", &strategy_id),
            strategy_id,
            run_id,
            action: body_string(req, "action", "manual_order"),
            action_timestamp: body_i64(
                req,
                "action_timestamp",
                chrono::Utc::now().timestamp_millis(),
            ),
            client_order_id: client_order_id.to_string(),
        })
    }
}
