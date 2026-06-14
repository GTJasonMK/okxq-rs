use std::collections::BTreeSet;

use sqlx::{
    query::Query,
    sqlite::{SqliteArguments, SqliteRow},
    Row, Sqlite, SqlitePool,
};

use crate::error::{AppError, AppResult};

use super::identity::{
    ensure_fill_identity_available, ensure_parent_identity_available,
    find_unique_parent_identity_match, identity_scope_for_action,
    order_identity_participates_in_sync, query_order_identity_record_context,
    LiveOrderIdentityScope,
};

#[derive(Clone, Debug)]
pub(in crate::live_strategy) struct LiveOrderSyncCandidate {
    pub(in crate::live_strategy) id: i64,
    pub(in crate::live_strategy) symbol: String,
    pub(in crate::live_strategy) inst_type: String,
    pub(in crate::live_strategy) order_id: String,
    pub(in crate::live_strategy) client_order_id: String,
    pub(in crate::live_strategy) status: String,
    pub(in crate::live_strategy) created_at_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::live_strategy) struct LiveAlgoOrderIdentityContext {
    pub(in crate::live_strategy) id: i64,
    pub(in crate::live_strategy) symbol: String,
    pub(in crate::live_strategy) inst_type: String,
    pub(in crate::live_strategy) order_type: String,
    pub(in crate::live_strategy) order_id: String,
    pub(in crate::live_strategy) client_order_id: String,
    pub(in crate::live_strategy) status: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::live_strategy) struct LiveOrderIdentityContext {
    pub(in crate::live_strategy) id: i64,
    pub(in crate::live_strategy) symbol: String,
    pub(in crate::live_strategy) inst_type: String,
    pub(in crate::live_strategy) order_type: String,
    pub(in crate::live_strategy) order_id: String,
    pub(in crate::live_strategy) client_order_id: String,
    pub(in crate::live_strategy) status: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::live_strategy) struct LiveFillSyncScope {
    pub(in crate::live_strategy) symbol: String,
    pub(in crate::live_strategy) inst_type: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::live_strategy) struct LiveOrderExchangeState {
    pub(in crate::live_strategy) status: String,
    pub(in crate::live_strategy) success: bool,
    pub(in crate::live_strategy) error_message: String,
    pub(in crate::live_strategy) order_id: String,
    pub(in crate::live_strategy) client_order_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::live_strategy) struct LiveAlgoOrderActualState {
    pub(in crate::live_strategy) status: String,
    pub(in crate::live_strategy) success: bool,
    pub(in crate::live_strategy) error_message: String,
    pub(in crate::live_strategy) actual_order_id: String,
    pub(in crate::live_strategy) actual_client_order_id: String,
}

type LiveOrderStateQuery<'q> = Query<'q, Sqlite, SqliteArguments<'q>>;

#[derive(Clone, Copy, Debug)]
enum LiveOrderSyncCandidateKind {
    Exchange,
    Algo,
}

const LIVE_ORDER_SYNC_ACTIVE_STATUSES_SQL: &str = r#"
            'submitting', 'submit_unknown', 'submitted', 'pending', 'open', 'live',
            'partially_filled', 'partial-filled', 'partially-filled',
            'cancel_requested', 'modify_requested'
"#;

const LIVE_ALGO_ORDER_SYNC_ACTIVE_STATUSES_SQL: &str = r#"
            'algo_submitting', 'algo_submitted', 'algo_submit_unknown', 'algo_live',
            'algo_cancel_requested', 'algo_modify_requested'
"#;

const LIVE_ATTACHED_ALGO_PENDING_ACTIVATION_STATUSES_SQL: &str = r#"
            'algo_submitting', 'algo_submitted', 'algo_submit_unknown'
"#;

const LIVE_ORDER_IDENTITY_MATCH_SQL: &str = r#"
          AND (
            (LENGTH(TRIM(?)) > 0 AND order_id = ?)
            OR (LENGTH(TRIM(?)) > 0 AND client_order_id = ?)
          )
"#;

const LIVE_ORDER_EXCHANGE_STATE_SET_SQL: &str = r#"
        SET status = ?,
            success = ?,
            error_message = ?,
            order_id = CASE WHEN LENGTH(TRIM(?)) > 0 THEN ? ELSE order_id END,
            client_order_id = CASE WHEN LENGTH(TRIM(?)) > 0 THEN ? ELSE client_order_id END
"#;

const LIVE_ORDER_EXCHANGE_STATE_CHANGED_SQL: &str = r#"
          AND (
            status IS NOT ?
            OR success IS NOT ?
            OR error_message IS NOT ?
            OR (LENGTH(TRIM(?)) > 0 AND order_id IS NOT ?)
            OR (LENGTH(TRIM(?)) > 0 AND client_order_id IS NOT ?)
          )
"#;

impl LiveOrderSyncCandidateKind {
    fn action_filter_sql(self) -> &'static str {
        match self {
            Self::Exchange => "AND action <> 'place_risk_order'",
            Self::Algo => "AND action = 'place_risk_order'",
        }
    }

    fn active_statuses_sql(self) -> &'static str {
        match self {
            Self::Exchange => LIVE_ORDER_SYNC_ACTIVE_STATUSES_SQL,
            Self::Algo => LIVE_ALGO_ORDER_SYNC_ACTIVE_STATUSES_SQL,
        }
    }

    fn max_limit(self) -> i64 {
        match self {
            Self::Exchange => 200,
            Self::Algo => 100,
        }
    }

    fn identity_scope(self) -> LiveOrderIdentityScope {
        match self {
            Self::Exchange => LiveOrderIdentityScope::ExchangeParent,
            Self::Algo => LiveOrderIdentityScope::AlgoParent,
        }
    }
}

fn bind_live_order_exchange_state<'q>(
    query: LiveOrderStateQuery<'q>,
    state: &'q LiveOrderExchangeState,
) -> LiveOrderStateQuery<'q> {
    query
        .bind(&state.status)
        .bind(if state.success { 1_i64 } else { 0_i64 })
        .bind(&state.error_message)
        .bind(&state.order_id)
        .bind(&state.order_id)
        .bind(&state.client_order_id)
        .bind(&state.client_order_id)
}

fn bind_live_order_exchange_state_changed_filter<'q>(
    query: LiveOrderStateQuery<'q>,
    state: &'q LiveOrderExchangeState,
) -> LiveOrderStateQuery<'q> {
    query
        .bind(&state.status)
        .bind(if state.success { 1_i64 } else { 0_i64 })
        .bind(&state.error_message)
        .bind(&state.order_id)
        .bind(&state.order_id)
        .bind(&state.client_order_id)
        .bind(&state.client_order_id)
}

pub(in crate::live_strategy) async fn query_live_order_sync_candidates(
    pool: &SqlitePool,
    mode: &str,
    strategy_id: &str,
    limit: i64,
) -> AppResult<Vec<LiveOrderSyncCandidate>> {
    query_live_order_sync_candidates_for_kind(
        pool,
        mode,
        strategy_id,
        limit,
        LiveOrderSyncCandidateKind::Exchange,
    )
    .await
}

