use std::collections::HashSet;

use serde_json::{json, Value};
use sqlx::{sqlite::SqliteRow, Row, SqlitePool};

use crate::{
    error::{AppError, AppResult},
    strategy_engine::StrategyActionRecord,
};

use super::super::{decision::StrategyPlannedExitIntent, types::LiveStrategyConfig};

#[derive(Clone, Debug)]
pub(in crate::live_strategy) struct LivePlannedExitPlan {
    pub(in crate::live_strategy) id: i64,
    pub(in crate::live_strategy) strategy_id: String,
    pub(in crate::live_strategy) strategy_name: String,
    pub(in crate::live_strategy) mode: String,
    pub(in crate::live_strategy) symbol: String,
    pub(in crate::live_strategy) inst_type: String,
    pub(in crate::live_strategy) timeframe: String,
    pub(in crate::live_strategy) entry_order_id: String,
    pub(in crate::live_strategy) entry_client_order_id: String,
    pub(in crate::live_strategy) entry_price: f64,
    pub(in crate::live_strategy) close_side: String,
    pub(in crate::live_strategy) planned_exit_time: i64,
    pub(in crate::live_strategy) planned_exit_reason: String,
    pub(in crate::live_strategy) planned_exit_contract: String,
    pub(in crate::live_strategy) exit_order_id: String,
    pub(in crate::live_strategy) exit_client_order_id: String,
    pub(in crate::live_strategy) exit_order_history: String,
    pub(in crate::live_strategy) attempt_count: i64,
}

#[derive(Clone, Debug)]
pub(in crate::live_strategy) struct LivePlannedExitOrderSyncCandidate {
    pub(in crate::live_strategy) id: i64,
    pub(in crate::live_strategy) symbol: String,
    pub(in crate::live_strategy) inst_type: String,
    pub(in crate::live_strategy) order_id: String,
    pub(in crate::live_strategy) client_order_id: String,
    pub(in crate::live_strategy) updated_at_ms: i64,
}

#[derive(Clone, Debug)]
pub(in crate::live_strategy) struct LivePlannedExitInsertOutcome {
    pub(in crate::live_strategy) id: i64,
    pub(in crate::live_strategy) inserted: bool,
}

#[allow(clippy::too_many_arguments)]
pub(in crate::live_strategy) async fn insert_live_planned_exit_plan(
    pool: &SqlitePool,
    config: &LiveStrategyConfig,
    run_id: &str,
    action_record: &StrategyActionRecord,
    entry_side: &str,
    close_side: &str,
    planned_exit: &StrategyPlannedExitIntent,
    entry_order_id: &str,
    entry_client_order_id: &str,
) -> AppResult<LivePlannedExitInsertOutcome> {
    validate_planned_exit_plan(config, action_record, entry_side, close_side, planned_exit)?;
    let plan_key = planned_exit_plan_key(
        config,
        action_record,
        entry_side,
        close_side,
        planned_exit,
        entry_order_id,
        entry_client_order_id,
    );
    let now = chrono::Utc::now().to_rfc3339();
    let result = sqlx::query(
        r#"
        INSERT OR IGNORE INTO live_execution_plans (
          plan_key, strategy_id, strategy_name, mode, entry_run_id,
          symbol, inst_id, inst_type, timeframe,
          entry_order_id, entry_client_order_id, entry_action_timestamp,
          entry_side, entry_price, close_side,
          planned_exit_time, planned_exit_reason, planned_exit_contract,
          status, next_attempt_at, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'scheduled', 0, ?, ?)
        "#,
    )
    .bind(&plan_key)
    .bind(&config.strategy_id)
    .bind(&config.strategy_name)
    .bind(&config.mode)
    .bind(run_id)
    .bind(&config.symbol)
    .bind(&config.symbol)
    .bind(&config.inst_type)
    .bind(&config.timeframe)
    .bind(entry_order_id)
    .bind(entry_client_order_id)
    .bind(action_record.timestamp)
    .bind(entry_side)
    .bind(action_record.price)
    .bind(close_side)
    .bind(planned_exit.timestamp)
    .bind(&planned_exit.reason)
    .bind(&planned_exit.contract)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    let inserted = result.rows_affected() > 0;
    let id = if inserted {
        result.last_insert_rowid()
    } else {
        sqlx::query_scalar::<_, i64>("SELECT id FROM live_execution_plans WHERE plan_key = ?")
            .bind(&plan_key)
            .fetch_one(pool)
            .await?
    };
    Ok(LivePlannedExitInsertOutcome { id, inserted })
}

pub(in crate::live_strategy) async fn query_due_live_planned_exits(
    pool: &SqlitePool,
    mode: &str,
    strategy_id: &str,
    now_ms: i64,
    limit: i64,
) -> AppResult<Vec<LivePlannedExitPlan>> {
    let rows = sqlx::query(
        r#"
        SELECT id, strategy_id, strategy_name, mode, symbol, inst_type, timeframe,
               entry_order_id, entry_client_order_id, entry_price, close_side,
               planned_exit_time, planned_exit_reason, planned_exit_contract,
               exit_order_id, exit_client_order_id, exit_order_history, attempt_count
        FROM live_execution_plans
        WHERE mode = ?
          AND strategy_id = ?
          AND status = 'scheduled'
          AND planned_exit_time <= ?
          AND next_attempt_at <= ?
        ORDER BY planned_exit_time ASC, id ASC
        LIMIT ?
        "#,
    )
    .bind(mode.trim())
    .bind(strategy_id.trim())
    .bind(now_ms)
    .bind(now_ms)
    .bind(limit.clamp(1, 100))
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(plan_from_row).collect()
}

