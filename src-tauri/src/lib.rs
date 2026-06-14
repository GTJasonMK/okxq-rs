mod alerts;
mod app_state;
mod backtest_progress;
mod backtest_result_persistence;
mod commands;
mod config;
mod correlation;
mod dataset_builder;
mod error;
mod factor_engine;
mod feature_bar_rows;
mod guardian;
mod indicators;
mod instrument;
mod live_strategy;
pub mod live_trade_smoke;
mod market_candle_rows;
mod model_trainer;
mod monte_carlo;
mod ohlcv;
mod okx;
mod okx_network;
mod okx_outbound;
mod orderbook_snapshot;
mod realtime;
mod risk_controls;
mod storage;
mod strategy_engine;
mod strategy_execution_contract;
mod strategy_execution_semantics;
mod strategy_executor;
mod sync_jobs;
mod sync_record_summary;
mod tick_collector;
mod timeframes;
mod token_bucket;
mod trading_fills;
mod trading_semantics;
mod walk_forward;

use app_state::AppState;
use commands::{
    config::{
        get_assistant_config, get_okx_config, save_assistant_config, save_okx_config,
        test_okx_connection,
    },
    local_api::local_api_request,
    preferences::{
        add_watched_symbol, delete_preference, get_preference, get_preferences,
        get_watched_symbols, save_preferences, update_preferences,
    },
    system::{get_app_info, system_health, system_status},
};
use tauri::Manager;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn run() {
    init_logging();
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "OKXQ Tauri application starting"
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::block_on(async move {
                tracing::info!("bootstrapping application state");
                let state = AppState::bootstrap(&handle).await?;
                tracing::info!(
                    root = %state.paths.root.display(),
                    config_dir = %state.paths.config_dir.display(),
                    data_dir = %state.paths.data_dir.display(),
                    logs_dir = %state.paths.logs_dir.display(),
                    "application state bootstrapped"
                );
                handle.manage(state);
                Ok::<(), anyhow::Error>(())
            })?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            system_health,
            system_status,
            local_api_request,
            get_okx_config,
            save_okx_config,
            test_okx_connection,
            get_assistant_config,
            save_assistant_config,
            get_preferences,
            get_preference,
            save_preferences,
            update_preferences,
            delete_preference,
            get_watched_symbols,
            add_watched_symbol,
        ])
        .run(tauri::generate_context!())
        .expect("error while running OKX Quantitative Tauri application");
}

fn init_logging() {
    let default_filter = if cfg!(debug_assertions) {
        "okxq_rs=debug,info"
    } else {
        "okxq_rs=info,warn"
    };
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));
    if tracing_subscriber::registry()
        .with(env_filter)
        .with(
            fmt::layer()
                .with_target(true)
                .with_thread_ids(true)
                .with_level(true),
        )
        .try_init()
        .is_err()
    {
        eprintln!("[okxq] tracing subscriber already initialized");
    }
}
