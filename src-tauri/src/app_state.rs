use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};
use tokio::sync::RwLock;

use crate::{
    backtest_progress::BacktestProgressRegistry,
    config::{AppConfig, ConfigManager, PreferencesStore},
    guardian::GuardianRuntime,
    live_strategy::LiveStrategyRuntime,
    okx_outbound::{OKXOutboundTimelineStore, OKXRateRuleRegistry},
    realtime::RealtimeManager,
    storage,
    sync_jobs::SyncJobManager,
    tick_collector::TickCollectorManager,
    token_bucket::SharedTokenBucketRegistry,
};

pub struct AppState {
    pub paths: ProjectPaths,
    pub config: RwLock<AppConfig>,
    pub config_manager: ConfigManager,
    pub preferences: PreferencesStore,
    pub db: SqlitePool,
    pub backtest_progress: BacktestProgressRegistry,
    pub sync_jobs: SyncJobManager,
    pub guardian: GuardianRuntime,
    pub realtime: RealtimeManager,
    pub live_strategy: LiveStrategyRuntime,
    pub okx_outbound_timeline: Arc<OKXOutboundTimelineStore>,
    pub okx_rate_rules: Arc<OKXRateRuleRegistry>,
    pub app_handle: AppHandle,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub token_bucket: SharedTokenBucketRegistry,
    pub tick_collector: TickCollectorManager,
}

impl AppState {
    pub async fn bootstrap(app: &AppHandle) -> anyhow::Result<Self> {
        let paths = ProjectPaths::resolve(app)?;
        paths.ensure()?;
        tracing::info!(
            root = %paths.root.display(),
            config_dir = %paths.config_dir.display(),
            data_dir = %paths.data_dir.display(),
            logs_dir = %paths.logs_dir.display(),
            strategies_dir = %paths.strategies_dir.display(),
            "project paths resolved"
        );

        match crate::strategy_executor::discoverable_strategy_ids_fast(&paths.root) {
            Ok(strategies) => {
                tracing::info!(count = strategies.len(), "runtime strategy ids indexed");
            }
            Err(error) => {
                tracing::warn!("运行策略索引失败: {error}");
            }
        }

        let config_manager = ConfigManager::new(paths.config_dir.join(".env"));
        let config = config_manager.load()?;
        tracing::info!(
            okx_mode = config.okx.default_mode(),
            demo_configured = config.okx.demo.is_valid(),
            live_configured = config.okx.live.is_valid(),
            okx_proxy = crate::okx_network::effective_proxy_label(&config.okx.proxy_url).as_str(),
            assistant_enabled = config.assistant.enabled,
            assistant_configured = config.assistant.is_configured(),
            api_debug = config.api_debug,
            "configuration loaded"
        );

        let db_path = config
            .database_path
            .clone()
            .unwrap_or_else(|| paths.data_dir.join("market.db"));
        tracing::info!(db_path = %db_path.display(), "connecting database");
        let db = storage::connect_and_migrate(&db_path).await?;
        let preferences = PreferencesStore::new(paths.config_dir.join("user_preferences.json"));
        let sync_jobs = SyncJobManager::new(db.clone());
        sync_jobs.recover_interrupted_jobs().await?;

        let okx_rate_rules = Arc::new(OKXRateRuleRegistry::default());
        let okx_outbound_timeline = Arc::new(OKXOutboundTimelineStore::default());
        let token_bucket = Arc::new(crate::token_bucket::TokenBucketRegistry::from_rules(
            &okx_rate_rules.rules,
        ));
        let live_strategy = LiveStrategyRuntime::new();
        let realtime = RealtimeManager::new(
            app.clone(),
            db.clone(),
            config.okx.proxy_url.clone(),
            live_strategy.clone(),
            okx_outbound_timeline.clone(),
            okx_rate_rules.clone(),
            token_bucket.clone(),
        );

        Ok(Self {
            paths,
            config: RwLock::new(config),
            config_manager,
            preferences,
            db,
            backtest_progress: BacktestProgressRegistry::default(),
            sync_jobs,
            guardian: GuardianRuntime::new(),
            realtime,
            live_strategy,
            okx_outbound_timeline,
            okx_rate_rules,
            app_handle: app.clone(),
            started_at: chrono::Utc::now(),
            token_bucket,
            tick_collector: TickCollectorManager::new(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct ProjectPaths {
    pub root: PathBuf,
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub strategies_dir: PathBuf,
}

impl ProjectPaths {
    fn resolve(app: &AppHandle) -> anyhow::Result<Self> {
        let root = std::env::var_os("OKXQ_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| resolve_workspace_root(app));

        Ok(Self {
            config_dir: root.join("config"),
            data_dir: root.join("data"),
            logs_dir: root.join("logs"),
            strategies_dir: root.join("strategies"),
            root,
        })
    }

    fn ensure(&self) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.config_dir)?;
        std::fs::create_dir_all(&self.data_dir)?;
        std::fs::create_dir_all(&self.logs_dir)?;
        std::fs::create_dir_all(&self.strategies_dir)?;
        Ok(())
    }
}

fn resolve_workspace_root(app: &AppHandle) -> PathBuf {
    if let Ok(current) = std::env::current_dir() {
        if current.file_name().and_then(|name| name.to_str()) == Some("src-tauri") {
            if let Some(parent) = current.parent() {
                return parent.to_path_buf();
            }
        }
        if looks_like_project_root(&current) {
            return current;
        }
    }

    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| Path::new(".").to_path_buf())
}

fn looks_like_project_root(path: &Path) -> bool {
    path.join("src-tauri").exists() || path.join("package.json").exists()
}