pub(in crate::live_strategy) async fn query_live_algo_order_sync_candidates(
    pool: &SqlitePool,
    mode: &str,
    strategy_id: &str,
    limit: i64,
) -> AppResult<Vec<LiveOrderSyncCandidate>> {
    query_live_order_sync_candidates_for_kind(
        pool,
        mode,
        strategy_id,
        limit,
        LiveOrderSyncCandidateKind::Algo,
    )
    .await
}

async fn query_live_order_sync_candidates_for_kind(
    pool: &SqlitePool,
    mode: &str,
    strategy_id: &str,
    limit: i64,
    kind: LiveOrderSyncCandidateKind,
) -> AppResult<Vec<LiveOrderSyncCandidate>> {
    let action_filter_sql = kind.action_filter_sql();
    let active_statuses_sql = kind.active_statuses_sql();
    let sql = format!(
        r#"
        SELECT id, inst_id, inst_type, order_id, client_order_id, status,
               CAST(strftime('%s', created_at) AS INTEGER) * 1000 AS created_at_ms
        FROM live_order_records
        WHERE mode = ?
          AND strategy_id = ?
          {action_filter_sql}
          AND LOWER(TRIM(status)) IN (
{active_statuses_sql}
          )
          AND (
            LENGTH(TRIM(order_id)) > 0
            OR LENGTH(TRIM(client_order_id)) > 0
          )
        ORDER BY created_at DESC, id DESC
        LIMIT ?
        "#
    );
    let rows = sqlx::query(&sql)
        .bind(mode.trim())
        .bind(strategy_id.trim())
        .bind(limit.clamp(1, kind.max_limit()))
        .fetch_all(pool)
        .await?;

    rows.into_iter()
        .map(live_order_sync_candidate_from_row)
        .collect::<AppResult<Vec<_>>>()
}

fn live_order_sync_candidate_from_row(row: SqliteRow) -> AppResult<LiveOrderSyncCandidate> {
    Ok(LiveOrderSyncCandidate {
        id: positive_i64_column(&row, "id")?,
        symbol: required_text_column(&row, "inst_id")?,
        inst_type: required_upper_text_column(&row, "inst_type")?,
        order_id: row.try_get::<String, _>("order_id")?,
        client_order_id: row.try_get::<String, _>("client_order_id")?,
        status: required_text_column(&row, "status")?,
        created_at_ms: positive_optional_i64_column(&row, "created_at_ms")?,
    })
}

pub(in crate::live_strategy) async fn query_live_algo_order_identity_context(
    pool: &SqlitePool,
    mode: &str,
    order_id: &str,
    client_order_id: &str,
) -> AppResult<Option<LiveAlgoOrderIdentityContext>> {
    query_live_algo_order_identity_context_scoped(pool, mode, None, order_id, client_order_id).await
}

pub(in crate::live_strategy) async fn query_live_algo_order_identity_context_for_symbol(
    pool: &SqlitePool,
    mode: &str,
    inst_id: &str,
    order_id: &str,
    client_order_id: &str,
) -> AppResult<Option<LiveAlgoOrderIdentityContext>> {
    let inst_id = inst_id.trim();
    if inst_id.is_empty() {
        return Ok(None);
    }
    query_live_algo_order_identity_context_scoped(
        pool,
        mode,
        Some(inst_id),
        order_id,
        client_order_id,
    )
    .await
}

async fn query_live_algo_order_identity_context_scoped(
    pool: &SqlitePool,
    mode: &str,
    inst_id: Option<&str>,
    order_id: &str,
    client_order_id: &str,
) -> AppResult<Option<LiveAlgoOrderIdentityContext>> {
    if order_id.trim().is_empty() && client_order_id.trim().is_empty() {
        return Ok(None);
    }
    if inst_id.is_none() {
        reject_ambiguous_live_order_identity(
            pool,
            mode,
            order_id,
            client_order_id,
            LiveOrderSyncCandidateKind::Algo,
        )
        .await?;
    }
    let inst_id_filter_sql = if inst_id.is_some() {
        "AND UPPER(TRIM(inst_id)) = UPPER(TRIM(?))"
    } else {
        ""
    };
    let sql = format!(
        r#"
        SELECT id, inst_id, inst_type, order_type, order_id, client_order_id, status
        FROM live_order_records
        WHERE mode = ?
          {inst_id_filter_sql}
          AND action = 'place_risk_order'
          AND LOWER(TRIM(status)) IN (
{LIVE_ALGO_ORDER_SYNC_ACTIVE_STATUSES_SQL}
          )
{LIVE_ORDER_IDENTITY_MATCH_SQL}
        ORDER BY
          CASE
            WHEN LENGTH(TRIM(?)) > 0 AND order_id = ? THEN 0
            ELSE 1
          END,
          created_at DESC,
          id DESC
        LIMIT 1
        "#
    );
    let mut query = sqlx::query(&sql).bind(mode.trim());
    if let Some(inst_id) = inst_id {
        query = query.bind(inst_id.trim());
    }
    let row = query
        .bind(order_id.trim())
        .bind(order_id.trim())
        .bind(client_order_id.trim())
        .bind(client_order_id.trim())
        .bind(order_id.trim())
        .bind(order_id.trim())
        .fetch_optional(pool)
        .await?;

    row.map(live_algo_order_identity_context_from_row)
        .transpose()
}

pub(in crate::live_strategy) async fn query_live_order_identity_context(
    pool: &SqlitePool,
    mode: &str,
    order_id: &str,
    client_order_id: &str,
) -> AppResult<Option<LiveOrderIdentityContext>> {
    query_live_order_identity_context_scoped(pool, mode, None, order_id, client_order_id).await
}

pub(in crate::live_strategy) async fn query_live_order_identity_context_for_symbol(
    pool: &SqlitePool,
    mode: &str,
    inst_id: &str,
    order_id: &str,
    client_order_id: &str,
) -> AppResult<Option<LiveOrderIdentityContext>> {
    let inst_id = inst_id.trim();
    if inst_id.is_empty() {
        return Ok(None);
    }
    query_live_order_identity_context_scoped(pool, mode, Some(inst_id), order_id, client_order_id)
        .await
}

