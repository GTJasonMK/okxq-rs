use std::collections::HashSet;

use sqlx::{QueryBuilder, Sqlite, SqlitePool};

use crate::{
    error::AppResult, live_strategy::storage::LivePlannedExitPlan,
    trading_semantics::planned_exit_residual_exchange_quantity,
};

#[derive(Clone, Copy, Debug)]
pub(super) struct PlannedExitFillScope {
    pub(super) entry_filled: f64,
    pub(super) exit_filled: f64,
    pub(super) residual: f64,
}

pub(super) async fn planned_exit_fill_scope(
    db: &SqlitePool,
    plan: &LivePlannedExitPlan,
) -> AppResult<PlannedExitFillScope> {
    let entry_filled = sum_local_fill_size_for_order_identity(
        db,
        &plan.mode,
        &plan.entry_order_id,
        &plan.entry_client_order_id,
    )
    .await?;
    let exit_filled = sum_local_fill_size_for_order_identities(
        db,
        &plan.mode,
        &planned_exit_order_history_identities(plan),
    )
    .await?;
    let residual = planned_exit_residual_exchange_quantity(entry_filled, exit_filled)?;
    Ok(PlannedExitFillScope {
        entry_filled,
        exit_filled,
        residual,
    })
}

async fn sum_local_fill_size_for_order_identity(
    db: &SqlitePool,
    mode: &str,
    order_id: &str,
    client_order_id: &str,
) -> AppResult<f64> {
    sum_local_fill_size_for_order_identities(
        db,
        mode,
        &[OrderIdentity::new(order_id, client_order_id)],
    )
    .await
}

async fn sum_local_fill_size_for_order_identities(
    db: &SqlitePool,
    mode: &str,
    identities: &[OrderIdentity],
) -> AppResult<f64> {
    let order_ids = sorted_non_empty_identity_values(identities.iter().map(|item| &item.order_id));
    let client_order_ids =
        sorted_non_empty_identity_values(identities.iter().map(|item| &item.client_order_id));
    if order_ids.is_empty() && client_order_ids.is_empty() {
        return Ok(0.0);
    }
    let mut query = QueryBuilder::<Sqlite>::new(
        r#"
        SELECT SUM(CASE
          WHEN CAST(fill_sz AS REAL) > 0 THEN CAST(fill_sz AS REAL)
          ELSE 0
        END)
        FROM local_fills
        WHERE mode =
        "#,
    );
    query.push_bind(mode.trim());
    query.push(" AND (");
    let mut has_condition = false;
    if !order_ids.is_empty() {
        query.push("order_id IN (");
        let mut separated = query.separated(", ");
        for order_id in &order_ids {
            separated.push_bind(order_id);
        }
        separated.push_unseparated(")");
        has_condition = true;
    }
    if !client_order_ids.is_empty() {
        if has_condition {
            query.push(" OR ");
        }
        query.push("client_order_id IN (");
        let mut separated = query.separated(", ");
        for client_order_id in &client_order_ids {
            separated.push_bind(client_order_id);
        }
        separated.push_unseparated(")");
    }
    query.push(")");
    let value = query
        .build_query_scalar::<Option<f64>>()
        .fetch_one(db)
        .await?;
    Ok(value.unwrap_or(0.0).max(0.0))
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct OrderIdentity {
    order_id: String,
    client_order_id: String,
}

impl OrderIdentity {
    fn new(order_id: &str, client_order_id: &str) -> Self {
        Self {
            order_id: order_id.trim().to_string(),
            client_order_id: client_order_id.trim().to_string(),
        }
    }

    fn is_empty(&self) -> bool {
        self.order_id.is_empty() && self.client_order_id.is_empty()
    }
}

fn planned_exit_order_history_identities(plan: &LivePlannedExitPlan) -> Vec<OrderIdentity> {
    let mut identities = Vec::new();
    push_unique_order_identity(
        &mut identities,
        OrderIdentity::new(&plan.exit_order_id, &plan.exit_client_order_id),
    );
    for line in plan.exit_order_history.lines() {
        let mut parts = line.splitn(2, '\t');
        let order_id = parts.next().unwrap_or_default();
        let client_order_id = parts.next().unwrap_or_default();
        push_unique_order_identity(
            &mut identities,
            OrderIdentity::new(order_id, client_order_id),
        );
    }
    identities
}

fn push_unique_order_identity(identities: &mut Vec<OrderIdentity>, identity: OrderIdentity) {
    if identity.is_empty() || identities.iter().any(|item| item == &identity) {
        return;
    }
    identities.push(identity);
}

fn sorted_non_empty_identity_values<'a>(values: impl Iterator<Item = &'a String>) -> Vec<String> {
    let mut unique = HashSet::new();
    for value in values {
        let value = value.trim();
        if !value.is_empty() {
            unique.insert(value.to_string());
        }
    }
    let mut values = unique.into_iter().collect::<Vec<_>>();
    values.sort();
    values
}
