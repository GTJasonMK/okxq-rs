use crate::backtest_progress::BacktestProgressReporter;
use crate::strategy_executor;
use serde_json::json;

use super::{
    inputs::{backtest_strategy_config, load_backtest_candles},
    reports::backtest_report_value,
    runner::run_runtime_backtest,
    window::{backtest_window, expected_backtest_candle_count, should_persist_backtest_result},
    *,
};

pub(in crate::commands::local_api) async fn run_backtest_strategy(
    state: &AppState,
    strategy_id: &str,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let progress = BacktestProgressReporter::new(
        state.backtest_progress.clone(),
        request_string(req, "progress_id", ""),
        strategy_id,
    );
    progress.start("准备运行策略");

    let result = run_backtest_strategy_inner(state, strategy_id, req, &progress).await;
    match &result {
        Ok(_) => progress.complete("回测完成"),
        Err(error) => progress.fail(&format!("回测失败: {error}")),
    }
    result
}

async fn run_backtest_strategy_inner(
    state: &AppState,
    strategy_id: &str,
    req: &LocalApiRequest,
    progress: &BacktestProgressReporter,
) -> AppResult<Value> {
    progress.report_detail(
        2,
        "prepare",
        "解析回测窗口",
        0,
        0,
        json!({
            "step": "parse_window",
            "strategy_id": strategy_id,
        }),
    );
    let window = backtest_window(req, 365)?;
    let persist_result = should_persist_backtest_result(req);
    progress.report_detail(
        6,
        "prepare",
        "回测窗口已解析",
        0,
        0,
        json!({
            "step": "window_ready",
            "window_days": window.days,
            "window_start": window.start_ts,
            "window_end": window.end_ts,
            "persist_result": persist_result,
        }),
    );
    progress.report_detail(
        8,
        "config",
        "加载策略配置",
        0,
        0,
        json!({
            "step": "load_strategy_config",
            "strategy_id": strategy_id,
        }),
    );
    let config = backtest_strategy_config(state, strategy_id, req).await?;
    let expected_candles = expected_backtest_candle_count(&config.timeframe, window) as usize;
    progress.report_detail(
        12,
        "config",
        "策略配置已加载",
        0,
        expected_candles,
        json!({
            "step": "strategy_config_ready",
            "symbol": config.symbol,
            "inst_type": config.inst_type,
            "timeframe": config.timeframe,
            "initial_capital": config.initial_capital,
            "position_size": config.position_size,
            "expected_candles": expected_candles,
        }),
    );
    progress.report_detail(
        15,
        "candles",
        "加载主标的K线",
        0,
        expected_candles,
        json!({
            "step": "load_primary_candles",
            "symbol": config.symbol,
            "inst_type": config.inst_type,
            "timeframe": config.timeframe,
            "expected_candles": expected_candles,
        }),
    );
    let candles = load_backtest_candles(state, &config, window).await?;
    progress.report_detail(
        22,
        "candles",
        "主标的K线加载完成",
        candles.len(),
        expected_candles.max(candles.len()),
        json!({
            "step": "primary_candles_ready",
            "symbol": config.symbol,
            "timeframe": config.timeframe,
            "loaded_candles": candles.len(),
            "expected_candles": expected_candles,
        }),
    );

    let report = if let Some(runtime_meta) = strategy_executor::get_meta(strategy_id) {
        run_runtime_backtest(state, &runtime_meta, &config, &candles, window, progress).await?
    } else {
        return Err(AppError::Validation(format!("策略不存在: {strategy_id}")));
    };

    progress.report_detail(
        96,
        "persist",
        "保存回测结果",
        candles.len(),
        candles.len(),
        json!({
            "step": "persist_result",
            "persist_result": persist_result,
            "loaded_candles": candles.len(),
            "total_trades": report.total_trades,
            "final_capital": report.final_capital,
        }),
    );
    let data = backtest_report_value(&state.db, &report, persist_result).await?;
    Ok(code_ok(data))
}