async fn query_live_order_identity_context_scoped(
    pool: &SqlitePool,
    mode: &str,
    inst_id: Option<&str>,
    order_id: &str,
    client_order_id: &str,
) -> AppResult<Option<LiveOrderIdentityContext>> {
    if order_id.trim().is_empty() && client_order_id.trim().is_empty() {
        return Ok(None);
    }
    if inst_id.is_none() {
        reject_ambiguous_live_order_identity(
            pool,
            mode,
            order_id,
            client_order_id,
            LiveOrderSyncCandidateKind::Exchange,
        )
        .await?;
    }
    let inst_id_filter_sql = if inst_id.is_some() {
        "AND UPPER(TRIM(inst_id)) = UPPER(TRIM(?))"
    } else {
        ""
    };
    let sql = format!(
        r#"
        SELECT id, inst_id, inst_type, order_type, order_id, client_order_id, status
        FROM live_order_records
        WHERE mode = ?
          {inst_id_filter_sql}
          AND action <> 'place_risk_order'
{LIVE_ORDER_IDENTITY_MATCH_SQL}
        ORDER BY
          CASE
            WHEN LOWER(TRIM(status)) IN (
{LIVE_ORDER_SYNC_ACTIVE_STATUSES_SQL}
            ) THEN 0
            ELSE 1
          END,
          CASE
            WHEN LENGTH(TRIM(?)) > 0 AND order_id = ? THEN 0
            ELSE 1
          END,
          created_at DESC,
          id DESC
        LIMIT 1
        "#
    );
    let mut query = sqlx::query(&sql).bind(mode.trim());
    if let Some(inst_id) = inst_id {
        query = query.bind(inst_id.trim());
    }
    let row = query
        .bind(order_id.trim())
        .bind(order_id.trim())
        .bind(client_order_id.trim())
        .bind(client_order_id.trim())
        .bind(order_id.trim())
        .bind(order_id.trim())
        .fetch_optional(pool)
        .await?;

    row.map(live_order_identity_context_from_row).transpose()
}

fn live_algo_order_identity_context_from_row(
    row: SqliteRow,
) -> AppResult<LiveAlgoOrderIdentityContext> {
    Ok(LiveAlgoOrderIdentityContext {
        id: positive_i64_column(&row, "id")?,
        symbol: required_text_column(&row, "inst_id")?,
        inst_type: required_upper_text_column(&row, "inst_type")?,
        order_type: required_text_column(&row, "order_type")?,
        order_id: row.try_get::<String, _>("order_id")?,
        client_order_id: row.try_get::<String, _>("client_order_id")?,
        status: required_text_column(&row, "status")?,
    })
}

fn live_order_identity_context_from_row(row: SqliteRow) -> AppResult<LiveOrderIdentityContext> {
    Ok(LiveOrderIdentityContext {
        id: positive_i64_column(&row, "id")?,
        symbol: required_text_column(&row, "inst_id")?,
        inst_type: required_upper_text_column(&row, "inst_type")?,
        order_type: required_text_column(&row, "order_type")?,
        order_id: row.try_get::<String, _>("order_id")?,
        client_order_id: row.try_get::<String, _>("client_order_id")?,
        status: required_text_column(&row, "status")?,
    })
}

fn required_text_column(row: &SqliteRow, column: &str) -> AppResult<String> {
    let value = row.try_get::<String, _>(column)?;
    if value.trim().is_empty() {
        return Err(AppError::Runtime(format!("live order {column} 不能为空")));
    }
    Ok(value)
}

fn required_upper_text_column(row: &SqliteRow, column: &str) -> AppResult<String> {
    Ok(required_text_column(row, column)?
        .trim()
        .to_ascii_uppercase())
}

fn positive_i64_column(row: &SqliteRow, column: &str) -> AppResult<i64> {
    let value = row.try_get::<i64, _>(column)?;
    if value <= 0 {
        return Err(AppError::Runtime(format!(
            "live order {column} 必须为正整数"
        )));
    }
    Ok(value)
}

fn positive_optional_i64_column(row: &SqliteRow, column: &str) -> AppResult<i64> {
    let Some(value) = row.try_get::<Option<i64>, _>(column)? else {
        return Err(AppError::Runtime(format!("live order {column} 不能为空")));
    };
    if value <= 0 {
        return Err(AppError::Runtime(format!(
            "live order {column} 必须为正整数"
        )));
    }
    Ok(value)
}

async fn reject_ambiguous_live_order_identity(
    pool: &SqlitePool,
    mode: &str,
    order_id: &str,
    client_order_id: &str,
    kind: LiveOrderSyncCandidateKind,
) -> AppResult<()> {
    let action_filter_sql = kind.action_filter_sql();
    let status_filter_sql = match kind {
        LiveOrderSyncCandidateKind::Exchange => "",
        LiveOrderSyncCandidateKind::Algo => {
            r#"
          AND LOWER(TRIM(status)) IN (
            'algo_submitting', 'algo_submitted', 'algo_submit_unknown', 'algo_live',
            'algo_cancel_requested', 'algo_modify_requested'
          )
"#
        }
    };
    let sql = format!(
        r#"
        SELECT UPPER(TRIM(inst_id)) AS inst_id
        FROM live_order_records
        WHERE mode = ?
          {action_filter_sql}
          {status_filter_sql}
{LIVE_ORDER_IDENTITY_MATCH_SQL}
          AND LENGTH(TRIM(inst_id)) > 0
        GROUP BY UPPER(TRIM(inst_id))
        ORDER BY inst_id ASC
        LIMIT 3
        "#
    );
    let rows = sqlx::query(&sql)
        .bind(mode.trim())
        .bind(order_id.trim())
        .bind(order_id.trim())
        .bind(client_order_id.trim())
        .bind(client_order_id.trim())
        .fetch_all(pool)
        .await?;
    if rows.len() <= 1 {
        return Ok(());
    }
    let symbols = rows
        .iter()
        .map(|row| row.get::<String, _>("inst_id"))
        .collect::<Vec<_>>()
        .join(", ");
    let order_kind = match kind {
        LiveOrderSyncCandidateKind::Exchange => "普通订单",
        LiveOrderSyncCandidateKind::Algo => "保护单",
    };
    Err(AppError::Validation(format!(
        "实时策略 {order_kind} 身份在多个交易对下命中: {symbols}；请在 cancel_order/modify_order action 中显式提供 symbol 以避免误撤或误改"
    )))
}

