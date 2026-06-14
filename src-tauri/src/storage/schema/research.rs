pub(super) const SCHEMA: &[&str] = &[
    r#"
    CREATE TABLE IF NOT EXISTS research_collection_sessions (
      session_id TEXT PRIMARY KEY,
      status TEXT NOT NULL,
      payload_json TEXT NOT NULL DEFAULT '{}',
      created_at REAL NOT NULL,
      updated_at REAL NOT NULL,
      started_at REAL,
      ended_at REAL,
      failed_at REAL,
      stop_reason TEXT NOT NULL DEFAULT '',
      last_error_code TEXT NOT NULL DEFAULT '',
      last_error_message TEXT NOT NULL DEFAULT ''
    )
    "#,
    "CREATE INDEX IF NOT EXISTS idx_research_collection_sessions_recent ON research_collection_sessions(updated_at DESC)",
    r#"
    CREATE TABLE IF NOT EXISTS research_second_states (
      session_id TEXT NOT NULL,
      inst_id TEXT NOT NULL,
      second_bucket INTEGER NOT NULL,
      payload_json TEXT NOT NULL DEFAULT '{}',
      created_at REAL NOT NULL,
      PRIMARY KEY(session_id, inst_id, second_bucket)
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS research_census_second_states (
      session_id TEXT NOT NULL,
      inst_id TEXT NOT NULL,
      second_bucket INTEGER NOT NULL,
      payload_json TEXT NOT NULL DEFAULT '{}',
      created_at REAL NOT NULL,
      PRIMARY KEY(session_id, inst_id, second_bucket)
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS research_target_census_15m (
      inst_id TEXT NOT NULL,
      target_bucket INTEGER NOT NULL,
      payload_json TEXT NOT NULL DEFAULT '{}',
      created_at REAL NOT NULL,
      PRIMARY KEY(inst_id, target_bucket)
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS research_artifacts (
      artifact_ref TEXT PRIMARY KEY,
      payload_json TEXT NOT NULL,
      created_at REAL NOT NULL
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS research_boundary_targets_15m (
      inst_id TEXT NOT NULL,
      target_bucket INTEGER NOT NULL,
      label_definition_version TEXT NOT NULL DEFAULT '',
      payload_json TEXT NOT NULL DEFAULT '{}',
      created_at REAL NOT NULL,
      PRIMARY KEY(inst_id, target_bucket, label_definition_version)
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS research_sample_index_15m (
      sample_id TEXT PRIMARY KEY,
      dataset_id TEXT NOT NULL DEFAULT '',
      inst_id TEXT NOT NULL,
      target_bucket INTEGER NOT NULL,
      payload_json TEXT NOT NULL DEFAULT '{}',
      created_at REAL NOT NULL
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS research_dataset_manifests (
      dataset_id TEXT PRIMARY KEY,
      status TEXT NOT NULL DEFAULT 'created',
      manifest_json TEXT NOT NULL DEFAULT '{}',
      created_at REAL NOT NULL,
      updated_at REAL NOT NULL
    )
    "#,
    "CREATE INDEX IF NOT EXISTS idx_research_dataset_manifests_recent ON research_dataset_manifests(updated_at DESC)",
    r#"
    CREATE TABLE IF NOT EXISTS research_training_runs (
      run_id TEXT PRIMARY KEY,
      dataset_id TEXT NOT NULL,
      status TEXT NOT NULL,
      progress_stage TEXT NOT NULL DEFAULT '',
      payload_json TEXT NOT NULL DEFAULT '{}',
      created_at REAL NOT NULL,
      updated_at REAL NOT NULL
    )
    "#,
    "CREATE INDEX IF NOT EXISTS idx_research_training_runs_recent ON research_training_runs(updated_at DESC)",
    "CREATE INDEX IF NOT EXISTS idx_research_training_runs_dataset_recent ON research_training_runs(dataset_id, updated_at DESC)",
    r#"
    CREATE TABLE IF NOT EXISTS feature_bars_1s (
      inst_id TEXT NOT NULL,
      ts INTEGER NOT NULL,
      payload_json TEXT NOT NULL DEFAULT '{}',
      created_at REAL NOT NULL,
      PRIMARY KEY(inst_id, ts)
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS swing_labels (
      inst_id TEXT NOT NULL,
      ts INTEGER NOT NULL,
      payload_json TEXT NOT NULL DEFAULT '{}',
      created_at REAL NOT NULL,
      PRIMARY KEY(inst_id, ts)
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS factor_scores (
      inst_id TEXT NOT NULL,
      factor_name TEXT NOT NULL,
      payload_json TEXT NOT NULL DEFAULT '{}',
      created_at REAL NOT NULL,
      PRIMARY KEY(inst_id, factor_name)
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS research_dataset_splits (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      dataset_id TEXT NOT NULL,
      split TEXT NOT NULL,
      row_index INTEGER NOT NULL,
      inst_id TEXT NOT NULL,
      ts INTEGER NOT NULL,
      features_json TEXT NOT NULL DEFAULT '{}',
      label_json TEXT NOT NULL DEFAULT '{}',
      created_at REAL NOT NULL
    )
    "#,
    "CREATE INDEX IF NOT EXISTS idx_research_dataset_splits_dataset_split_row ON research_dataset_splits(dataset_id, split, row_index)",
    r#"
    CREATE TABLE IF NOT EXISTS inference_snapshots (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      inst_id TEXT NOT NULL,
      payload_json TEXT NOT NULL DEFAULT '{}',
      created_at REAL NOT NULL
    )
    "#,
    "CREATE INDEX IF NOT EXISTS idx_inference_snapshots_recent ON inference_snapshots(created_at DESC)",
];
