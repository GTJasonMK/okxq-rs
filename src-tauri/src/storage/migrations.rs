use sqlx::SqlitePool;

use super::{schema, sqlite_meta};

struct AddColumnMigration {
    table: &'static str,
    column: &'static str,
    statement: &'static str,
}

const ADDITIVE_COLUMNS: &[AddColumnMigration] = &[
    AddColumnMigration {
        table: "candles",
        column: "volume_quote",
        statement: "ALTER TABLE candles ADD COLUMN volume_quote REAL",
    },
    AddColumnMigration {
        table: "local_fills",
        column: "order_id",
        statement: "ALTER TABLE local_fills ADD COLUMN order_id TEXT DEFAULT ''",
    },
    AddColumnMigration {
        table: "local_fills",
        column: "client_order_id",
        statement: "ALTER TABLE local_fills ADD COLUMN client_order_id TEXT DEFAULT ''",
    },
    AddColumnMigration {
        table: "local_fills",
        column: "strategy_id",
        statement: "ALTER TABLE local_fills ADD COLUMN strategy_id TEXT DEFAULT ''",
    },
    AddColumnMigration {
        table: "local_fills",
        column: "run_id",
        statement: "ALTER TABLE local_fills ADD COLUMN run_id TEXT DEFAULT ''",
    },
    AddColumnMigration {
        table: "local_fills",
        column: "arrival_ts",
        statement: "ALTER TABLE local_fills ADD COLUMN arrival_ts INTEGER",
    },
    AddColumnMigration {
        table: "local_fills",
        column: "arrival_mid_px",
        statement: "ALTER TABLE local_fills ADD COLUMN arrival_mid_px REAL",
    },
    AddColumnMigration {
        table: "local_fills",
        column: "arrival_bid_px",
        statement: "ALTER TABLE local_fills ADD COLUMN arrival_bid_px REAL",
    },
    AddColumnMigration {
        table: "local_fills",
        column: "arrival_ask_px",
        statement: "ALTER TABLE local_fills ADD COLUMN arrival_ask_px REAL",
    },
    AddColumnMigration {
        table: "live_order_records",
        column: "inst_type",
        statement: "ALTER TABLE live_order_records ADD COLUMN inst_type TEXT NOT NULL DEFAULT ''",
    },
    AddColumnMigration {
        table: "live_order_records",
        column: "run_id",
        statement: "ALTER TABLE live_order_records ADD COLUMN run_id TEXT DEFAULT ''",
    },
    AddColumnMigration {
        table: "live_order_records",
        column: "action",
        statement: "ALTER TABLE live_order_records ADD COLUMN action TEXT DEFAULT ''",
    },
    AddColumnMigration {
        table: "live_order_records",
        column: "action_timestamp",
        statement: "ALTER TABLE live_order_records ADD COLUMN action_timestamp INTEGER",
    },
    AddColumnMigration {
        table: "live_order_records",
        column: "arrival_ts",
        statement: "ALTER TABLE live_order_records ADD COLUMN arrival_ts INTEGER",
    },
    AddColumnMigration {
        table: "live_order_records",
        column: "arrival_mid_px",
        statement: "ALTER TABLE live_order_records ADD COLUMN arrival_mid_px REAL",
    },
    AddColumnMigration {
        table: "live_order_records",
        column: "arrival_bid_px",
        statement: "ALTER TABLE live_order_records ADD COLUMN arrival_bid_px REAL",
    },
    AddColumnMigration {
        table: "live_order_records",
        column: "arrival_ask_px",
        statement: "ALTER TABLE live_order_records ADD COLUMN arrival_ask_px REAL",
    },
    AddColumnMigration {
        table: "live_order_records",
        column: "actual_order_id",
        statement: "ALTER TABLE live_order_records ADD COLUMN actual_order_id TEXT DEFAULT ''",
    },
    AddColumnMigration {
        table: "live_order_records",
        column: "actual_client_order_id",
        statement:
            "ALTER TABLE live_order_records ADD COLUMN actual_client_order_id TEXT DEFAULT ''",
    },
    AddColumnMigration {
        table: "live_order_records",
        column: "parent_order_id",
        statement: "ALTER TABLE live_order_records ADD COLUMN parent_order_id TEXT DEFAULT ''",
    },
    AddColumnMigration {
        table: "live_order_records",
        column: "parent_client_order_id",
        statement:
            "ALTER TABLE live_order_records ADD COLUMN parent_client_order_id TEXT DEFAULT ''",
    },
    AddColumnMigration {
        table: "okx_funding_rates",
        column: "inst_type",
        statement:
            "ALTER TABLE okx_funding_rates ADD COLUMN inst_type TEXT NOT NULL DEFAULT 'SWAP'",
    },
    AddColumnMigration {
        table: "okx_funding_rates",
        column: "realized_rate",
        statement: "ALTER TABLE okx_funding_rates ADD COLUMN realized_rate REAL",
    },
    AddColumnMigration {
        table: "okx_funding_rates",
        column: "method",
        statement: "ALTER TABLE okx_funding_rates ADD COLUMN method TEXT NOT NULL DEFAULT ''",
    },
    AddColumnMigration {
        table: "okx_funding_rates",
        column: "formula_type",
        statement: "ALTER TABLE okx_funding_rates ADD COLUMN formula_type TEXT NOT NULL DEFAULT ''",
    },
    AddColumnMigration {
        table: "okx_funding_rates",
        column: "payload_json",
        statement:
            "ALTER TABLE okx_funding_rates ADD COLUMN payload_json TEXT NOT NULL DEFAULT '{}'",
    },
    AddColumnMigration {
        table: "okx_funding_rates",
        column: "fetched_at",
        statement: "ALTER TABLE okx_funding_rates ADD COLUMN fetched_at TEXT NOT NULL DEFAULT ''",
    },
    AddColumnMigration {
        table: "sync_jobs",
        column: "start_ts",
        statement: "ALTER TABLE sync_jobs ADD COLUMN start_ts INTEGER",
    },
    AddColumnMigration {
        table: "sync_jobs",
        column: "end_ts",
        statement: "ALTER TABLE sync_jobs ADD COLUMN end_ts INTEGER",
    },
    AddColumnMigration {
        table: "sync_jobs",
        column: "repair_method",
        statement: "ALTER TABLE sync_jobs ADD COLUMN repair_method TEXT NOT NULL DEFAULT ''",
    },
    AddColumnMigration {
        table: "live_execution_plans",
        column: "exit_order_history",
        statement: "ALTER TABLE live_execution_plans ADD COLUMN exit_order_history TEXT NOT NULL DEFAULT ''",
    },
    AddColumnMigration {
        table: "live_execution_plans",
        column: "entry_action_timestamp",
        statement:
            "ALTER TABLE live_execution_plans ADD COLUMN entry_action_timestamp INTEGER NOT NULL DEFAULT 0",
    },
    AddColumnMigration {
        table: "live_execution_logs",
        column: "mode",
        statement: "ALTER TABLE live_execution_logs ADD COLUMN mode TEXT NOT NULL DEFAULT ''",
    },
    AddColumnMigration {
        table: "live_execution_logs",
        column: "strategy_id",
        statement:
            "ALTER TABLE live_execution_logs ADD COLUMN strategy_id TEXT NOT NULL DEFAULT ''",
    },
    AddColumnMigration {
        table: "live_execution_logs",
        column: "strategy_name",
        statement:
            "ALTER TABLE live_execution_logs ADD COLUMN strategy_name TEXT NOT NULL DEFAULT ''",
    },
    AddColumnMigration {
        table: "live_execution_logs",
        column: "symbol",
        statement: "ALTER TABLE live_execution_logs ADD COLUMN symbol TEXT NOT NULL DEFAULT ''",
    },
    AddColumnMigration {
        table: "live_execution_logs",
        column: "inst_type",
        statement: "ALTER TABLE live_execution_logs ADD COLUMN inst_type TEXT NOT NULL DEFAULT ''",
    },
    AddColumnMigration {
        table: "live_execution_logs",
        column: "timeframe",
        statement: "ALTER TABLE live_execution_logs ADD COLUMN timeframe TEXT NOT NULL DEFAULT ''",
    },
];

