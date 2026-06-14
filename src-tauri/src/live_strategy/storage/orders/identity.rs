use sqlx::{Row, SqlitePool};

use crate::error::{AppError, AppResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LiveOrderIdentityScope {
    ExchangeParent,
    AlgoParent,
}

pub(super) struct LiveOrderIdentityRecordContext {
    pub(super) mode: String,
    pub(super) inst_id: String,
    pub(super) action: String,
}

const SYNCABLE_STATUS_FILTER_SQL: &str = r#"
          AND (
            success = 1
            OR LOWER(TRIM(status)) IN (
              'submitting', 'submit_unknown', 'submitted', 'pending', 'open', 'live',
              'partially_filled', 'partial-filled', 'partially-filled',
              'cancel_requested', 'modify_requested',
              'algo_submitting', 'algo_submitted', 'algo_submit_unknown', 'algo_live',
              'algo_cancel_requested', 'algo_modify_requested',
              'algo_partially_effective', 'algo_effective'
            )
          )
          AND LOWER(TRIM(status)) NOT IN (
            'blocked', 'risk_blocked', 'submit_failed', 'algo_failed', 'rejected', 'reject'
          )
"#;

pub(super) fn identity_scope_for_action(action: &str) -> LiveOrderIdentityScope {
    if action.trim() == "place_risk_order" {
        LiveOrderIdentityScope::AlgoParent
    } else {
        LiveOrderIdentityScope::ExchangeParent
    }
}

pub(super) fn order_identity_participates_in_sync(status: &str, success: bool) -> bool {
    let status = status.trim().to_ascii_lowercase();
    if matches!(
        status.as_str(),
        "blocked" | "risk_blocked" | "submit_failed" | "algo_failed" | "rejected" | "reject"
    ) {
        return false;
    }
    success
        || matches!(
            status.as_str(),
            "submitting"
                | "submit_unknown"
                | "submitted"
                | "pending"
                | "open"
                | "live"
                | "partially_filled"
                | "partial-filled"
                | "partially-filled"
                | "cancel_requested"
                | "modify_requested"
                | "algo_submitting"
                | "algo_submitted"
                | "algo_submit_unknown"
                | "algo_live"
                | "algo_cancel_requested"
                | "algo_modify_requested"
                | "algo_partially_effective"
                | "algo_effective"
        )
}

pub(super) async fn query_order_identity_record_context(
    pool: &SqlitePool,
    row_id: i64,
) -> AppResult<Option<LiveOrderIdentityRecordContext>> {
    let row = sqlx::query(
        r#"
        SELECT mode, inst_id, action
        FROM live_order_records
        WHERE id = ?
        "#,
    )
    .bind(row_id)
    .fetch_optional(pool)
    .await?;

    row.map(|row| {
        Ok(LiveOrderIdentityRecordContext {
            mode: required_identity_text(&row, "mode")?,
            inst_id: required_identity_text(&row, "inst_id")?,
            action: required_identity_text(&row, "action")?,
        })
    })
    .transpose()
}

pub(super) async fn find_unique_parent_identity_match(
    pool: &SqlitePool,
    mode: &str,
    inst_id: &str,
    scope: LiveOrderIdentityScope,
    order_id: &str,
    client_order_id: &str,
) -> AppResult<Option<i64>> {
    let ids =
        query_parent_identity_matches(pool, mode, inst_id, scope, order_id, client_order_id, None)
            .await?;
    reject_ambiguous_identity_matches(scope, mode, inst_id, order_id, client_order_id, &ids)?;
    Ok(ids.into_iter().next())
}

pub(super) async fn ensure_parent_identity_available(
    pool: &SqlitePool,
    mode: &str,
    inst_id: &str,
    scope: LiveOrderIdentityScope,
    order_id: &str,
    client_order_id: &str,
    exclude_id: Option<i64>,
) -> AppResult<()> {
    let ids = query_parent_identity_matches(
        pool,
        mode,
        inst_id,
        scope,
        order_id,
        client_order_id,
        exclude_id,
    )
    .await?;
    if ids.is_empty() {
        return Ok(());
    }
    Err(AppError::Validation(format!(
        "实时策略{}身份已命中其他活动/待归因订单记录，已拒绝写入或更新: mode={}, inst_id={}, order_id={}, client_order_id={}, matched_ids={}",
        scope.label(),
        mode.trim(),
        inst_id.trim(),
        order_id.trim(),
        client_order_id.trim(),
        format_ids(&ids)
    )))
}

pub(super) async fn ensure_fill_identity_available(
    pool: &SqlitePool,
    mode: &str,
    inst_id: &str,
    order_id: &str,
    client_order_id: &str,
    exclude_id: Option<i64>,
) -> AppResult<()> {
    let ids =
        query_fill_identity_matches(pool, mode, inst_id, order_id, client_order_id, exclude_id)
            .await?;
    if ids.is_empty() {
        return Ok(());
    }
    Err(AppError::Validation(format!(
        "实时策略成交身份已命中其他活动/待归因订单记录，已拒绝写入或更新: mode={}, inst_id={}, order_id={}, client_order_id={}, matched_ids={}",
        mode.trim(),
        inst_id.trim(),
        order_id.trim(),
        client_order_id.trim(),
        format_ids(&ids)
    )))
}

async fn query_parent_identity_matches(
    pool: &SqlitePool,
    mode: &str,
    inst_id: &str,
    scope: LiveOrderIdentityScope,
    order_id: &str,
    client_order_id: &str,
    exclude_id: Option<i64>,
) -> AppResult<Vec<i64>> {
    if order_id.trim().is_empty() && client_order_id.trim().is_empty() {
        return Ok(Vec::new());
    }
    let action_filter_sql = scope.parent_action_filter_sql();
    let exclude_filter_sql = if exclude_id.is_some() {
        "AND id <> ?"
    } else {
        ""
    };
    let sql = format!(
        r#"
        SELECT id
        FROM live_order_records
        WHERE mode = ?
          AND UPPER(TRIM(inst_id)) = UPPER(TRIM(?))
          {action_filter_sql}
          {exclude_filter_sql}
          AND (
            (LENGTH(TRIM(?)) > 0 AND order_id = ?)
            OR (LENGTH(TRIM(?)) > 0 AND client_order_id = ?)
          )
{SYNCABLE_STATUS_FILTER_SQL}
        ORDER BY created_at DESC, id DESC
        LIMIT 2
        "#
    );
    let mut query = sqlx::query(&sql).bind(mode.trim()).bind(inst_id.trim());
    if let Some(exclude_id) = exclude_id {
        query = query.bind(exclude_id);
    }
    let rows = query
        .bind(order_id.trim())
        .bind(order_id.trim())
        .bind(client_order_id.trim())
        .bind(client_order_id.trim())
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| row.get::<i64, _>("id"))
        .collect())
}