pub(in crate::live_strategy) async fn claim_due_live_planned_exit(
    pool: &SqlitePool,
    plan_id: i64,
    now_ms: i64,
) -> AppResult<bool> {
    let now = chrono::Utc::now().to_rfc3339();
    let result = sqlx::query(
        r#"
        UPDATE live_execution_plans
        SET status = 'exit_processing',
            last_error = '',
            updated_at = ?
        WHERE id = ?
          AND status = 'scheduled'
          AND planned_exit_time <= ?
          AND next_attempt_at <= ?
        "#,
    )
    .bind(&now)
    .bind(plan_id)
    .bind(now_ms)
    .bind(now_ms)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub(in crate::live_strategy) async fn next_live_planned_exit_wakeup(
    pool: &SqlitePool,
    mode: &str,
    strategy_id: &str,
) -> AppResult<Option<i64>> {
    sqlx::query_scalar::<_, Option<i64>>(
        r#"
        SELECT MIN(MAX(planned_exit_time, next_attempt_at))
        FROM live_execution_plans
        WHERE mode = ?
          AND strategy_id = ?
          AND status = 'scheduled'
        "#,
    )
    .bind(mode.trim())
    .bind(strategy_id.trim())
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

pub(in crate::live_strategy) async fn query_submitted_live_planned_exit_order_sync_candidates(
    pool: &SqlitePool,
    mode: &str,
    strategy_id: &str,
    limit: i64,
) -> AppResult<Vec<LivePlannedExitOrderSyncCandidate>> {
    let rows = sqlx::query(
        r#"
        SELECT p.id, p.symbol, p.inst_id, p.inst_type, p.exit_order_id, p.exit_client_order_id,
               CAST(strftime('%s', p.updated_at) AS INTEGER) * 1000 AS updated_at_ms
        FROM live_execution_plans p
        WHERE p.mode = ?
          AND p.strategy_id = ?
          AND p.status = 'exit_submitted'
          AND (
            LENGTH(TRIM(p.exit_order_id)) > 0
            OR LENGTH(TRIM(p.exit_client_order_id)) > 0
          )
          AND NOT EXISTS (
            SELECT 1
            FROM live_order_records o
            WHERE o.mode = p.mode
              AND o.strategy_id = p.strategy_id
              AND UPPER(TRIM(o.inst_id)) = UPPER(TRIM(p.inst_id))
              AND o.action <> 'place_risk_order'
              AND (
                (LENGTH(TRIM(p.exit_order_id)) > 0
                 AND o.order_id = p.exit_order_id)
                OR (LENGTH(TRIM(p.exit_client_order_id)) > 0
                    AND o.client_order_id = p.exit_client_order_id)
              )
              AND LOWER(TRIM(o.status)) IN (
                'submit_unknown', 'submitted', 'pending', 'open', 'live',
                'partially_filled', 'partial-filled', 'partially-filled'
              )
          )
        ORDER BY p.updated_at ASC, p.id ASC
        LIMIT ?
        "#,
    )
    .bind(mode.trim())
    .bind(strategy_id.trim())
    .bind(limit.clamp(1, 100))
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(planned_exit_order_sync_candidate_from_row)
        .collect()
}

pub(in crate::live_strategy) async fn requeue_stale_live_planned_exit_claims(
    pool: &SqlitePool,
    mode: &str,
    strategy_id: &str,
    stale_before: &str,
    retry_at: i64,
) -> AppResult<u64> {
    let now = chrono::Utc::now().to_rfc3339();
    let result = sqlx::query(
        r#"
        UPDATE live_execution_plans
        SET status = 'scheduled',
            attempt_count = attempt_count + 1,
            next_attempt_at = ?,
            last_error = '计划退出处理超时，已重新排队',
            updated_at = ?
        WHERE mode = ?
          AND strategy_id = ?
          AND status = 'exit_processing'
          AND updated_at <= ?
        "#,
    )
    .bind(retry_at.max(0))
    .bind(&now)
    .bind(mode.trim())
    .bind(strategy_id.trim())
    .bind(stale_before)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

pub(in crate::live_strategy) async fn recover_stale_live_planned_exit_claims_from_orders(
    pool: &SqlitePool,
    mode: &str,
    strategy_id: &str,
    stale_before: &str,
    retry_at: i64,
) -> AppResult<u64> {
    let rows = sqlx::query(
        r#"
        SELECT p.id AS plan_id,
               o.run_id AS run_id,
               COALESCE(o.order_id, '') AS order_id,
               COALESCE(o.client_order_id, '') AS client_order_id,
               o.status AS order_status,
               COALESCE(o.error_message, '') AS error_message
        FROM live_execution_plans p
        JOIN live_order_records o
          ON o.mode = p.mode
         AND o.strategy_id = p.strategy_id
         AND (o.symbol = p.symbol OR o.inst_id = p.symbol OR o.inst_id = p.inst_id)
         AND o.side = p.close_side
         AND o.action = 'close_position'
         AND o.action_timestamp = p.planned_exit_time
        WHERE p.mode = ?
          AND p.strategy_id = ?
          AND p.status = 'exit_processing'
          AND p.updated_at <= ?
          AND (
            LENGTH(TRIM(COALESCE(o.order_id, ''))) > 0
            OR LENGTH(TRIM(COALESCE(o.client_order_id, ''))) > 0
          )
          AND LOWER(TRIM(o.status)) IN (
            'submitted', 'live', 'partially_filled', 'filled',
            'canceled', 'cancelled', 'rejected'
          )
        ORDER BY p.id ASC, o.created_at DESC, o.id DESC
        "#,
    )
    .bind(mode.trim())
    .bind(strategy_id.trim())
    .bind(stale_before)
    .fetch_all(pool)
    .await?;

    let mut seen = HashSet::new();
    let mut changed = 0_u64;
    for row in rows {
        let plan_id = row.get::<i64, _>("plan_id");
        if !seen.insert(plan_id) {
            continue;
        }
        let run_id = row.get::<String, _>("run_id");
        let order_id = row.get::<String, _>("order_id");
        let client_order_id = row.get::<String, _>("client_order_id");
        let order_status = row
            .get::<String, _>("order_status")
            .trim()
            .to_ascii_lowercase();
        let error_message = row.get::<String, _>("error_message");
        changed += recover_stale_claim_from_order(
            pool,
            plan_id,
            &run_id,
            &order_id,
            &client_order_id,
            &order_status,
            &error_message,
            retry_at,
        )
        .await?;
    }
    Ok(changed)
}

async fn recover_stale_claim_from_order(
    pool: &SqlitePool,
    plan_id: i64,
    run_id: &str,
    order_id: &str,
    client_order_id: &str,
    order_status: &str,
    error_message: &str,
    retry_at: i64,
) -> AppResult<u64> {
    let now = chrono::Utc::now().to_rfc3339();
    let history_entry = exit_order_history_entry(order_id, client_order_id);
    let result = match order_status {
        "filled" | "fully_filled" | "fully-filled" => {
            sqlx::query(
                r#"
                UPDATE live_execution_plans
                SET status = 'exit_filled',
                    exit_run_id = ?,
                    exit_order_id = ?,
                    exit_client_order_id = ?,
                    exit_order_history = CASE
                      WHEN LENGTH(?) > 0 THEN exit_order_history || ?
                      ELSE exit_order_history
                    END,
                    last_error = ?,
                    updated_at = ?
                WHERE id = ?
                  AND status = 'exit_processing'
                "#,
            )
            .bind(run_id)
            .bind(order_id)
            .bind(client_order_id)
            .bind(&history_entry)
            .bind(&history_entry)
            .bind(non_empty_or(
                error_message,
                "已根据本地平仓成交记录恢复计划退出终态",
            ))
            .bind(&now)
            .bind(plan_id)
            .execute(pool)
            .await?
        }
        "canceled" | "cancelled" | "rejected" | "reject" => {
            sqlx::query(
                r#"
                UPDATE live_execution_plans
                SET status = 'scheduled',
                    exit_run_id = ?,
                    exit_order_id = ?,
                    exit_client_order_id = ?,
                    exit_order_history = CASE
                      WHEN LENGTH(?) > 0 THEN exit_order_history || ?
                      ELSE exit_order_history
                    END,
                    attempt_count = attempt_count + 1,
                    next_attempt_at = ?,
                    last_error = ?,
                    updated_at = ?
                WHERE id = ?
                  AND status = 'exit_processing'
                "#,
            )
            .bind(run_id)
            .bind(order_id)
            .bind(client_order_id)
            .bind(&history_entry)
            .bind(&history_entry)
            .bind(retry_at.max(0))
            .bind(non_empty_or(
                error_message,
                "已根据本地平仓订单终态重新排队",
            ))
            .bind(&now)
            .bind(plan_id)
            .execute(pool)
            .await?
        }
        _ => {
            sqlx::query(
                r#"
                UPDATE live_execution_plans
                SET status = 'exit_submitted',
                    exit_run_id = ?,
                    exit_order_id = ?,
                    exit_client_order_id = ?,
                    exit_order_history = CASE
                      WHEN LENGTH(?) > 0 THEN exit_order_history || ?
                      ELSE exit_order_history
                    END,
                    last_error = ?,
                    updated_at = ?
                WHERE id = ?
                  AND status = 'exit_processing'
                "#,
            )
            .bind(run_id)
            .bind(order_id)
            .bind(client_order_id)
            .bind(&history_entry)
            .bind(&history_entry)
            .bind(non_empty_or(
                error_message,
                "已根据本地平仓订单记录恢复计划退出提交状态",
            ))
            .bind(&now)
            .bind(plan_id)
            .execute(pool)
            .await?
        }
    };
    Ok(result.rows_affected())
}

pub async fn query_live_execution_plans(
    pool: &SqlitePool,
    limit: i64,
    mode: &str,
    run_id: &str,
) -> AppResult<Vec<Value>> {
    let mut sql = String::from(
        r#"
        SELECT id, plan_key, strategy_id, strategy_name, mode,
               entry_run_id, exit_run_id, symbol, inst_id, inst_type, timeframe,
               entry_order_id, entry_client_order_id, entry_action_timestamp,
               entry_side, entry_price, close_side,
               planned_exit_time, planned_exit_reason, planned_exit_contract,
               status, exit_order_id, exit_client_order_id, exit_order_history,
               attempt_count, next_attempt_at, last_error, created_at, updated_at
        FROM live_execution_plans
        WHERE 1 = 1
        "#,
    );
    if !mode.trim().is_empty() {
        sql.push_str(" AND mode = ?");
    }
    if !run_id.trim().is_empty() {
        sql.push_str(" AND (entry_run_id = ? OR exit_run_id = ?)");
    }
    sql.push_str(" ORDER BY planned_exit_time DESC, id DESC LIMIT ?");

    let mut query = sqlx::query(&sql);
    if !mode.trim().is_empty() {
        query = query.bind(mode.trim());
    }
    if !run_id.trim().is_empty() {
        query = query.bind(run_id.trim()).bind(run_id.trim());
    }
    query = query.bind(limit.clamp(1, 500));

    query
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(execution_plan_row_to_json)
        .collect::<AppResult<Vec<_>>>()
}

pub(in crate::live_strategy) async fn mark_live_planned_exit_submitted(
    pool: &SqlitePool,
    plan_id: i64,
    run_id: &str,
    exit_order_id: &str,
    exit_client_order_id: &str,
) -> AppResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let history_entry = exit_order_history_entry(exit_order_id, exit_client_order_id);
    sqlx::query(
        r#"
        UPDATE live_execution_plans
        SET status = 'exit_submitted',
            exit_run_id = ?,
            exit_order_id = ?,
            exit_client_order_id = ?,
            exit_order_history = CASE
              WHEN LENGTH(?) > 0 THEN exit_order_history || ?
              ELSE exit_order_history
            END,
            last_error = '',
            updated_at = ?
        WHERE id = ?
          AND status IN ('scheduled', 'exit_processing')
        "#,
    )
    .bind(run_id)
    .bind(exit_order_id)
    .bind(exit_client_order_id)
    .bind(&history_entry)
    .bind(&history_entry)
    .bind(&now)
    .bind(plan_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub(in crate::live_strategy) async fn mark_live_planned_exit_submitting(
    pool: &SqlitePool,
    plan_id: i64,
    run_id: &str,
    exit_client_order_id: &str,
) -> AppResult<bool> {
    let now = chrono::Utc::now().to_rfc3339();
    let history_entry = exit_order_history_entry("", exit_client_order_id);
    let result = sqlx::query(
        r#"
        UPDATE live_execution_plans
        SET exit_run_id = ?,
            exit_client_order_id = ?,
            exit_order_history = CASE
              WHEN LENGTH(?) > 0 THEN exit_order_history || ?
              ELSE exit_order_history
            END,
            last_error = '计划退出平仓单准备提交 OKX',
            updated_at = ?
        WHERE id = ?
          AND status = 'exit_processing'
        "#,
    )
    .bind(run_id)
    .bind(exit_client_order_id.trim())
    .bind(&history_entry)
    .bind(&history_entry)
    .bind(&now)
    .bind(plan_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub(in crate::live_strategy) async fn mark_live_planned_exit_skipped(
    pool: &SqlitePool,
    plan_id: i64,
    run_id: &str,
    status: &str,
    reason: &str,
) -> AppResult<()> {
    let normalized_status = match status.trim() {
        "skipped_no_position" | "skipped_invalid_plan" | "cancelled" => status.trim(),
        _ => "skipped_invalid_plan",
    };
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        UPDATE live_execution_plans
        SET status = ?,
            exit_run_id = ?,
            last_error = ?,
            updated_at = ?
        WHERE id = ?
          AND status IN ('scheduled', 'exit_processing')
        "#,
    )
    .bind(normalized_status)
    .bind(run_id)
    .bind(reason)
    .bind(&now)
    .bind(plan_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub(in crate::live_strategy) async fn mark_live_planned_exit_retry(
    pool: &SqlitePool,
    plan_id: i64,
    next_attempt_at: i64,
    reason: &str,
) -> AppResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        UPDATE live_execution_plans
        SET status = 'scheduled',
            attempt_count = attempt_count + 1,
            next_attempt_at = ?,
            last_error = ?,
            updated_at = ?
        WHERE id = ?
          AND status IN ('scheduled', 'exit_processing')
        "#,
    )
    .bind(next_attempt_at.max(0))
    .bind(reason)
    .bind(&now)
    .bind(plan_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub(in crate::live_strategy) async fn mark_live_planned_exit_entry_order_failed_for_mode(
    pool: &SqlitePool,
    mode: &str,
    inst_id: &str,
    order_id: &str,
    client_order_id: &str,
    order_status: &str,
    reason: &str,
) -> AppResult<u64> {
    let mode = mode.trim();
    let inst_id = inst_id.trim();
    if mode.is_empty() || inst_id.is_empty() {
        return Ok(0);
    }
    mark_live_planned_exit_entry_order_failed_scoped(
        pool,
        Some(mode),
        None,
        Some(inst_id),
        order_id,
        client_order_id,
        order_status,
        reason,
    )
    .await
}

pub(in crate::live_strategy) async fn mark_live_planned_exit_entry_order_failed_for_strategy(
    pool: &SqlitePool,
    mode: &str,
    strategy_id: &str,
    inst_id: &str,
    order_id: &str,
    client_order_id: &str,
    order_status: &str,
    reason: &str,
) -> AppResult<u64> {
    let mode = mode.trim();
    let strategy_id = strategy_id.trim();
    let inst_id = inst_id.trim();
    if mode.is_empty() || strategy_id.is_empty() || inst_id.is_empty() {
        return Ok(0);
    }
    mark_live_planned_exit_entry_order_failed_scoped(
        pool,
        Some(mode),
        Some(strategy_id),
        Some(inst_id),
        order_id,
        client_order_id,
        order_status,
        reason,
    )
    .await
}

async fn mark_live_planned_exit_entry_order_failed_scoped(
    pool: &SqlitePool,
    mode: Option<&str>,
    strategy_id: Option<&str>,
    inst_id: Option<&str>,
    order_id: &str,
    client_order_id: &str,
    order_status: &str,
    reason: &str,
) -> AppResult<u64> {
    if !is_entry_order_failed_without_fill_status(order_status) {
        return Ok(0);
    }
    let order_id = order_id.trim();
    let client_order_id = client_order_id.trim();
    if order_id.is_empty() && client_order_id.is_empty() {
        return Ok(0);
    }
    reject_ambiguous_planned_exit_identity_scope(
        pool,
        mode,
        strategy_id,
        inst_id,
        PlannedExitIdentityKind::Entry,
        order_id,
        client_order_id,
    )
    .await?;
    let now = chrono::Utc::now().to_rfc3339();
    let mode_filter_sql = if mode.is_some() { "AND mode = ?" } else { "" };
    let strategy_filter_sql = if strategy_id.is_some() {
        "AND strategy_id = ?"
    } else {
        ""
    };
    let inst_id_filter_sql = if inst_id.is_some() {
        "AND UPPER(TRIM(inst_id)) = UPPER(TRIM(?))"
    } else {
        ""
    };
    let sql = format!(
        r#"
        UPDATE live_execution_plans
        SET status = 'cancelled',
            last_error = ?,
            updated_at = ?
        WHERE status IN ('scheduled', 'exit_processing')
          {mode_filter_sql}
          {strategy_filter_sql}
          {inst_id_filter_sql}
          AND (
            (LENGTH(TRIM(?)) > 0 AND entry_order_id = ?)
            OR (LENGTH(TRIM(?)) > 0 AND entry_client_order_id = ?)
          )
        "#
    );
    let mut query = sqlx::query(&sql).bind(reason).bind(&now);
    if let Some(mode) = mode {
        query = query.bind(mode.trim());
    }
    if let Some(strategy_id) = strategy_id {
        query = query.bind(strategy_id.trim());
    }
    if let Some(inst_id) = inst_id {
        query = query.bind(inst_id.trim());
    }
    let result = query
        .bind(order_id)
        .bind(order_id)
        .bind(client_order_id)
        .bind(client_order_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

#[cfg(test)]
pub(in crate::live_strategy) async fn mark_live_planned_exit_order_terminal(
    pool: &SqlitePool,
    order_id: &str,
    client_order_id: &str,
    order_status: &str,
    reason: &str,
    retry_at: i64,
) -> AppResult<u64> {
    mark_live_planned_exit_order_terminal_scoped(
        pool,
        None,
        None,
        None,
        order_id,
        client_order_id,
        order_status,
        reason,
        retry_at,
    )
    .await
}

pub(in crate::live_strategy) async fn mark_live_planned_exit_order_terminal_for_mode(
    pool: &SqlitePool,
    mode: &str,
    inst_id: &str,
    order_id: &str,
    client_order_id: &str,
    order_status: &str,
    reason: &str,
    retry_at: i64,
) -> AppResult<u64> {
    let mode = mode.trim();
    let inst_id = inst_id.trim();
    if mode.is_empty() || inst_id.is_empty() {
        return Ok(0);
    }
    mark_live_planned_exit_order_terminal_scoped(
        pool,
        Some(mode),
        None,
        Some(inst_id),
        order_id,
        client_order_id,
        order_status,
        reason,
        retry_at,
    )
    .await
}

pub(in crate::live_strategy) async fn mark_live_planned_exit_order_terminal_for_strategy(
    pool: &SqlitePool,
    mode: &str,
    strategy_id: &str,
    inst_id: &str,
    order_id: &str,
    client_order_id: &str,
    order_status: &str,
    reason: &str,
    retry_at: i64,
) -> AppResult<u64> {
    let mode = mode.trim();
    let strategy_id = strategy_id.trim();
    let inst_id = inst_id.trim();
    if mode.is_empty() || strategy_id.is_empty() || inst_id.is_empty() {
        return Ok(0);
    }
    mark_live_planned_exit_order_terminal_scoped(
        pool,
        Some(mode),
        Some(strategy_id),
        Some(inst_id),
        order_id,
        client_order_id,
        order_status,
        reason,
        retry_at,
    )
    .await
}

async fn mark_live_planned_exit_order_terminal_scoped(
    pool: &SqlitePool,
    mode: Option<&str>,
    strategy_id: Option<&str>,
    inst_id: Option<&str>,
    order_id: &str,
    client_order_id: &str,
    order_status: &str,
    reason: &str,
    retry_at: i64,
) -> AppResult<u64> {
    let Some(plan_status) = planned_exit_order_status_resolution(order_status) else {
        return Ok(0);
    };
    let order_id = order_id.trim();
    let client_order_id = client_order_id.trim();
    if order_id.is_empty() && client_order_id.is_empty() {
        return Ok(0);
    }
    reject_ambiguous_planned_exit_identity_scope(
        pool,
        mode,
        strategy_id,
        inst_id,
        PlannedExitIdentityKind::Exit,
        order_id,
        client_order_id,
    )
    .await?;
    let now = chrono::Utc::now().to_rfc3339();
    let history_entry = exit_order_history_entry(order_id, client_order_id);
    let mode_filter_sql = if mode.is_some() { "AND mode = ?" } else { "" };
    let strategy_filter_sql = if strategy_id.is_some() {
        "AND strategy_id = ?"
    } else {
        ""
    };
    let inst_id_filter_sql = if inst_id.is_some() {
        "AND UPPER(TRIM(inst_id)) = UPPER(TRIM(?))"
    } else {
        ""
    };
    let result = match plan_status {
        PlannedExitOrderResolution::Filled => {
            let sql = format!(
                r#"
                UPDATE live_execution_plans
                SET status = 'exit_filled',
                    exit_order_id = CASE WHEN LENGTH(TRIM(?)) > 0 THEN ? ELSE exit_order_id END,
                    exit_client_order_id = CASE WHEN LENGTH(TRIM(?)) > 0 THEN ? ELSE exit_client_order_id END,
                    exit_order_history = CASE
                      WHEN LENGTH(?) > 0 THEN exit_order_history || ?
                      ELSE exit_order_history
                    END,
                    last_error = ?,
                    updated_at = ?
                WHERE status IN ('exit_submitted', 'exit_processing')
                  {mode_filter_sql}
                  {strategy_filter_sql}
                  {inst_id_filter_sql}
                  AND (
                    (LENGTH(TRIM(?)) > 0 AND exit_order_id = ?)
                    OR (LENGTH(TRIM(?)) > 0 AND exit_client_order_id = ?)
                  )
                "#
            );
            let mut query = sqlx::query(&sql)
                .bind(order_id)
                .bind(order_id)
                .bind(client_order_id)
                .bind(client_order_id)
                .bind(&history_entry)
                .bind(&history_entry)
                .bind(reason)
                .bind(&now);
            if let Some(mode) = mode {
                query = query.bind(mode.trim());
            }
            if let Some(strategy_id) = strategy_id {
                query = query.bind(strategy_id.trim());
            }
            if let Some(inst_id) = inst_id {
                query = query.bind(inst_id.trim());
            }
            query
                .bind(order_id)
                .bind(order_id)
                .bind(client_order_id)
                .bind(client_order_id)
                .execute(pool)
                .await?
        }
        PlannedExitOrderResolution::Retry => {
            let sql = format!(
                r#"
                UPDATE live_execution_plans
                SET status = 'scheduled',
                    exit_order_id = CASE WHEN LENGTH(TRIM(?)) > 0 THEN ? ELSE exit_order_id END,
                    exit_client_order_id = CASE WHEN LENGTH(TRIM(?)) > 0 THEN ? ELSE exit_client_order_id END,
                    exit_order_history = CASE
                      WHEN LENGTH(?) > 0 THEN exit_order_history || ?
                      ELSE exit_order_history
                    END,
                    attempt_count = attempt_count + 1,
                    next_attempt_at = ?,
                    last_error = ?,
                    updated_at = ?
                WHERE status IN ('exit_submitted', 'exit_processing')
                  {mode_filter_sql}
                  {strategy_filter_sql}
                  {inst_id_filter_sql}
                  AND (
                    (LENGTH(TRIM(?)) > 0 AND exit_order_id = ?)
                    OR (LENGTH(TRIM(?)) > 0 AND exit_client_order_id = ?)
                  )
                "#
            );
            let mut query = sqlx::query(&sql)
                .bind(order_id)
                .bind(order_id)
                .bind(client_order_id)
                .bind(client_order_id)
                .bind(&history_entry)
                .bind(&history_entry)
                .bind(retry_at.max(0))
                .bind(reason)
                .bind(&now);
            if let Some(mode) = mode {
                query = query.bind(mode.trim());
            }
            if let Some(strategy_id) = strategy_id {
                query = query.bind(strategy_id.trim());
            }
            if let Some(inst_id) = inst_id {
                query = query.bind(inst_id.trim());
            }
            query
                .bind(order_id)
                .bind(order_id)
                .bind(client_order_id)
                .bind(client_order_id)
                .execute(pool)
                .await?
        }
    };
    Ok(result.rows_affected())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlannedExitOrderResolution {
    Filled,
    Retry,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlannedExitIdentityKind {
    Entry,
    Exit,
}

impl PlannedExitIdentityKind {
    fn label(self) -> &'static str {
        match self {
            Self::Entry => "入口订单",
            Self::Exit => "计划退出平仓单",
        }
    }

    fn status_filter_sql(self) -> &'static str {
        match self {
            Self::Entry => "status IN ('scheduled', 'exit_processing')",
            Self::Exit => "status IN ('exit_submitted', 'exit_processing')",
        }
    }

    fn order_id_column(self) -> &'static str {
        match self {
            Self::Entry => "entry_order_id",
            Self::Exit => "exit_order_id",
        }
    }

    fn client_order_id_column(self) -> &'static str {
        match self {
            Self::Entry => "entry_client_order_id",
            Self::Exit => "exit_client_order_id",
        }
    }
}

async fn reject_ambiguous_planned_exit_identity_scope(
    pool: &SqlitePool,
    mode: Option<&str>,
    strategy_id: Option<&str>,
    inst_id: Option<&str>,
    identity_kind: PlannedExitIdentityKind,
    order_id: &str,
    client_order_id: &str,
) -> AppResult<()> {
    if order_id.trim().is_empty() && client_order_id.trim().is_empty() {
        return Ok(());
    }
    let mode_filter_sql = if mode.is_some() { "AND mode = ?" } else { "" };
    let strategy_filter_sql = if strategy_id.is_some() {
        "AND strategy_id = ?"
    } else {
        ""
    };
    let inst_id_filter_sql = if inst_id.is_some() {
        "AND UPPER(TRIM(inst_id)) = UPPER(TRIM(?))"
    } else {
        ""
    };
    let status_filter_sql = identity_kind.status_filter_sql();
    let order_id_column = identity_kind.order_id_column();
    let client_order_id_column = identity_kind.client_order_id_column();
    let sql = format!(
        r#"
        SELECT id
        FROM live_execution_plans
        WHERE {status_filter_sql}
          {mode_filter_sql}
          {strategy_filter_sql}
          {inst_id_filter_sql}
          AND (
            (LENGTH(TRIM(?)) > 0 AND {order_id_column} = ?)
            OR (LENGTH(TRIM(?)) > 0 AND {client_order_id_column} = ?)
          )
        ORDER BY id ASC
        LIMIT 2
        "#
    );
    let mut query = sqlx::query(&sql);
    if let Some(mode) = mode {
        query = query.bind(mode.trim());
    }
    if let Some(strategy_id) = strategy_id {
        query = query.bind(strategy_id.trim());
    }
    if let Some(inst_id) = inst_id {
        query = query.bind(inst_id.trim());
    }
    let rows = query
        .bind(order_id.trim())
        .bind(order_id.trim())
        .bind(client_order_id.trim())
        .bind(client_order_id.trim())
        .fetch_all(pool)
        .await?;
    if rows.len() <= 1 {
        return Ok(());
    }
    let ids = rows
        .iter()
        .map(|row| row.get::<i64, _>("id").to_string())
        .collect::<Vec<_>>()
        .join(",");
    Err(AppError::Validation(format!(
        "实时策略{}身份命中多个计划退出记录，已拒绝批量更新: mode={}, strategy_id={}, inst_id={}, order_id={}, client_order_id={}, matched_plan_ids={}",
        identity_kind.label(),
        mode.unwrap_or("").trim(),
        strategy_id.unwrap_or("").trim(),
        inst_id.unwrap_or("").trim(),
        order_id.trim(),
        client_order_id.trim(),
        ids
    )))
}

fn planned_exit_order_status_resolution(order_status: &str) -> Option<PlannedExitOrderResolution> {
    match order_status.trim().to_ascii_lowercase().as_str() {
        "filled" | "fully_filled" | "fully-filled" => Some(PlannedExitOrderResolution::Filled),
        "canceled" | "cancelled" | "rejected" | "reject" => Some(PlannedExitOrderResolution::Retry),
        _ => None,
    }
}

fn is_entry_order_failed_without_fill_status(order_status: &str) -> bool {
    matches!(
        order_status.trim().to_ascii_lowercase().as_str(),
        "canceled" | "cancelled" | "rejected" | "reject"
    )
}

fn execution_plan_row_to_json(row: SqliteRow) -> AppResult<Value> {
    Ok(json!({
        "id": row.try_get::<i64, _>("id")?,
        "plan_key": required_plan_text(&row, "plan_key")?,
        "strategy_id": required_plan_text(&row, "strategy_id")?,
        "strategy_name": required_plan_text(&row, "strategy_name")?,
        "mode": required_plan_text(&row, "mode")?,
        "entry_run_id": required_plan_text(&row, "entry_run_id")?,
        "exit_run_id": plan_text(&row, "exit_run_id")?,
        "symbol": required_plan_text(&row, "symbol")?,
        "inst_id": required_plan_text(&row, "inst_id")?,
        "inst_type": required_plan_text(&row, "inst_type")?,
        "timeframe": required_plan_text(&row, "timeframe")?,
        "entry_order_id": plan_text(&row, "entry_order_id")?,
        "entry_client_order_id": plan_text(&row, "entry_client_order_id")?,
        "entry_timestamp": required_positive_i64_row(&row, "entry_action_timestamp")?,
        "entry_side": required_plan_text(&row, "entry_side")?,
        "entry_price": required_positive_f64_row(&row, "entry_price")?,
        "close_side": required_plan_text(&row, "close_side")?,
        "planned_exit_time": required_positive_i64_row(&row, "planned_exit_time")?,
        "planned_exit_reason": required_plan_text(&row, "planned_exit_reason")?,
        "planned_exit_contract": required_plan_text(&row, "planned_exit_contract")?,
        "status": required_plan_text(&row, "status")?,
        "exit_order_id": plan_text(&row, "exit_order_id")?,
        "exit_client_order_id": plan_text(&row, "exit_client_order_id")?,
        "exit_order_history": plan_text(&row, "exit_order_history")?,
        "attempt_count": non_negative_i64_row(&row, "attempt_count")?,
        "next_attempt_at": non_negative_i64_row(&row, "next_attempt_at")?,
        "last_error": plan_text(&row, "last_error")?,
        "created_at": required_plan_text(&row, "created_at")?,
        "updated_at": required_plan_text(&row, "updated_at")?,
    }))
}

fn required_plan_text(row: &SqliteRow, column: &str) -> AppResult<String> {
    let value = row.try_get::<String, _>(column)?;
    if value.trim().is_empty() {
        Err(AppError::Runtime(format!(
            "live execution plan {column} 不能为空"
        )))
    } else {
        Ok(value)
    }
}

fn plan_text(row: &SqliteRow, column: &str) -> AppResult<String> {
    row.try_get::<String, _>(column).map_err(Into::into)
}

fn required_positive_i64_row(row: &SqliteRow, column: &str) -> AppResult<i64> {
    let Some(value) = row.try_get::<Option<i64>, _>(column)? else {
        return Err(AppError::Runtime(format!(
            "live execution plan {column} 缺失"
        )));
    };
    if value > 0 {
        Ok(value)
    } else {
        Err(AppError::Runtime(format!(
            "live execution plan {column} 必须是正整数"
        )))
    }
}

fn non_negative_i64_row(row: &SqliteRow, column: &str) -> AppResult<i64> {
    let value = row.try_get::<i64, _>(column)?;
    if value >= 0 {
        Ok(value)
    } else {
        Err(AppError::Runtime(format!(
            "live execution plan {column} 不能为负数"
        )))
    }
}

fn required_positive_f64_row(row: &SqliteRow, column: &str) -> AppResult<f64> {
    let value = row
        .try_get::<Option<f64>, _>(column)?
        .ok_or_else(|| AppError::Runtime(format!("live execution plan {column} 缺失")))?;
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(AppError::Runtime(format!(
            "live execution plan {column} 必须是有限正数"
        )))
    }
}

fn non_empty_or<'a>(value: &'a str, fallback: &'static str) -> &'a str {
    let value = value.trim();
    if value.is_empty() {
        fallback
    } else {
        value
    }
}

fn exit_order_history_entry(order_id: &str, client_order_id: &str) -> String {
    let order_id = sanitize_exit_order_history_value(order_id);
    let client_order_id = sanitize_exit_order_history_value(client_order_id);
    if order_id.is_empty() && client_order_id.is_empty() {
        String::new()
    } else {
        format!("{order_id}\t{client_order_id}\n")
    }
}

fn sanitize_exit_order_history_value(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|item| !matches!(item, '\r' | '\n' | '\t'))
        .collect()
}

fn validate_planned_exit_plan(
    config: &LiveStrategyConfig,
    action_record: &StrategyActionRecord,
    entry_side: &str,
    close_side: &str,
    planned_exit: &StrategyPlannedExitIntent,
) -> AppResult<()> {
    if config.strategy_id.trim().is_empty() || config.symbol.trim().is_empty() {
        return Err(AppError::Validation(
            "planned exit 缺少 strategy_id 或 symbol".to_string(),
        ));
    }
    if action_record.timestamp <= 0 {
        return Err(AppError::Validation(
            "planned exit 缺少有效 entry action timestamp".to_string(),
        ));
    }
    if !action_record.price.is_finite() || action_record.price <= 0.0 {
        return Err(AppError::Validation(
            "planned exit 缺少有效 entry price".to_string(),
        ));
    }
    if !matches!(
        entry_side.trim().to_ascii_lowercase().as_str(),
        "buy" | "sell"
    ) {
        return Err(AppError::Validation(
            "planned exit 缺少有效开仓方向".to_string(),
        ));
    }
    if !matches!(
        close_side.trim().to_ascii_lowercase().as_str(),
        "buy" | "sell"
    ) {
        return Err(AppError::Validation(
            "planned exit 缺少有效平仓方向".to_string(),
        ));
    }
    if planned_exit.timestamp <= 0 {
        return Err(AppError::Validation(
            "planned exit 缺少有效退出时间".to_string(),
        ));
    }
    Ok(())
}

fn planned_exit_plan_key(
    config: &LiveStrategyConfig,
    action_record: &StrategyActionRecord,
    entry_side: &str,
    close_side: &str,
    planned_exit: &StrategyPlannedExitIntent,
    entry_order_id: &str,
    entry_client_order_id: &str,
) -> String {
    let entry_identity = if !entry_order_id.trim().is_empty() {
        format!("ord={}", entry_order_id.trim())
    } else if !entry_client_order_id.trim().is_empty() {
        format!("cl={}", entry_client_order_id.trim())
    } else {
        format!("ts={}", action_record.timestamp)
    };
    format!(
        "{}|{}|{}|{}|{}|{}|{}",
        config.mode.trim().to_ascii_lowercase(),
        config.strategy_id.trim(),
        config.symbol.trim().to_ascii_uppercase(),
        entry_identity,
        planned_exit.timestamp,
        entry_side.trim().to_ascii_lowercase(),
        close_side.trim().to_ascii_lowercase()
    )
}

fn plan_from_row(row: SqliteRow) -> AppResult<LivePlannedExitPlan> {
    Ok(LivePlannedExitPlan {
        id: row.try_get::<i64, _>("id")?,
        strategy_id: required_plan_text(&row, "strategy_id")?,
        strategy_name: required_plan_text(&row, "strategy_name")?,
        mode: required_plan_text(&row, "mode")?,
        symbol: required_plan_text(&row, "symbol")?,
        inst_type: required_plan_text(&row, "inst_type")?,
        timeframe: required_plan_text(&row, "timeframe")?,
        entry_order_id: plan_text(&row, "entry_order_id")?,
        entry_client_order_id: plan_text(&row, "entry_client_order_id")?,
        entry_price: required_positive_f64_row(&row, "entry_price")?,
        close_side: required_plan_text(&row, "close_side")?,
        planned_exit_time: required_positive_i64_row(&row, "planned_exit_time")?,
        planned_exit_reason: required_plan_text(&row, "planned_exit_reason")?,
        planned_exit_contract: required_plan_text(&row, "planned_exit_contract")?,
        exit_order_id: plan_text(&row, "exit_order_id")?,
        exit_client_order_id: plan_text(&row, "exit_client_order_id")?,
        exit_order_history: plan_text(&row, "exit_order_history")?,
        attempt_count: non_negative_i64_row(&row, "attempt_count")?,
    })
}

fn planned_exit_order_sync_candidate_from_row(
    row: SqliteRow,
) -> AppResult<LivePlannedExitOrderSyncCandidate> {
    Ok(LivePlannedExitOrderSyncCandidate {
        id: row.try_get::<i64, _>("id")?,
        symbol: required_plan_text(&row, "inst_id")?,
        inst_type: required_plan_text(&row, "inst_type")?,
        order_id: plan_text(&row, "exit_order_id")?,
        client_order_id: plan_text(&row, "exit_client_order_id")?,
        updated_at_ms: required_positive_i64_row(&row, "updated_at_ms")?,
    })
}

#[cfg(test)]
mod tests;
