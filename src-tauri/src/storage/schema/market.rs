pub(super) const SCHEMA: &[&str] = &[
    r#"
    CREATE TABLE IF NOT EXISTS candles (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      inst_id TEXT NOT NULL,
      inst_type TEXT NOT NULL DEFAULT 'SPOT',
      timeframe TEXT NOT NULL,
      timestamp INTEGER NOT NULL,
      open REAL NOT NULL,
      high REAL NOT NULL,
      low REAL NOT NULL,
      close REAL NOT NULL,
      volume REAL NOT NULL,
      volume_ccy REAL DEFAULT 0,
      volume_quote REAL,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      UNIQUE(inst_id, inst_type, timeframe, timestamp)
    )
    "#,
    "CREATE INDEX IF NOT EXISTS idx_candles_query ON candles(inst_id, inst_type, timeframe, timestamp)",
    "CREATE INDEX IF NOT EXISTS idx_candles_scope_time ON candles(inst_id, inst_type, timestamp DESC)",
    "CREATE INDEX IF NOT EXISTS idx_candles_time ON candles(timestamp)",
    r#"
    CREATE TABLE IF NOT EXISTS sync_records (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      inst_id TEXT NOT NULL,
      inst_type TEXT NOT NULL DEFAULT 'SPOT',
      timeframe TEXT NOT NULL,
      last_sync_time TIMESTAMP,
      oldest_timestamp INTEGER,
      newest_timestamp INTEGER,
      candle_count INTEGER DEFAULT 0,
      history_complete INTEGER NOT NULL DEFAULT 0,
      last_sync_mode TEXT NOT NULL DEFAULT 'window',
      UNIQUE(inst_id, inst_type, timeframe)
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS sync_jobs (
      task_id TEXT PRIMARY KEY,
      inst_id TEXT NOT NULL,
      inst_type TEXT NOT NULL DEFAULT 'SPOT',
      timeframe TEXT NOT NULL,
      source_timeframe TEXT NOT NULL DEFAULT '1m',
      target_timeframes TEXT NOT NULL DEFAULT '[]',
      mode TEXT NOT NULL DEFAULT 'window',
      days INTEGER NOT NULL DEFAULT 30,
      start_ts INTEGER,
      end_ts INTEGER,
      repair_method TEXT NOT NULL DEFAULT '',
      status TEXT NOT NULL DEFAULT 'queued',
      progress INTEGER NOT NULL DEFAULT 0,
      message TEXT NOT NULL DEFAULT '',
      created_at TEXT NOT NULL,
      started_at TEXT,
      updated_at TEXT NOT NULL,
      finished_at TEXT,
      error TEXT NOT NULL DEFAULT '',
      fetched_count INTEGER NOT NULL DEFAULT 0,
      target_fetch_count INTEGER NOT NULL DEFAULT 0,
      saved_count INTEGER NOT NULL DEFAULT 0,
      target_save_count INTEGER NOT NULL DEFAULT 0,
      inserted_count INTEGER NOT NULL DEFAULT 0,
      derived_count INTEGER NOT NULL DEFAULT 0,
      target_derive_count INTEGER NOT NULL DEFAULT 0,
      batches INTEGER NOT NULL DEFAULT 0,
      target_batches INTEGER NOT NULL DEFAULT 0,
      api_calls INTEGER NOT NULL DEFAULT 0,
      candle_count INTEGER NOT NULL DEFAULT 0,
      history_complete INTEGER NOT NULL DEFAULT 0,
      last_sync_mode TEXT NOT NULL DEFAULT 'window',
      last_sync_time TEXT,
      oldest_timestamp INTEGER,
      newest_timestamp INTEGER,
      oldest_time TEXT,
      newest_time TEXT,
      reused_existing INTEGER NOT NULL DEFAULT 0,
      truncated INTEGER NOT NULL DEFAULT 0,
      cancel_requested INTEGER NOT NULL DEFAULT 0
    )
    "#,
    "CREATE INDEX IF NOT EXISTS idx_sync_jobs_status_updated ON sync_jobs(status, updated_at)",
    "CREATE INDEX IF NOT EXISTS idx_sync_jobs_recent ON sync_jobs(updated_at DESC, created_at DESC)",
    "CREATE INDEX IF NOT EXISTS idx_sync_jobs_scope ON sync_jobs(inst_id, inst_type, timeframe, mode, days)",
    "CREATE INDEX IF NOT EXISTS idx_sync_jobs_scope_range ON sync_jobs(inst_id, inst_type, source_timeframe, target_timeframes, mode, days, start_ts, end_ts, repair_method, status)",
    r#"
    CREATE TABLE IF NOT EXISTS price_alerts (
      id TEXT PRIMARY KEY,
      inst_id TEXT NOT NULL,
      symbol TEXT NOT NULL DEFAULT '',
      inst_type TEXT NOT NULL DEFAULT 'SPOT',
      alert_type TEXT NOT NULL DEFAULT 'price',
      direction TEXT NOT NULL DEFAULT 'above',
      target_price REAL,
      change_percent REAL,
      note TEXT NOT NULL DEFAULT '',
      enabled INTEGER NOT NULL DEFAULT 1,
      trigger_once INTEGER NOT NULL DEFAULT 1,
      cooldown_seconds INTEGER NOT NULL DEFAULT 300,
      created_at TEXT NOT NULL,
      updated_at TEXT NOT NULL,
      triggered_at TEXT,
      last_value REAL,
      last_trigger_value REAL,
      last_trigger_ts INTEGER NOT NULL DEFAULT 0
    )
    "#,
    "CREATE INDEX IF NOT EXISTS idx_price_alerts_scope ON price_alerts(inst_id, inst_type, enabled)",
    "CREATE INDEX IF NOT EXISTS idx_price_alerts_updated ON price_alerts(updated_at)",
    r#"
    CREATE TABLE IF NOT EXISTS market_ticker_snapshots (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      inst_id TEXT NOT NULL,
      inst_type TEXT NOT NULL DEFAULT 'SPOT',
      payload_json TEXT NOT NULL DEFAULT '{}',
      ts INTEGER NOT NULL,
      created_at TEXT NOT NULL
    )
    "#,
    "CREATE INDEX IF NOT EXISTS idx_market_ticker_inst_time ON market_ticker_snapshots(inst_id, ts)",
    "CREATE INDEX IF NOT EXISTS idx_market_ticker_type_time ON market_ticker_snapshots(inst_type, ts)",
    r#"
    CREATE TABLE IF NOT EXISTS okx_funding_rates (
      inst_id TEXT NOT NULL,
      inst_type TEXT NOT NULL DEFAULT 'SWAP',
      funding_time INTEGER NOT NULL,
      funding_rate REAL NOT NULL,
      realized_rate REAL,
      method TEXT NOT NULL DEFAULT '',
      formula_type TEXT NOT NULL DEFAULT '',
      payload_json TEXT NOT NULL DEFAULT '{}',
      fetched_at TEXT NOT NULL DEFAULT '',
      PRIMARY KEY(inst_id, inst_type, funding_time)
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS market_recent_trades (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      inst_id TEXT NOT NULL,
      inst_type TEXT NOT NULL DEFAULT 'SPOT',
      trade_id TEXT NOT NULL DEFAULT '',
      payload_json TEXT NOT NULL DEFAULT '{}',
      ts INTEGER NOT NULL,
      created_at TEXT NOT NULL,
      UNIQUE(inst_id, inst_type, trade_id)
    )
    "#,
    "CREATE INDEX IF NOT EXISTS idx_market_trades_inst_time ON market_recent_trades(inst_id, ts)",
    "CREATE INDEX IF NOT EXISTS idx_market_trades_type_time ON market_recent_trades(inst_type, ts)",
    "CREATE INDEX IF NOT EXISTS idx_market_trades_scope_recent ON market_recent_trades(inst_id, inst_type, ts DESC)",
    r#"
    CREATE TABLE IF NOT EXISTS inventory_deletion_marks (
      symbol TEXT PRIMARY KEY,
      requested_at TEXT NOT NULL,
      reason TEXT NOT NULL DEFAULT '',
      last_error TEXT NOT NULL DEFAULT ''
    )
    "#,
    "CREATE INDEX IF NOT EXISTS idx_inventory_deletion_marks_requested ON inventory_deletion_marks(requested_at)",
];