async fn query_fill_identity_matches(
    pool: &SqlitePool,
    mode: &str,
    inst_id: &str,
    order_id: &str,
    client_order_id: &str,
    exclude_id: Option<i64>,
) -> AppResult<Vec<i64>> {
    if order_id.trim().is_empty() && client_order_id.trim().is_empty() {
        return Ok(Vec::new());
    }
    let exclude_filter_sql = if exclude_id.is_some() {
        "AND id <> ?"
    } else {
        ""
    };
    let sql = format!(
        r#"
        SELECT id
        FROM live_order_records
        WHERE mode = ?
          AND UPPER(TRIM(inst_id)) = UPPER(TRIM(?))
          {exclude_filter_sql}
          AND (
            (LENGTH(TRIM(?)) > 0 AND (order_id = ? OR actual_order_id = ?))
            OR (LENGTH(TRIM(?)) > 0 AND (client_order_id = ? OR actual_client_order_id = ?))
          )
{SYNCABLE_STATUS_FILTER_SQL}
        ORDER BY created_at DESC, id DESC
        LIMIT 2
        "#
    );
    let mut query = sqlx::query(&sql).bind(mode.trim()).bind(inst_id.trim());
    if let Some(exclude_id) = exclude_id {
        query = query.bind(exclude_id);
    }
    let rows = query
        .bind(order_id.trim())
        .bind(order_id.trim())
        .bind(order_id.trim())
        .bind(client_order_id.trim())
        .bind(client_order_id.trim())
        .bind(client_order_id.trim())
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| row.get::<i64, _>("id"))
        .collect())
}

fn reject_ambiguous_identity_matches(
    scope: LiveOrderIdentityScope,
    mode: &str,
    inst_id: &str,
    order_id: &str,
    client_order_id: &str,
    ids: &[i64],
) -> AppResult<()> {
    if ids.len() <= 1 {
        return Ok(());
    }
    Err(AppError::Validation(format!(
        "实时策略{}身份在本地命中多条活动/待归因订单记录，已拒绝状态更新: mode={}, inst_id={}, order_id={}, client_order_id={}, matched_ids={}",
        scope.label(),
        mode.trim(),
        inst_id.trim(),
        order_id.trim(),
        client_order_id.trim(),
        format_ids(ids)
    )))
}

impl LiveOrderIdentityScope {
    fn parent_action_filter_sql(self) -> &'static str {
        match self {
            Self::ExchangeParent => "AND action <> 'place_risk_order'",
            Self::AlgoParent => "AND action = 'place_risk_order'",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::ExchangeParent => "普通订单",
            Self::AlgoParent => "保护单",
        }
    }
}

fn required_identity_text(row: &sqlx::sqlite::SqliteRow, column: &str) -> AppResult<String> {
    let value = row.try_get::<String, _>(column)?;
    if value.trim().is_empty() {
        return Err(AppError::Runtime(format!(
            "live order identity {column} 不能为空"
        )));
    }
    Ok(value)
}

fn format_ids(ids: &[i64]) -> String {
    ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
}