pub(super) async fn run(pool: &SqlitePool) -> anyhow::Result<()> {
    tracing::debug!("running database migrations");
    apply_statements(pool, schema::core_statements()).await?;
    apply_additive_columns(pool).await?;
    apply_conditional_indexes(pool).await?;
    apply_statements(pool, schema::post_migration_statements()).await?;
    tracing::debug!("database migrations completed");
    Ok(())
}

async fn apply_statements(
    pool: &SqlitePool,
    statements: impl Iterator<Item = &'static str>,
) -> anyhow::Result<()> {
    for statement in statements {
        sqlx::query(statement).execute(pool).await?;
    }
    Ok(())
}

async fn apply_additive_columns(pool: &SqlitePool) -> anyhow::Result<()> {
    for migration in ADDITIVE_COLUMNS {
        if sqlite_meta::table_exists(pool, migration.table).await?
            && !sqlite_meta::column_exists(pool, migration.table, migration.column).await?
        {
            sqlx::query(migration.statement).execute(pool).await?;
        }
    }
    Ok(())
}

async fn apply_conditional_indexes(pool: &SqlitePool) -> anyhow::Result<()> {
    if sqlite_meta::table_exists(pool, "okx_funding_rates").await?
        && sqlite_meta::column_exists(pool, "okx_funding_rates", "inst_type").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_okx_funding_inst_time ON okx_funding_rates(inst_id, inst_type, funding_time)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "sync_jobs").await?
        && sqlite_meta::column_exists(pool, "sync_jobs", "start_ts").await?
        && sqlite_meta::column_exists(pool, "sync_jobs", "end_ts").await?
        && sqlite_meta::column_exists(pool, "sync_jobs", "repair_method").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_sync_jobs_scope_range ON sync_jobs(inst_id, inst_type, source_timeframe, target_timeframes, mode, days, start_ts, end_ts, repair_method, status)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "sync_jobs").await?
        && sqlite_meta::column_exists(pool, "sync_jobs", "updated_at").await?
        && sqlite_meta::column_exists(pool, "sync_jobs", "created_at").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_sync_jobs_recent ON sync_jobs(updated_at DESC, created_at DESC)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "market_recent_trades").await?
        && sqlite_meta::column_exists(pool, "market_recent_trades", "inst_id").await?
        && sqlite_meta::column_exists(pool, "market_recent_trades", "inst_type").await?
        && sqlite_meta::column_exists(pool, "market_recent_trades", "ts").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_market_trades_scope_recent ON market_recent_trades(inst_id, inst_type, ts DESC)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "live_order_records").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "mode").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "created_at").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_live_orders_mode_recent ON live_order_records(mode, created_at DESC, id DESC)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "live_order_records").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "mode").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "run_id").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "created_at").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_live_orders_mode_run_recent ON live_order_records(mode, run_id, created_at DESC, id DESC)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "scanner_results").await?
        && sqlite_meta::column_exists(pool, "scanner_results", "scan_time").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_scanner_results_recent ON scanner_results(scan_time DESC)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "candles").await?
        && sqlite_meta::column_exists(pool, "candles", "inst_id").await?
        && sqlite_meta::column_exists(pool, "candles", "inst_type").await?
        && sqlite_meta::column_exists(pool, "candles", "timestamp").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_candles_scope_time ON candles(inst_id, inst_type, timestamp DESC)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "backtest_results").await?
        && sqlite_meta::column_exists(pool, "backtest_results", "strategy_id").await?
        && sqlite_meta::column_exists(pool, "backtest_results", "created_at").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_backtest_strategy_recent ON backtest_results(strategy_id, created_at DESC)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "backtest_results").await?
        && sqlite_meta::column_exists(pool, "backtest_results", "strategy_id").await?
        && sqlite_meta::column_exists(pool, "backtest_results", "symbol").await?
        && sqlite_meta::column_exists(pool, "backtest_results", "created_at").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_backtest_strategy_symbol_recent ON backtest_results(strategy_id, symbol, created_at DESC)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "journal_entries").await?
        && sqlite_meta::column_exists(pool, "journal_entries", "created_at").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_journal_recent ON journal_entries(created_at DESC)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "journal_entries").await?
        && sqlite_meta::column_exists(pool, "journal_entries", "strategy_id").await?
        && sqlite_meta::column_exists(pool, "journal_entries", "created_at").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_journal_strategy_time ON journal_entries(strategy_id, created_at DESC)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "research_collection_sessions").await?
        && sqlite_meta::column_exists(pool, "research_collection_sessions", "updated_at").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_research_collection_sessions_recent ON research_collection_sessions(updated_at DESC)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "research_dataset_manifests").await?
        && sqlite_meta::column_exists(pool, "research_dataset_manifests", "updated_at").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_research_dataset_manifests_recent ON research_dataset_manifests(updated_at DESC)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "research_training_runs").await?
        && sqlite_meta::column_exists(pool, "research_training_runs", "updated_at").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_research_training_runs_recent ON research_training_runs(updated_at DESC)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "research_training_runs").await?
        && sqlite_meta::column_exists(pool, "research_training_runs", "dataset_id").await?
        && sqlite_meta::column_exists(pool, "research_training_runs", "updated_at").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_research_training_runs_dataset_recent ON research_training_runs(dataset_id, updated_at DESC)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "inference_snapshots").await?
        && sqlite_meta::column_exists(pool, "inference_snapshots", "created_at").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_inference_snapshots_recent ON inference_snapshots(created_at DESC)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "research_dataset_splits").await?
        && sqlite_meta::column_exists(pool, "research_dataset_splits", "dataset_id").await?
        && sqlite_meta::column_exists(pool, "research_dataset_splits", "split").await?
        && sqlite_meta::column_exists(pool, "research_dataset_splits", "row_index").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_research_dataset_splits_dataset_split_row ON research_dataset_splits(dataset_id, split, row_index)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "live_order_records").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "mode").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "order_id").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "created_at").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "id").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_live_orders_mode_order_recent ON live_order_records(mode, order_id, created_at DESC, id DESC)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "live_order_records").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "mode").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "client_order_id").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "created_at").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "id").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_live_orders_mode_client_recent ON live_order_records(mode, client_order_id, created_at DESC, id DESC)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "live_order_records").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "mode").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "parent_order_id").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "created_at").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "id").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_live_orders_mode_parent_order_recent ON live_order_records(mode, parent_order_id, created_at DESC, id DESC)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "live_order_records").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "mode").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "parent_client_order_id").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "created_at").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "id").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_live_orders_mode_parent_client_recent ON live_order_records(mode, parent_client_order_id, created_at DESC, id DESC)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "assistant_order_drafts").await?
        && sqlite_meta::column_exists(pool, "assistant_order_drafts", "created_at").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_assistant_order_drafts_recent ON assistant_order_drafts(created_at DESC)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "assistant_order_drafts").await?
        && sqlite_meta::column_exists(pool, "assistant_order_drafts", "session_id").await?
        && sqlite_meta::column_exists(pool, "assistant_order_drafts", "created_at").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_assistant_order_drafts_session_recent ON assistant_order_drafts(session_id, created_at DESC)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "assistant_level_snapshots").await?
        && sqlite_meta::column_exists(pool, "assistant_level_snapshots", "created_at").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_assistant_level_snapshots_recent ON assistant_level_snapshots(created_at DESC)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "assistant_level_snapshots").await?
        && sqlite_meta::column_exists(pool, "assistant_level_snapshots", "session_id").await?
        && sqlite_meta::column_exists(pool, "assistant_level_snapshots", "created_at").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_assistant_level_snapshots_session_recent ON assistant_level_snapshots(session_id, created_at DESC)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "assistant_sessions").await?
        && sqlite_meta::column_exists(pool, "assistant_sessions", "kind").await?
        && sqlite_meta::column_exists(pool, "assistant_sessions", "updated_at").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_assistant_sessions_kind_updated ON assistant_sessions(kind, updated_at)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "assistant_messages").await?
        && sqlite_meta::column_exists(pool, "assistant_messages", "session_id").await?
        && sqlite_meta::column_exists(pool, "assistant_messages", "created_at").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_assistant_messages_session ON assistant_messages(session_id, created_at)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "assistant_steps").await?
        && sqlite_meta::column_exists(pool, "assistant_steps", "session_id").await?
        && sqlite_meta::column_exists(pool, "assistant_steps", "created_at").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_assistant_steps_session ON assistant_steps(session_id, created_at)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "live_order_records").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "mode").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "actual_order_id").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "created_at").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "id").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_live_orders_mode_actual_order_recent ON live_order_records(mode, actual_order_id, created_at DESC, id DESC)",
        )
        .execute(pool)
        .await?;
    }
    if sqlite_meta::table_exists(pool, "live_order_records").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "mode").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "actual_client_order_id").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "created_at").await?
        && sqlite_meta::column_exists(pool, "live_order_records", "id").await?
    {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_live_orders_mode_actual_client_recent ON live_order_records(mode, actual_client_order_id, created_at DESC, id DESC)",
        )
        .execute(pool)
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

    use super::{run, sqlite_meta};

    #[tokio::test]
    async fn run_migrates_legacy_live_order_table_before_actual_order_indexes() -> anyhow::Result<()>
    {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        sqlx::query(
            r#"
            CREATE TABLE live_order_records (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              strategy_id TEXT NOT NULL,
              mode TEXT NOT NULL DEFAULT 'simulated',
              order_id TEXT,
              client_order_id TEXT DEFAULT '',
              created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&pool)
        .await?;

        run(&pool).await?;

        assert!(sqlite_meta::column_exists(&pool, "live_order_records", "actual_order_id").await?);
        assert!(
            sqlite_meta::column_exists(&pool, "live_order_records", "actual_client_order_id")
                .await?
        );
        assert!(sqlite_meta::column_exists(&pool, "live_order_records", "parent_order_id").await?);
        assert!(
            sqlite_meta::column_exists(&pool, "live_order_records", "parent_client_order_id")
                .await?
        );
        assert!(
            index_exists(&pool, "idx_live_orders_mode_actual_order_recent").await?,
            "actual order index should be created after the additive column migration"
        );
        assert!(
            index_exists(&pool, "idx_live_orders_mode_actual_client_recent").await?,
            "actual client order index should be created after the additive column migration"
        );
        assert!(
            index_exists(&pool, "idx_live_orders_mode_parent_order_recent").await?,
            "parent order index should be created after the additive column migration"
        );
        assert!(
            index_exists(&pool, "idx_live_orders_mode_parent_client_recent").await?,
            "parent client order index should be created after the additive column migration"
        );
        Ok(())
    }

    #[tokio::test]
    async fn run_migrates_legacy_live_execution_logs_before_mode_indexes() -> anyhow::Result<()> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        sqlx::query(
            r#"
            CREATE TABLE live_execution_logs (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              run_id TEXT NOT NULL,
              seq INTEGER NOT NULL,
              timestamp_ms INTEGER NOT NULL,
              time TEXT NOT NULL DEFAULT '',
              stage TEXT NOT NULL DEFAULT '',
              level TEXT NOT NULL DEFAULT 'info',
              message TEXT NOT NULL DEFAULT '',
              details_json TEXT NOT NULL DEFAULT '{}',
              created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&pool)
        .await?;

        run(&pool).await?;

        assert!(sqlite_meta::column_exists(&pool, "live_execution_logs", "mode").await?);
        assert!(sqlite_meta::column_exists(&pool, "live_execution_logs", "strategy_id").await?);
        assert!(
            index_exists(&pool, "idx_live_execution_logs_mode_run_recent").await?,
            "mode+run log index should be created after the additive column migration"
        );
        assert!(
            index_exists(&pool, "idx_live_execution_logs_mode_recent").await?,
            "mode log index should be created after the additive column migration"
        );
        Ok(())
    }

    async fn index_exists(pool: &SqlitePool, index: &str) -> anyhow::Result<bool> {
        let row = sqlx::query("SELECT name FROM sqlite_master WHERE type = 'index' AND name = ?")
            .bind(index)
            .fetch_optional(pool)
            .await?;
        Ok(row.is_some())
    }
}
