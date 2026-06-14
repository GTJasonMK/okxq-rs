mod decision;
mod gates;

pub(super) use self::decision::{
    attach_execution_decision, execution_decision_requires_exchange_risk_state,
    ExchangeRiskEvidence,
};

#[cfg(test)]
mod tests;
