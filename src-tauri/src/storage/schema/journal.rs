pub(super) const SCHEMA: &[&str] = &[
    r#"
    CREATE TABLE IF NOT EXISTS journal_entries (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      entry_id TEXT NOT NULL UNIQUE,
      title TEXT NOT NULL DEFAULT '',
      content TEXT NOT NULL DEFAULT '',
      mode TEXT NOT NULL DEFAULT 'simulated',
      inst_id TEXT DEFAULT '',
      inst_type TEXT NOT NULL DEFAULT 'SPOT',
      trade_ids_json TEXT DEFAULT '[]',
      order_ids_json TEXT DEFAULT '[]',
      tags_json TEXT DEFAULT '[]',
      strategy_id TEXT NOT NULL DEFAULT '',
      strategy_name TEXT NOT NULL DEFAULT '',
      rating INTEGER DEFAULT 0,
      emotion TEXT DEFAULT '',
      screenshots_json TEXT NOT NULL DEFAULT '[]',
      pnl_snapshot REAL DEFAULT 0,
      metadata_json TEXT NOT NULL DEFAULT '{}',
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    )
    "#,
    "CREATE INDEX IF NOT EXISTS idx_journal_mode_time ON journal_entries(mode, created_at)",
    "CREATE INDEX IF NOT EXISTS idx_journal_inst ON journal_entries(inst_id, created_at)",
    "CREATE INDEX IF NOT EXISTS idx_journal_recent ON journal_entries(created_at DESC)",
    "CREATE INDEX IF NOT EXISTS idx_journal_strategy_time ON journal_entries(strategy_id, created_at DESC)",
    r#"
    CREATE TABLE IF NOT EXISTS journal_tags (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      tag TEXT NOT NULL UNIQUE,
      color TEXT DEFAULT '',
      usage_count INTEGER DEFAULT 0,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    )
    "#,
];