pub(in crate::live_strategy) async fn update_live_order_exchange_state(
    pool: &SqlitePool,
    order_id: i64,
    state: &LiveOrderExchangeState,
) -> AppResult<bool> {
    let Some(context) = query_order_identity_record_context(pool, order_id).await? else {
        return Ok(false);
    };
    if order_identity_participates_in_sync(&state.status, state.success) {
        ensure_parent_identity_available(
            pool,
            &context.mode,
            &context.inst_id,
            identity_scope_for_action(&context.action),
            &state.order_id,
            &state.client_order_id,
            Some(order_id),
        )
        .await?;
    }
    let sql = format!(
        r#"
        UPDATE live_order_records
        {LIVE_ORDER_EXCHANGE_STATE_SET_SQL}
        WHERE id = ?
        {LIVE_ORDER_EXCHANGE_STATE_CHANGED_SQL}
        "#
    );
    let query = bind_live_order_exchange_state(sqlx::query(&sql), state).bind(order_id);
    let result = bind_live_order_exchange_state_changed_filter(query, state)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub(in crate::live_strategy) async fn update_live_exchange_order_state_by_identity_and_symbol(
    pool: &SqlitePool,
    mode: &str,
    inst_id: &str,
    order_id: &str,
    client_order_id: &str,
    state: &LiveOrderExchangeState,
) -> AppResult<u64> {
    update_live_order_exchange_state_by_identity_and_symbol_for_kind(
        pool,
        mode,
        inst_id,
        order_id,
        client_order_id,
        state,
        LiveOrderSyncCandidateKind::Exchange,
    )
    .await
}

pub(in crate::live_strategy) async fn update_live_algo_order_exchange_state_by_identity_and_symbol(
    pool: &SqlitePool,
    mode: &str,
    inst_id: &str,
    order_id: &str,
    client_order_id: &str,
    state: &LiveOrderExchangeState,
) -> AppResult<u64> {
    update_live_order_exchange_state_by_identity_and_symbol_for_kind(
        pool,
        mode,
        inst_id,
        order_id,
        client_order_id,
        state,
        LiveOrderSyncCandidateKind::Algo,
    )
    .await
}

pub(in crate::live_strategy) async fn mark_live_attached_algo_orders_parent_terminal_unfilled(
    pool: &SqlitePool,
    mode: &str,
    inst_id: &str,
    parent_order_id: &str,
    parent_client_order_id: &str,
    parent_status: &str,
    parent_error_message: &str,
) -> AppResult<u64> {
    let inst_id = inst_id.trim();
    let parent_order_id = parent_order_id.trim();
    let parent_client_order_id = parent_client_order_id.trim();
    if inst_id.is_empty() || (parent_order_id.is_empty() && parent_client_order_id.is_empty()) {
        return Ok(0);
    }
    let status = attached_algo_terminal_status_for_parent(parent_status).to_string();
    let error_message = attached_algo_parent_terminal_message(
        parent_status,
        parent_error_message,
        parent_order_id,
        parent_client_order_id,
    );
    let sql = format!(
        r#"
        UPDATE live_order_records
        SET status = ?,
            success = 0,
            error_message = ?
        WHERE mode = ?
          AND UPPER(TRIM(inst_id)) = UPPER(TRIM(?))
          AND action = 'place_risk_order'
          AND LOWER(TRIM(status)) IN (
{LIVE_ATTACHED_ALGO_PENDING_ACTIVATION_STATUSES_SQL}
          )
          AND (
            (LENGTH(TRIM(?)) > 0 AND parent_order_id = ?)
            OR (LENGTH(TRIM(?)) > 0 AND parent_client_order_id = ?)
          )
          AND (
            status IS NOT ?
            OR success IS NOT 0
            OR error_message IS NOT ?
          )
        "#
    );
    let result = sqlx::query(&sql)
        .bind(&status)
        .bind(&error_message)
        .bind(mode.trim())
        .bind(inst_id)
        .bind(parent_order_id)
        .bind(parent_order_id)
        .bind(parent_client_order_id)
        .bind(parent_client_order_id)
        .bind(&status)
        .bind(&error_message)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

fn attached_algo_terminal_status_for_parent(parent_status: &str) -> &'static str {
    match parent_status.trim().to_ascii_lowercase().as_str() {
        "canceled" | "cancelled" => "algo_canceled",
        _ => "algo_failed",
    }
}

fn attached_algo_parent_terminal_message(
    parent_status: &str,
    parent_error_message: &str,
    parent_order_id: &str,
    parent_client_order_id: &str,
) -> String {
    let parent_status = parent_status.trim();
    let parent_error_message = parent_error_message.trim();
    let suffix = if parent_error_message.is_empty() {
        String::new()
    } else {
        format!("; parent_error={parent_error_message}")
    };
    format!(
        "父订单未成交终止，随单保护单未激活: parent_status={parent_status}; parent_order_id={parent_order_id}; parent_client_order_id={parent_client_order_id}{suffix}"
    )
}

async fn update_live_order_exchange_state_by_identity_and_symbol_for_kind(
    pool: &SqlitePool,
    mode: &str,
    inst_id: &str,
    order_id: &str,
    client_order_id: &str,
    state: &LiveOrderExchangeState,
    kind: LiveOrderSyncCandidateKind,
) -> AppResult<u64> {
    let inst_id = inst_id.trim();
    if inst_id.is_empty() {
        return Ok(0);
    }
    let Some(row_id) = find_unique_parent_identity_match(
        pool,
        mode,
        inst_id,
        kind.identity_scope(),
        order_id,
        client_order_id,
    )
    .await?
    else {
        return Ok(0);
    };
    if order_identity_participates_in_sync(&state.status, state.success) {
        ensure_parent_identity_available(
            pool,
            mode,
            inst_id,
            kind.identity_scope(),
            &state.order_id,
            &state.client_order_id,
            Some(row_id),
        )
        .await?;
    }
    update_live_order_exchange_state(pool, row_id, state)
        .await
        .map(u64::from)
}

pub(in crate::live_strategy) async fn update_live_algo_order_actual_state_by_identity_and_symbol(
    pool: &SqlitePool,
    mode: &str,
    inst_id: &str,
    algo_order_id: &str,
    algo_client_order_id: &str,
    state: &LiveAlgoOrderActualState,
) -> AppResult<u64> {
    let inst_id = inst_id.trim();
    if inst_id.is_empty() {
        return Ok(0);
    }
    let Some(row_id) = find_unique_parent_identity_match(
        pool,
        mode,
        inst_id,
        LiveOrderIdentityScope::AlgoParent,
        algo_order_id,
        algo_client_order_id,
    )
    .await?
    else {
        return Ok(0);
    };
    if order_identity_participates_in_sync(&state.status, state.success) {
        ensure_fill_identity_available(
            pool,
            mode,
            inst_id,
            &state.actual_order_id,
            &state.actual_client_order_id,
            Some(row_id),
        )
        .await?;
    }
    let sql = r#"
        UPDATE live_order_records
        SET status = ?,
            success = ?,
            error_message = ?,
            actual_order_id = CASE
              WHEN LENGTH(TRIM(?)) > 0 THEN ?
              ELSE actual_order_id
            END,
            actual_client_order_id = CASE
              WHEN LENGTH(TRIM(?)) > 0 THEN ?
              ELSE actual_client_order_id
            END
        WHERE id = ?
          AND (
            status IS NOT ?
            OR success IS NOT ?
            OR error_message IS NOT ?
            OR (LENGTH(TRIM(?)) > 0 AND actual_order_id IS NOT ?)
            OR (LENGTH(TRIM(?)) > 0 AND actual_client_order_id IS NOT ?)
          )
        "#;
    let query = sqlx::query(sql)
        .bind(&state.status)
        .bind(if state.success { 1_i64 } else { 0_i64 })
        .bind(&state.error_message)
        .bind(&state.actual_order_id)
        .bind(&state.actual_order_id)
        .bind(&state.actual_client_order_id)
        .bind(&state.actual_client_order_id)
        .bind(row_id);
    let result = query
        .bind(&state.status)
        .bind(if state.success { 1_i64 } else { 0_i64 })
        .bind(&state.error_message)
        .bind(&state.actual_order_id)
        .bind(&state.actual_order_id)
        .bind(&state.actual_client_order_id)
        .bind(&state.actual_client_order_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

pub(in crate::live_strategy) async fn query_live_fill_sync_scopes(
    pool: &SqlitePool,
    mode: &str,
    strategy_id: &str,
    limit: i64,
) -> AppResult<Vec<LiveFillSyncScope>> {
    let rows = sqlx::query(
        r#"
        SELECT DISTINCT inst_type, inst_id
        FROM live_order_records
        WHERE mode = ?
          AND strategy_id = ?
          AND (
            success = 1
            OR LOWER(TRIM(status)) IN (
              'submitting', 'submit_unknown', 'submitted', 'pending', 'open', 'live',
              'partially_filled', 'partial-filled', 'partially-filled',
              'cancel_requested', 'modify_requested',
              'algo_submitting', 'algo_submitted', 'algo_submit_unknown', 'algo_live',
              'algo_cancel_requested', 'algo_modify_requested'
            )
          )
          AND LENGTH(TRIM(inst_id)) > 0
          AND (
            LENGTH(TRIM(order_id)) > 0
            OR LENGTH(TRIM(client_order_id)) > 0
          )
          AND LOWER(TRIM(status)) IN (
            'submit_unknown', 'submitted', 'pending', 'open', 'live',
            'partially_filled', 'partial-filled', 'partially-filled',
            'filled', 'fully_filled', 'fully-filled',
            'canceled', 'cancelled',
            'cancel_requested', 'modify_requested',
            'algo_submitted', 'algo_submit_unknown', 'algo_live',
            'algo_cancel_requested', 'algo_modify_requested',
            'algo_effective', 'algo_partially_effective', 'algo_canceled'
          )
        ORDER BY inst_type ASC, inst_id ASC
        LIMIT ?
        "#,
    )
    .bind(mode.trim())
    .bind(strategy_id.trim())
    .bind(limit.clamp(1, 50))
    .fetch_all(pool)
    .await?;

    let mut seen = BTreeSet::new();
    let mut scopes = Vec::new();
    for row in rows {
        let symbol = required_upper_text_column(&row, "inst_id")?;
        let inst_type = required_upper_text_column(&row, "inst_type")?;
        if seen.insert((inst_type.clone(), symbol.clone())) {
            scopes.push(LiveFillSyncScope { symbol, inst_type });
        }
    }
    Ok(scopes)
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::json;

    use super::*;
    use crate::live_strategy::{
        arrival::ArrivalQuote, storage::orders::insert::insert_live_exchange_order,
        types::LiveStrategyConfig,
    };

    #[tokio::test]
    async fn live_order_sync_candidates_only_include_non_terminal_exchange_orders() {
        let db_path = temp_db_path("live_order_sync_candidates");
        let pool = crate::storage::connect_and_migrate(&db_path)
            .await
            .expect("test database should migrate");
        let config = test_config();
        insert_live_exchange_order(
            &pool,
            &config,
            "buy",
            "market",
            1.0,
            100.0,
            "open_position",
            "submitted",
            true,
            "submitted",
            "run-sync",
            1_780_000_000_000,
            ArrivalQuote::default(),
            "ord-submitted",
            "clsubmitted",
        )
        .await
        .expect("submitted order should insert");
        insert_live_exchange_order(
            &pool,
            &config,
            "buy",
            "market",
            1.0,
            100.0,
            "open_position",
            "submit_unknown",
            false,
            "submit unknown",
            "run-sync",
            1_780_000_000_000,
            ArrivalQuote::default(),
            "",
            "clsubmitunknown",
        )
        .await
        .expect("submit unknown order should insert");
        insert_live_exchange_order(
            &pool,
            &config,
            "open_position",
            "market",
            1.0,
            100.0,
            "cancel_order",
            "cancel_requested",
            true,
            "cancel requested",
            "run-sync",
            1_780_000_000_000,
            ArrivalQuote::default(),
            "ord-cancel-requested",
            "clcancelrequested",
        )
        .await
        .expect("cancel requested order should insert");
        insert_live_exchange_order(
            &pool,
            &config,
            "open_position",
            "limit",
            1.0,
            100.0,
            "modify_order",
            "modify_requested",
            true,
            "modify requested",
            "run-sync",
            1_780_000_000_000,
            ArrivalQuote::default(),
            "ord-modify-requested",
            "clmodifyrequested",
        )
        .await
        .expect("modify requested order should insert");
        insert_live_exchange_order(
            &pool,
            &config,
            "sell",
            "stop_market",
            1.0,
            94.0,
            "place_risk_order",
            "algo_submitting",
            false,
            "algo submitting",
            "run-sync",
            1_780_000_000_000,
            ArrivalQuote::default(),
            "",
            "clalgosubmitting",
        )
        .await
        .expect("algo submitting order should insert");
        insert_live_exchange_order(
            &pool,
            &config,
            "sell",
            "stop_market",
            1.0,
            94.0,
            "place_risk_order",
            "algo_submitted",
            true,
            "algo submitted",
            "run-sync",
            1_780_000_000_000,
            ArrivalQuote::default(),
            "algo-submitted",
            "clalgosubmitted",
        )
        .await
        .expect("algo submitted order should insert");
        insert_live_exchange_order(
            &pool,
            &config,
            "sell",
            "take_profit_market",
            1.0,
            106.0,
            "place_risk_order",
            "algo_modify_requested",
            true,
            "algo modify requested",
            "run-sync",
            1_780_000_000_000,
            ArrivalQuote::default(),
            "algo-modify-requested",
            "clalgomodifyrequested",
        )
        .await
        .expect("algo modify requested order should insert");
        insert_live_exchange_order(
            &pool,
            &config,
            "buy",
            "market",
            1.0,
            100.0,
            "open_position",
            "filled",
            true,
            "filled",
            "run-sync",
            1_780_000_000_001,
            ArrivalQuote::default(),
            "ord-filled",
            "clfilled",
        )
        .await
        .expect("filled order should insert");

        let candidates =
            query_live_order_sync_candidates(&pool, &config.mode, &config.strategy_id, 10)
                .await
                .expect("candidate query should run");

        assert_eq!(candidates.len(), 4);
        assert!(candidates.iter().all(|item| item.inst_type == "SWAP"));
        assert!(candidates
            .iter()
            .any(|item| item.order_id == "ord-submitted" && item.status == "submitted"));
        assert!(candidates.iter().any(
            |item| item.client_order_id == "clsubmitunknown" && item.status == "submit_unknown"
        ));
        assert!(candidates.iter().any(
            |item| item.order_id == "ord-cancel-requested" && item.status == "cancel_requested"
        ));
        assert!(candidates.iter().any(
            |item| item.order_id == "ord-modify-requested" && item.status == "modify_requested"
        ));
        assert!(
            candidates
                .iter()
                .all(|item| item.order_id != "algo-submitted"),
            "algoId must not be queried with the ordinary order detail API"
        );
        assert!(
            candidates
                .iter()
                .all(|item| item.order_id != "algo-modify-requested"),
            "algo modify requests must stay out of ordinary order sync"
        );
        let algo_candidates =
            query_live_algo_order_sync_candidates(&pool, &config.mode, &config.strategy_id, 10)
                .await
                .expect("algo candidate query should run");
        assert_eq!(algo_candidates.len(), 3);
        assert!(algo_candidates.iter().all(|item| item.inst_type == "SWAP"));
        assert!(algo_candidates.iter().any(|item| {
            item.order_id.is_empty()
                && item.client_order_id == "clalgosubmitting"
                && item.status == "algo_submitting"
        }));
        assert!(algo_candidates.iter().any(|item| {
            item.order_id == "algo-submitted" && item.client_order_id == "clalgosubmitted"
        }));
        assert!(algo_candidates.iter().any(|item| {
            item.order_id == "algo-modify-requested"
                && item.client_order_id == "clalgomodifyrequested"
        }));
        assert!(
            query_live_algo_order_identity_context(&pool, &config.mode, "algo-submitted", "",)
                .await
                .expect("algo identity lookup should run")
                .is_some()
        );
        let algo_context = query_live_algo_order_identity_context(
            &pool,
            &config.mode,
            "",
            "clalgomodifyrequested",
        )
        .await
        .expect("algo identity context query should run")
        .expect("algo identity context should be found");
        assert_eq!(algo_context.order_id, "algo-modify-requested");
        assert_eq!(algo_context.inst_type, "SWAP");
        assert_eq!(algo_context.order_type, "take_profit_market");
        assert_eq!(algo_context.status, "algo_modify_requested");
        assert!(
            query_live_algo_order_identity_context(&pool, &config.mode, "ord-submitted", "",)
                .await
                .expect("ordinary identity lookup should run")
                .is_none()
        );

        pool.close().await;
        if let Some(parent) = db_path.parent() {
            std::fs::remove_dir_all(parent).ok();
        }
    }

    #[tokio::test]
    async fn live_order_exchange_state_update_marks_filled_order_terminal() {
        let db_path = temp_db_path("live_order_exchange_state_update");
        let pool = crate::storage::connect_and_migrate(&db_path)
            .await
            .expect("test database should migrate");
        let config = test_config();
        let row_id = insert_live_exchange_order(
            &pool,
            &config,
            "sell",
            "market",
            1.0,
            100.0,
            "close_position",
            "submitted",
            true,
            "submitted",
            "run-sync-update",
            1_780_000_000_000,
            ArrivalQuote::default(),
            "ord-exit",
            "clexit",
        )
        .await
        .expect("order should insert");

        let changed = update_live_order_exchange_state(
            &pool,
            row_id,
            &LiveOrderExchangeState {
                status: "filled".to_string(),
                success: true,
                error_message: "OKX order filled".to_string(),
                order_id: "ord-exit".to_string(),
                client_order_id: "clexit".to_string(),
            },
        )
        .await
        .expect("update should run");

        assert!(changed);
        let row = sqlx::query(
            "SELECT status, success, error_message FROM live_order_records WHERE id = ?",
        )
        .bind(row_id)
        .fetch_one(&pool)
        .await
        .expect("updated order should exist");
        assert_eq!(row.get::<String, _>("status"), "filled");
        assert_eq!(row.get::<i64, _>("success"), 1);
        assert_eq!(row.get::<String, _>("error_message"), "OKX order filled");

        pool.close().await;
        if let Some(parent) = db_path.parent() {
            std::fs::remove_dir_all(parent).ok();
        }
    }

    #[tokio::test]
    async fn identity_state_updates_are_scoped_between_exchange_and_algo_orders() {
        let db_path = temp_db_path("live_order_identity_scoped_updates");
        let pool = crate::storage::connect_and_migrate(&db_path)
            .await
            .expect("test database should migrate");
        let config = test_config();
        insert_live_exchange_order(
            &pool,
            &config,
            "buy",
            "limit",
            1.0,
            100.0,
            "open_position",
            "submitted",
            true,
            "ordinary submitted",
            "run-identity-scope",
            1_780_000_000_000,
            ArrivalQuote::default(),
            "shared-order-id",
            "shared-client-id",
        )
        .await
        .expect("ordinary order should insert");
        insert_live_exchange_order(
            &pool,
            &config,
            "sell",
            "stop_market",
            1.0,
            94.0,
            "place_risk_order",
            "algo_submitted",
            true,
            "algo submitted",
            "run-identity-scope",
            1_780_000_000_000,
            ArrivalQuote::default(),
            "shared-order-id",
            "shared-client-id",
        )
        .await
        .expect("algo order should insert");

        let ordinary_changed = update_live_exchange_order_state_by_identity_and_symbol(
            &pool,
            &config.mode,
            &config.symbol,
            "shared-order-id",
            "shared-client-id",
            &LiveOrderExchangeState {
                status: "cancel_requested".to_string(),
                success: true,
                error_message: "ordinary cancel requested".to_string(),
                order_id: "shared-order-id".to_string(),
                client_order_id: "shared-client-id".to_string(),
            },
        )
        .await
        .expect("ordinary scoped update should run");
        assert_eq!(ordinary_changed, 1);
        let rows = sqlx::query(
            r#"
            SELECT action, status
            FROM live_order_records
            WHERE order_id = ?
            ORDER BY action ASC
            "#,
        )
        .bind("shared-order-id")
        .fetch_all(&pool)
        .await
        .expect("rows should query");
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|row| {
            row.get::<String, _>("action") == "open_position"
                && row.get::<String, _>("status") == "cancel_requested"
        }));
        assert!(rows.iter().any(|row| {
            row.get::<String, _>("action") == "place_risk_order"
                && row.get::<String, _>("status") == "algo_submitted"
        }));

        let algo_changed = update_live_algo_order_exchange_state_by_identity_and_symbol(
            &pool,
            &config.mode,
            &config.symbol,
            "shared-order-id",
            "shared-client-id",
            &LiveOrderExchangeState {
                status: "algo_cancel_requested".to_string(),
                success: true,
                error_message: "algo cancel requested".to_string(),
                order_id: "shared-order-id".to_string(),
                client_order_id: "shared-client-id".to_string(),
            },
        )
        .await
        .expect("algo scoped update should run");
        assert_eq!(algo_changed, 1);
        let rows = sqlx::query(
            r#"
            SELECT action, status
            FROM live_order_records
            WHERE order_id = ?
            ORDER BY action ASC
            "#,
        )
        .bind("shared-order-id")
        .fetch_all(&pool)
        .await
        .expect("rows should query");
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|row| {
            row.get::<String, _>("action") == "open_position"
                && row.get::<String, _>("status") == "cancel_requested"
        }));
        assert!(rows.iter().any(|row| {
            row.get::<String, _>("action") == "place_risk_order"
                && row.get::<String, _>("status") == "algo_cancel_requested"
        }));

        pool.close().await;
        if let Some(parent) = db_path.parent() {
            std::fs::remove_dir_all(parent).ok();
        }
    }

    #[tokio::test]
    async fn algo_actual_identity_update_rejects_fill_identity_collision() {
        let db_path = temp_db_path("algo_actual_identity_collision");
        let pool = crate::storage::connect_and_migrate(&db_path)
            .await
            .expect("test database should migrate");
        let config = test_config();
        insert_live_exchange_order(
            &pool,
            &config,
            "buy",
            "limit",
            1.0,
            100.0,
            "open_position",
            "submitted",
            true,
            "ordinary submitted",
            "run-actual-collision-ordinary",
            1_780_000_000_000,
            ArrivalQuote::default(),
            "actual-collision-order",
            "actualcollisionclient",
        )
        .await
        .expect("ordinary order should insert");
        insert_live_exchange_order(
            &pool,
            &config,
            "sell",
            "stop_market",
            1.0,
            94.0,
            "place_risk_order",
            "algo_submitted",
            true,
            "algo submitted",
            "run-actual-collision-algo",
            1_780_000_000_001,
            ArrivalQuote::default(),
            "algo-collision-order",
            "algocollisionclient",
        )
        .await
        .expect("algo order should insert");

        let error = update_live_algo_order_actual_state_by_identity_and_symbol(
            &pool,
            &config.mode,
            &config.symbol,
            "algo-collision-order",
            "algocollisionclient",
            &LiveAlgoOrderActualState {
                status: "algo_effective".to_string(),
                success: true,
                error_message: "actual order filled".to_string(),
                actual_order_id: "actual-collision-order".to_string(),
                actual_client_order_id: "newactualclient".to_string(),
            },
        )
        .await
        .expect_err("actual order identity collision must be rejected");

        assert!(error.to_string().contains("成交身份"));
        assert!(error.to_string().contains("已拒绝写入或更新"));

        pool.close().await;
        if let Some(parent) = db_path.parent() {
            std::fs::remove_dir_all(parent).ok();
        }
    }

    #[tokio::test]
    async fn unscoped_identity_context_queries_reject_cross_symbol_collisions() {
        let db_path = temp_db_path("live_order_identity_context_ambiguous_symbols");
        let pool = crate::storage::connect_and_migrate(&db_path)
            .await
            .expect("test database should migrate");
        let btc_config = test_config();
        let mut eth_config = btc_config.clone();
        eth_config.symbol = "ETH-USDT-SWAP".to_string();

        insert_live_exchange_order(
            &pool,
            &eth_config,
            "buy",
            "limit",
            1.0,
            200.0,
            "open_position",
            "submitted",
            true,
            "eth submitted",
            "run-identity-context-eth",
            1_780_000_000_000,
            ArrivalQuote::default(),
            "",
            "shared-context-exchange",
        )
        .await
        .expect("eth ordinary order should insert");
        insert_live_exchange_order(
            &pool,
            &btc_config,
            "buy",
            "limit",
            1.0,
            100.0,
            "open_position",
            "submitted",
            true,
            "btc submitted",
            "run-identity-context-btc",
            1_780_000_000_001,
            ArrivalQuote::default(),
            "",
            "shared-context-exchange",
        )
        .await
        .expect("btc ordinary order should insert");
        insert_live_exchange_order(
            &pool,
            &eth_config,
            "sell",
            "stop_market",
            1.0,
            194.0,
            "place_risk_order",
            "algo_submitting",
            true,
            "eth algo submitting",
            "run-algo-identity-context-eth",
            1_780_000_000_000,
            ArrivalQuote::default(),
            "",
            "shared-context-algo",
        )
        .await
        .expect("eth algo order should insert");
        insert_live_exchange_order(
            &pool,
            &btc_config,
            "sell",
            "stop_market",
            1.0,
            94.0,
            "place_risk_order",
            "algo_submitting",
            true,
            "btc algo submitting",
            "run-algo-identity-context-btc",
            1_780_000_000_001,
            ArrivalQuote::default(),
            "",
            "shared-context-algo",
        )
        .await
        .expect("btc algo order should insert");

        let ordinary_error = query_live_order_identity_context(
            &pool,
            &btc_config.mode,
            "",
            "shared-context-exchange",
        )
        .await
        .expect_err("unscoped ordinary identity collision should fail");
        assert!(ordinary_error.to_string().contains("普通订单"));
        assert!(ordinary_error.to_string().contains("多个交易对"));
        assert!(ordinary_error.to_string().contains("显式提供 symbol"));
        let scoped_ordinary = query_live_order_identity_context_for_symbol(
            &pool,
            &btc_config.mode,
            "ETH-USDT-SWAP",
            "",
            "shared-context-exchange",
        )
        .await
        .expect("scoped ordinary identity query should run")
        .expect("scoped ordinary identity should resolve");
        assert_eq!(scoped_ordinary.symbol, "ETH-USDT-SWAP");
        assert_eq!(scoped_ordinary.client_order_id, "shared-context-exchange");

        let algo_error = query_live_algo_order_identity_context(
            &pool,
            &btc_config.mode,
            "",
            "shared-context-algo",
        )
        .await
        .expect_err("unscoped algo identity collision should fail");
        assert!(algo_error.to_string().contains("保护单"));
        assert!(algo_error.to_string().contains("多个交易对"));
        assert!(algo_error.to_string().contains("显式提供 symbol"));
        let scoped_algo = query_live_algo_order_identity_context_for_symbol(
            &pool,
            &btc_config.mode,
            "ETH-USDT-SWAP",
            "",
            "shared-context-algo",
        )
        .await
        .expect("scoped algo identity query should run")
        .expect("scoped algo identity should resolve");
        assert_eq!(scoped_algo.symbol, "ETH-USDT-SWAP");
        assert_eq!(scoped_algo.client_order_id, "shared-context-algo");

        pool.close().await;
        if let Some(parent) = db_path.parent() {
            std::fs::remove_dir_all(parent).ok();
        }
    }

    #[tokio::test]
    async fn live_fill_sync_symbols_include_recent_syncable_exchange_orders() {
        let db_path = temp_db_path("live_fill_sync_symbols");
        let pool = crate::storage::connect_and_migrate(&db_path)
            .await
            .expect("test database should migrate");
        let config = test_config();
        insert_live_exchange_order(
            &pool,
            &config,
            "sell",
            "market",
            1.0,
            100.0,
            "close_position",
            "filled",
            true,
            "filled",
            "run-fill-symbols",
            1_780_000_000_000,
            ArrivalQuote::default(),
            "ord-filled",
            "clfilled",
        )
        .await
        .expect("filled order should insert");
        let mut eth_config = config.clone();
        eth_config.symbol = "ETH-USDT-SWAP".to_string();
        insert_live_exchange_order(
            &pool,
            &eth_config,
            "sell",
            "market",
            1.0,
            100.0,
            "close_position",
            "canceled",
            true,
            "OKX order canceled after partial fill",
            "run-fill-symbols",
            1_780_000_000_001,
            ArrivalQuote::default(),
            "ord-canceled-partial",
            "clcanceledpartial",
        )
        .await
        .expect("partial canceled order should insert");
        insert_live_exchange_order(
            &pool,
            &config,
            "buy",
            "market",
            1.0,
            100.0,
            "open_position",
            "risk_blocked",
            false,
            "blocked",
            "run-fill-symbols",
            1_780_000_000_002,
            ArrivalQuote::default(),
            "ord-blocked",
            "clblocked",
        )
        .await
        .expect("blocked order should insert");
        let mut bnb_config = config.clone();
        bnb_config.symbol = "BNB-USDT-SWAP".to_string();
        insert_live_exchange_order(
            &pool,
            &bnb_config,
            "buy",
            "market",
            1.0,
            100.0,
            "open_position",
            "submit_unknown",
            false,
            "submit unknown pending exchange sync",
            "run-fill-symbols",
            1_780_000_000_003,
            ArrivalQuote::default(),
            "",
            "clsubmitunknown",
        )
        .await
        .expect("submit unknown order should insert");
        let mut doge_config = config.clone();
        doge_config.symbol = "DOGE-USDT-SWAP".to_string();
        insert_live_exchange_order(
            &pool,
            &doge_config,
            "buy",
            "market",
            1.0,
            100.0,
            "open_position",
            "canceled",
            false,
            "canceled without fill should not drive fill sync",
            "run-fill-symbols",
            1_780_000_000_004,
            ArrivalQuote::default(),
            "ord-canceled-empty",
            "clcanceledempty",
        )
        .await
        .expect("unfilled canceled order should insert");
        let mut ada_config = config.clone();
        ada_config.symbol = "ADA-USDT-SWAP".to_string();
        insert_live_exchange_order(
            &pool,
            &ada_config,
            "sell",
            "stop_market",
            1.0,
            94.0,
            "place_risk_order",
            "algo_cancel_requested",
            false,
            "algo cancel requested",
            "run-fill-symbols",
            1_780_000_000_005,
            ArrivalQuote::default(),
            "algo-cancel-requested",
            "clalgocancelrequested",
        )
        .await
        .expect("algo cancel requested order should insert");
        let mut ltc_config = config.clone();
        ltc_config.symbol = "LTC-USDT-SWAP".to_string();
        insert_live_exchange_order(
            &pool,
            &ltc_config,
            "sell",
            "stop_market",
            1.0,
            94.0,
            "place_risk_order",
            "algo_submit_unknown",
            false,
            "algo submit unknown pending exchange sync",
            "run-fill-symbols",
            1_780_000_000_006,
            ArrivalQuote::default(),
            "",
            "clalgosubmitunknown",
        )
        .await
        .expect("algo submit unknown order should insert");
        let mut sol_config = config.clone();
        sol_config.symbol = "SOL-USDT-SWAP".to_string();
        insert_live_exchange_order(
            &pool,
            &sol_config,
            "buy",
            "market",
            1.0,
            100.0,
            "open_position",
            "cancel_requested",
            false,
            "cancel requested",
            "run-fill-symbols",
            1_780_000_000_007,
            ArrivalQuote::default(),
            "ord-cancel-requested",
            "clcancelrequested",
        )
        .await
        .expect("cancel requested order should insert");
        let mut xrp_config = config.clone();
        xrp_config.symbol = "XRP-USDT-SWAP".to_string();
        insert_live_exchange_order(
            &pool,
            &xrp_config,
            "buy",
            "limit",
            1.0,
            100.0,
            "open_position",
            "modify_requested",
            false,
            "modify requested",
            "run-fill-symbols",
            1_780_000_000_008,
            ArrivalQuote::default(),
            "ord-modify-requested",
            "clmodifyrequested",
        )
        .await
        .expect("modify requested order should insert");
        let mut zrx_config = config.clone();
        zrx_config.symbol = "ZRX-USDT-SWAP".to_string();
        insert_live_exchange_order(
            &pool,
            &zrx_config,
            "sell",
            "stop_market",
            1.0,
            94.0,
            "place_risk_order",
            "algo_modify_requested",
            false,
            "algo modify requested",
            "run-fill-symbols",
            1_780_000_000_009,
            ArrivalQuote::default(),
            "algo-modify-requested",
            "clalgomodifyrequested",
        )
        .await
        .expect("algo modify requested order should insert");

        let scopes = query_live_fill_sync_scopes(&pool, &config.mode, &config.strategy_id, 10)
            .await
            .expect("scope query should run");

        assert_eq!(
            scopes,
            vec![
                LiveFillSyncScope {
                    symbol: "ADA-USDT-SWAP".to_string(),
                    inst_type: "SWAP".to_string(),
                },
                LiveFillSyncScope {
                    symbol: "BNB-USDT-SWAP".to_string(),
                    inst_type: "SWAP".to_string(),
                },
                LiveFillSyncScope {
                    symbol: "BTC-USDT-SWAP".to_string(),
                    inst_type: "SWAP".to_string(),
                },
                LiveFillSyncScope {
                    symbol: "ETH-USDT-SWAP".to_string(),
                    inst_type: "SWAP".to_string(),
                },
                LiveFillSyncScope {
                    symbol: "LTC-USDT-SWAP".to_string(),
                    inst_type: "SWAP".to_string(),
                },
                LiveFillSyncScope {
                    symbol: "SOL-USDT-SWAP".to_string(),
                    inst_type: "SWAP".to_string(),
                },
                LiveFillSyncScope {
                    symbol: "XRP-USDT-SWAP".to_string(),
                    inst_type: "SWAP".to_string(),
                },
                LiveFillSyncScope {
                    symbol: "ZRX-USDT-SWAP".to_string(),
                    inst_type: "SWAP".to_string(),
                },
            ]
        );

        pool.close().await;
        if let Some(parent) = db_path.parent() {
            std::fs::remove_dir_all(parent).ok();
        }
    }

    fn test_config() -> LiveStrategyConfig {
        LiveStrategyConfig {
            strategy_id: "live_order_sync_test".to_string(),
            strategy_name: "Live Order Sync Test".to_string(),
            symbol: "BTC-USDT-SWAP".to_string(),
            timeframe: "15m".to_string(),
            inst_type: "SWAP".to_string(),
            mode: "simulated".to_string(),
            initial_capital: 1000.0,
            position_size: 0.2,
            stop_loss: 0.0,
            take_profit: 0.0,
            risk_timeframe: "1m".to_string(),
            check_interval: 60,
            params: json!({}),
            project_root: PathBuf::from("."),
            risk_control_enabled: false,
            max_single_loss_ratio: 0.0,
            max_position_pct: 0.0,
            max_order_value: 0.0,
        }
    }

    fn temp_db_path(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir()
            .join(format!("okxq_{name}_{}_{}", std::process::id(), suffix))
            .join("market.db")
    }
}
