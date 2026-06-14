pub(super) const SCHEMA: &[&str] = &[
    r#"
    CREATE TABLE IF NOT EXISTS scanner_profiles (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      profile_id TEXT NOT NULL UNIQUE,
      name TEXT NOT NULL,
      conditions_json TEXT NOT NULL DEFAULT '[]',
      logic TEXT DEFAULT 'and',
      symbols_json TEXT DEFAULT '[]',
      timeframe TEXT DEFAULT '1H',
      inst_type TEXT DEFAULT 'SPOT',
      enabled INTEGER DEFAULT 1,
      interval_seconds INTEGER DEFAULT 300,
      created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
      updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS scanner_results (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      profile_id TEXT NOT NULL,
      inst_id TEXT NOT NULL,
      inst_type TEXT DEFAULT 'SPOT',
      timeframe TEXT NOT NULL,
      matched_conditions_json TEXT DEFAULT '[]',
      indicator_values_json TEXT DEFAULT '{}',
      price REAL DEFAULT 0,
      scan_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    )
    "#,
    "CREATE INDEX IF NOT EXISTS idx_scanner_results_time ON scanner_results(profile_id, scan_time)",
    "CREATE INDEX IF NOT EXISTS idx_scanner_results_recent ON scanner_results(scan_time DESC)",
];
