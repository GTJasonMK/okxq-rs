pub(super) const SCHEMA: &[&str] = &[
    r#"
    CREATE TABLE IF NOT EXISTS assistant_sessions (
      id TEXT PRIMARY KEY,
      kind TEXT NOT NULL DEFAULT 'chat',
      title TEXT NOT NULL DEFAULT '',
      metadata_json TEXT NOT NULL DEFAULT '{}',
      created_at TEXT NOT NULL,
      updated_at TEXT NOT NULL
    )
    "#,
    "CREATE INDEX IF NOT EXISTS idx_assistant_sessions_kind_updated ON assistant_sessions(kind, updated_at)",
    r#"
    CREATE TABLE IF NOT EXISTS assistant_messages (
      id TEXT PRIMARY KEY,
      session_id TEXT NOT NULL,
      role TEXT NOT NULL,
      content TEXT NOT NULL,
      metadata_json TEXT NOT NULL DEFAULT '{}',
      created_at TEXT NOT NULL
    )
    "#,
    "CREATE INDEX IF NOT EXISTS idx_assistant_messages_session ON assistant_messages(session_id, created_at)",
    r#"
    CREATE TABLE IF NOT EXISTS assistant_steps (
      id TEXT PRIMARY KEY,
      session_id TEXT NOT NULL,
      step_type TEXT NOT NULL,
      title TEXT NOT NULL DEFAULT '',
      input_json TEXT NOT NULL DEFAULT '{}',
      output_json TEXT NOT NULL DEFAULT '{}',
      created_at TEXT NOT NULL
    )
    "#,
    "CREATE INDEX IF NOT EXISTS idx_assistant_steps_session ON assistant_steps(session_id, created_at)",
    r#"
    CREATE TABLE IF NOT EXISTS assistant_order_drafts (
      id TEXT PRIMARY KEY,
      session_id TEXT NOT NULL DEFAULT '',
      inst_id TEXT NOT NULL,
      mode TEXT NOT NULL,
      side TEXT NOT NULL,
      order_type TEXT NOT NULL,
      size TEXT NOT NULL,
      price TEXT NOT NULL DEFAULT '',
      status TEXT NOT NULL DEFAULT 'draft',
      risk_json TEXT NOT NULL DEFAULT '{}',
      plan_json TEXT NOT NULL DEFAULT '{}',
      annotations_json TEXT NOT NULL DEFAULT '[]',
      metadata_json TEXT NOT NULL DEFAULT '{}',
      created_at TEXT NOT NULL,
      updated_at TEXT NOT NULL
    )
    "#,
    "CREATE INDEX IF NOT EXISTS idx_assistant_order_drafts_session ON assistant_order_drafts(session_id)",
    "CREATE INDEX IF NOT EXISTS idx_assistant_order_drafts_recent ON assistant_order_drafts(created_at DESC)",
    "CREATE INDEX IF NOT EXISTS idx_assistant_order_drafts_session_recent ON assistant_order_drafts(session_id, created_at DESC)",
    "CREATE INDEX IF NOT EXISTS idx_assistant_order_drafts_inst ON assistant_order_drafts(inst_id, created_at)",
    r#"
    CREATE TABLE IF NOT EXISTS assistant_level_snapshots (
      id TEXT PRIMARY KEY,
      session_id TEXT NOT NULL DEFAULT '',
      inst_id TEXT NOT NULL,
      mode TEXT NOT NULL DEFAULT 'simulated',
      timeframes_json TEXT NOT NULL DEFAULT '[]',
      supports_json TEXT NOT NULL DEFAULT '[]',
      resistances_json TEXT NOT NULL DEFAULT '[]',
      invalidation_levels_json TEXT NOT NULL DEFAULT '[]',
      chart_annotations_json TEXT NOT NULL DEFAULT '[]',
      summary_json TEXT NOT NULL DEFAULT '{}',
      metadata_json TEXT NOT NULL DEFAULT '{}',
      created_at TEXT NOT NULL
    )
    "#,
    "CREATE INDEX IF NOT EXISTS idx_assistant_level_snapshots_inst ON assistant_level_snapshots(inst_id, created_at)",
    "CREATE INDEX IF NOT EXISTS idx_assistant_level_snapshots_session ON assistant_level_snapshots(session_id)",
    "CREATE INDEX IF NOT EXISTS idx_assistant_level_snapshots_recent ON assistant_level_snapshots(created_at DESC)",
    "CREATE INDEX IF NOT EXISTS idx_assistant_level_snapshots_session_recent ON assistant_level_snapshots(session_id, created_at DESC)",
    r#"
    CREATE TABLE IF NOT EXISTS assistant_patrol_runs (
      id TEXT PRIMARY KEY,
      mode TEXT NOT NULL DEFAULT 'simulated',
      status TEXT NOT NULL,
      summary_json TEXT NOT NULL DEFAULT '{}',
      candidates_json TEXT NOT NULL DEFAULT '[]',
      result_json TEXT NOT NULL DEFAULT '{}',
      event_json TEXT NOT NULL DEFAULT '{}',
      settings_json TEXT NOT NULL DEFAULT '{}',
      started_at TEXT NOT NULL,
      finished_at TEXT
    )
    "#,
    "CREATE INDEX IF NOT EXISTS idx_assistant_patrol_runs_time ON assistant_patrol_runs(started_at)",
    "CREATE INDEX IF NOT EXISTS idx_assistant_patrol_runs_mode ON assistant_patrol_runs(mode, started_at)",
];
