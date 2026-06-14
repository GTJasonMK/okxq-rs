use std::path::PathBuf;

use anyhow::{anyhow, bail, Context};
use serde::Serialize;
use serde_json::{json, Value};

use crate::{
    config::ConfigManager,
    okx::{generate_okx_client_order_id, OkxAttachedAlgoOrder, OkxPrivateClient, OkxPublicClient},
    strategy_executor::normalize_runtime_inst_id,
};

const DEFAULT_ENV_PATH: &str = ".env";
const DEFAULT_MODE: &str = "simulated";
const DEFAULT_SYMBOL: &str = "BTC-USDT-SWAP";
const DEFAULT_INST_TYPE: &str = "SWAP";
const DEFAULT_SIDE: &str = "buy";
const DEFAULT_SLEEP_MS: u64 = 1_500;
const SIMULATED_EXECUTE_CONFIRM: &str = "OKXQ_SMOKE_EXECUTE";
const LIVE_EXECUTE_CONFIRM: &str = "OKXQ_LIVE_SMOKE_EXECUTE";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveTradeSmokeArgs {
    pub env_path: PathBuf,
    pub mode: String,
    pub symbol: String,
    pub inst_type: String,
    pub td_mode: Option<String>,
    pub side: String,
    pub size: Option<String>,
    pub leverage: Option<String>,
    pub risk_stop_px: Option<String>,
    pub risk_amend_stop_px: Option<String>,
    pub manage_limit_px: Option<String>,
    pub manage_amend_px: Option<String>,
    pub execute: bool,
    pub confirm: String,
    pub skip_close: bool,
    pub sleep_ms: u64,
    symbol_explicit: bool,
}

impl Default for LiveTradeSmokeArgs {
    fn default() -> Self {
        Self {
            env_path: PathBuf::from(DEFAULT_ENV_PATH),
            mode: DEFAULT_MODE.to_string(),
            symbol: DEFAULT_SYMBOL.to_string(),
            inst_type: DEFAULT_INST_TYPE.to_string(),
            td_mode: None,
            side: DEFAULT_SIDE.to_string(),
            size: None,
            leverage: None,
            risk_stop_px: None,
            risk_amend_stop_px: None,
            manage_limit_px: None,
            manage_amend_px: None,
            execute: false,
            confirm: String::new(),
            skip_close: false,
            sleep_ms: DEFAULT_SLEEP_MS,
            symbol_explicit: false,
        }
    }
}

impl LiveTradeSmokeArgs {
    pub fn from_env_args<I, S>(args: I) -> anyhow::Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        parse_args(args.into_iter().map(Into::into).skip(1))
    }
}

#[derive(Clone, Debug, Serialize)]
struct SmokeSafety {
    execute: bool,
    confirmed: bool,
    confirmation_required: &'static str,
    auto_close: bool,
    manage_only: bool,
}

pub async fn run_live_trade_smoke(args: LiveTradeSmokeArgs) -> anyhow::Result<Value> {
    let normalized = NormalizedSmokeArgs::from_args(args)?;
    let config = ConfigManager::new(normalized.env_path.clone())
        .load()
        .with_context(|| format!("读取配置失败: {}", normalized.env_path.to_string_lossy()))?;
    let credentials = if normalized.mode == "live" {
        config.okx.live.clone()
    } else {
        config.okx.demo.clone()
    };
    if !credentials.is_valid() {
        bail!(
            "OKX {} API 密钥未配置完整",
            if normalized.mode == "live" {
                "实盘"
            } else {
                "模拟盘"
            }
        );
    }

    let public_client =
        OkxPublicClient::new_with_proxy(config.okx.rest_base_url.clone(), &config.okx.proxy_url)?;
    let private_client = OkxPrivateClient::new_with_proxy(
        config.okx.rest_base_url.clone(),
        credentials,
        normalized.mode == "simulated",
        &config.okx.proxy_url,
    )?;

    let ticker = public_client.get_ticker(&normalized.symbol).await?;
    let instruments = public_client
        .get_instruments(&normalized.inst_type)
        .await?
        .into_iter()
        .filter(|item| {
            item.get("instId")
                .and_then(Value::as_str)
                .is_some_and(|value| value.eq_ignore_ascii_case(&normalized.symbol))
        })
        .collect::<Vec<_>>();
    let account_config = private_client.get_account_config().await?;
    let account_pos_mode = account_position_mode(&account_config);
    let execution_plan = smoke_execution_plan(&normalized, &account_pos_mode, &ticker)?;
    let balance_before = private_client.get_account_balance().await?;
    let positions_before = private_client
        .get_positions(Some(&normalized.inst_type), Some(&normalized.symbol))
        .await?;
    let pending_orders_before = private_client
        .get_pending_orders(Some(&normalized.inst_type), Some(&normalized.symbol))
        .await?;
    let fills_before = private_client
        .get_fills(Some(&normalized.inst_type), Some(&normalized.symbol), 20)
        .await?;

    let mut report = json!({
        "ok": true,
        "kind": "live_trade_smoke",
        "mode": normalized.mode,
        "dry_run": !normalized.execute,
        "env_path": normalized.env_path.to_string_lossy(),
        "request": {
            "symbol": normalized.symbol,
            "inst_type": normalized.inst_type,
            "td_mode": normalized.td_mode,
            "side": normalized.side,
            "size": normalized.size,
            "leverage": normalized.leverage,
            "risk_stop_px": normalized.risk_stop_px,
            "risk_amend_stop_px": normalized.risk_amend_stop_px,
            "manage_limit_px": normalized.manage_limit_px,
            "manage_amend_px": normalized.manage_amend_px,
            "sleep_ms": normalized.sleep_ms,
        },
        "safety": SmokeSafety {
            execute: normalized.execute,
            confirmed: normalized.execute,
            confirmation_required: normalized.confirmation_required(),
            auto_close: normalized.execute && !normalized.skip_close && !normalized.is_manage_order_smoke(),
            manage_only: normalized.is_manage_order_smoke(),
        },
        "execution_plan": execution_plan,
        "preflight": {
            "ticker": ticker,
            "instrument": instruments.first().cloned(),
            "instrument_found": !instruments.is_empty(),
            "account_config": account_config,
            "account_position_mode": account_pos_mode,
            "balance_before": balance_before,
            "positions_before": positions_before,
            "pending_orders_before": pending_orders_before,
            "fills_before": fills_before,
        },
        "execution": Value::Null,
    });

    if !normalized.execute {
        return Ok(report);
    }

    if normalized.is_manage_order_smoke() {
        let manage_order_block =
            run_manage_limit_order_smoke(&private_client, &normalized, &account_pos_mode, &ticker)
                .await?;
        if manage_order_block
            .get("success")
            .and_then(Value::as_bool)
            .is_some_and(|success| !success)
        {
            report["ok"] = json!(false);
        }
        report["execution"] = json!({
            "manage_order": manage_order_block,
        });
        return Ok(report);
    }

    let open_context = exchange_order_context(
        &normalized.inst_type,
        &account_pos_mode,
        &normalized.side,
        false,
    )?;
    let mut leverage_response = Value::Null;
    if normalized.inst_type == "SWAP" {
        if let Some(leverage) = normalized.leverage.as_deref() {
            leverage_response = private_client
                .set_leverage(
                    &normalized.symbol,
                    leverage,
                    &normalized.td_mode,
                    &open_context.pos_side,
                )
                .await?;
        }
    }

    let size = normalized
        .size
        .as_deref()
        .expect("execute args should contain size after validation");
    let open_client_order_id = generate_okx_client_order_id();
    let open_response = private_client
        .place_order(
            &normalized.symbol,
            &normalized.td_mode,
            &normalized.side,
            "market",
            size,
            "",
            &open_context.pos_side,
            open_context.reduce_only,
            &open_client_order_id,
        )
        .await?;
    sleep_after_submit(normalized.sleep_ms).await;
    let positions_after_open = private_client
        .get_positions(Some(&normalized.inst_type), Some(&normalized.symbol))
        .await?;
    let pending_orders_after_open = private_client
        .get_pending_orders(Some(&normalized.inst_type), Some(&normalized.symbol))
        .await?;
    let fills_after_open = private_client
        .get_fills(Some(&normalized.inst_type), Some(&normalized.symbol), 20)
        .await?;

    let risk_order_block = if let Some(stop_px) = normalized.risk_stop_px.as_deref() {
        match async {
            let close_side = reverse_side(&normalized.side)?;
            let risk_context =
                exchange_order_context(&normalized.inst_type, &account_pos_mode, close_side, true)?;
            let risk_client_order_id = generate_okx_client_order_id();
            let algo = OkxAttachedAlgoOrder::stop_loss_market(stop_px);
            let place_response = private_client
                .place_algo_order(
                    &normalized.symbol,
                    &normalized.td_mode,
                    close_side,
                    size,
                    &risk_context.pos_side,
                    risk_context.reduce_only,
                    &risk_client_order_id,
                    &algo,
                )
                .await?;
            sleep_after_submit(normalized.sleep_ms).await;
            let (pending_after_place_rows, pending_after_place) =
                query_pending_algo_orders_value(
                    &private_client,
                    &normalized,
                    response_text(&place_response, "algoId")
                        .as_deref()
                        .unwrap_or(""),
                )
                .await;
            let algo_id =
                resolve_algo_id(&place_response, &pending_after_place_rows, &risk_client_order_id)
                    .ok_or_else(|| anyhow!("OKX 保护单已提交但未返回 algoId，无法安全撤销"))?;
            let algo_client_order_id = response_text(&place_response, "algoClOrdId")
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| risk_client_order_id.clone());

            let mut amend_success = true;
            let mut amend_block = Value::Null;
            if let Some(amend_stop_px) = normalized.risk_amend_stop_px.as_deref() {
                let amend_request_id = generate_okx_client_order_id();
                amend_block = match private_client
                    .amend_algo_order(
                        &normalized.symbol,
                        &algo_id,
                        &algo_client_order_id,
                        "",
                        amend_stop_px,
                        false,
                        true,
                        &amend_request_id,
                    )
                    .await
                {
                    Ok(amend_response) => {
                        sleep_after_submit(normalized.sleep_ms).await;
                        json!({
                            "success": true,
                            "request_id": amend_request_id,
                            "new_stop_px": amend_stop_px,
                            "response": amend_response,
                            "pending_after_amend": query_pending_algo_orders_value(
                                &private_client,
                                &normalized,
                                &algo_id,
                            ).await.1,
                        })
                    }
                    Err(error) => {
                        amend_success = false;
                        json!({
                            "success": false,
                            "request_id": amend_request_id,
                            "new_stop_px": amend_stop_px,
                            "error": error.to_string(),
                            "cleanup": "保护单改单失败，已继续尝试撤销该保护单",
                        })
                    }
                };
            }

            let cancel_block = match private_client
                .cancel_algo_order(&normalized.symbol, &algo_id, &algo_client_order_id)
                .await
            {
                Ok(cancel_response) => {
                    sleep_after_submit(normalized.sleep_ms).await;
                    json!({
                        "success": true,
                        "response": cancel_response,
                        "pending_after_cancel": query_pending_algo_orders_value(
                            &private_client,
                            &normalized,
                            "",
                        ).await.1,
                        "algo_history_after_cancel": query_algo_order_history_value(
                            &private_client,
                            &normalized,
                            "canceled",
                        ).await,
                    })
                }
                Err(error) => json!({
                    "success": false,
                    "error": error.to_string(),
                    "cleanup": "撤销保护单失败，请立即手动检查 OKX 当前条件单，避免 smoke 保护单遗留",
                    "pending_after_cancel_error": query_pending_algo_orders_value(
                        &private_client,
                        &normalized,
                        "",
                    ).await.1,
                }),
            };
            let cancel_success = cancel_block
                .get("success")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            Ok::<Value, anyhow::Error>(json!({
                "kind": "risk_order",
                "success": amend_success && cancel_success,
                "client_order_id": risk_client_order_id,
                "algo_id": algo_id,
                "algo_client_order_id": algo_client_order_id,
                "side": close_side,
                "pos_side": risk_context.pos_side,
                "reduce_only": risk_context.reduce_only,
                "trigger_price": stop_px,
                "place_response": place_response,
                "pending_after_place": pending_after_place,
                "amend": amend_block,
                "cancel": cancel_block,
            }))
        }
        .await
        {
            Ok(value) => value,
            Err(error) => json!({
                "kind": "risk_order",
                "success": false,
                "error": error.to_string(),
                "cleanup": "保护单 smoke 失败，仍会继续尝试执行后续平仓",
            }),
        }
    } else {
        Value::Null
    };

    let mut close_block = Value::Null;
    if !normalized.skip_close {
        close_block = match async {
            let close_side = reverse_side(&normalized.side)?;
            let close_context =
                exchange_order_context(&normalized.inst_type, &account_pos_mode, close_side, true)?;
            let close_client_order_id = generate_okx_client_order_id();
            let close_response = private_client
                .place_order(
                    &normalized.symbol,
                    &normalized.td_mode,
                    close_side,
                    "market",
                    size,
                    "",
                    &close_context.pos_side,
                    close_context.reduce_only,
                    &close_client_order_id,
                )
                .await?;
            sleep_after_submit(normalized.sleep_ms).await;
            Ok::<Value, anyhow::Error>(json!({
                "success": true,
                "client_order_id": close_client_order_id,
                "side": close_side,
                "pos_side": close_context.pos_side,
                "reduce_only": close_context.reduce_only,
                "response": close_response,
                "positions_after_close": private_client
                    .get_positions(Some(&normalized.inst_type), Some(&normalized.symbol))
                    .await?,
                "pending_orders_after_close": private_client
                    .get_pending_orders(Some(&normalized.inst_type), Some(&normalized.symbol))
                    .await?,
                "fills_after_close": private_client
                    .get_fills(Some(&normalized.inst_type), Some(&normalized.symbol), 20)
                    .await?,
                "order_history_after_close": private_client
                    .get_order_history(Some(&normalized.inst_type), Some(&normalized.symbol), 20)
                    .await?,
            }))
        }
        .await
        {
            Ok(value) => value,
            Err(error) => json!({
                "success": false,
                "error": error.to_string(),
                "cleanup": "自动平仓或平仓后核验失败，请立即手动检查 OKX 当前仓位，避免 smoke 仓位遗留",
            }),
        };
    }

    if !open_risk_close_smoke_success(&risk_order_block, &close_block) {
        report["ok"] = json!(false);
    }

    report["execution"] = json!({
        "leverage_response": leverage_response,
        "open": {
            "success": true,
            "client_order_id": open_client_order_id,
            "side": normalized.side,
            "pos_side": open_context.pos_side,
            "reduce_only": open_context.reduce_only,
            "response": open_response,
            "positions_after_open": positions_after_open,
            "pending_orders_after_open": pending_orders_after_open,
            "fills_after_open": fills_after_open,
        },
        "risk_order": risk_order_block,
        "close": close_block,
    });
    Ok(report)
}

#[derive(Clone, Debug)]
struct NormalizedSmokeArgs {
    env_path: PathBuf,
    mode: String,
    symbol: String,
    inst_type: String,
    td_mode: String,
    side: String,
    size: Option<String>,
    leverage: Option<String>,
    risk_stop_px: Option<String>,
    risk_amend_stop_px: Option<String>,
    manage_limit_px: Option<String>,
    manage_amend_px: Option<String>,
    execute: bool,
    skip_close: bool,
    sleep_ms: u64,
}

impl NormalizedSmokeArgs {
    fn from_args(args: LiveTradeSmokeArgs) -> anyhow::Result<Self> {
        let mode = normalize_mode(&args.mode)?;
        let inst_type = normalize_inst_type(&args.inst_type)?;
        let symbol = normalize_runtime_inst_id(&args.symbol, &inst_type)?;
        let td_mode = normalize_td_mode(args.td_mode.as_deref(), &inst_type)?;
        let side = normalize_side(&args.side)?;
        let size = args
            .size
            .as_deref()
            .map(|value| positive_decimal_string(value, "size"))
            .transpose()?;
        let leverage = args
            .leverage
            .as_deref()
            .map(|value| positive_decimal_string(value, "leverage"))
            .transpose()?;
        let risk_stop_px = args
            .risk_stop_px
            .as_deref()
            .map(|value| positive_decimal_string(value, "risk-stop-px"))
            .transpose()?;
        let risk_amend_stop_px = args
            .risk_amend_stop_px
            .as_deref()
            .map(|value| positive_decimal_string(value, "risk-amend-stop-px"))
            .transpose()?;
        let manage_limit_px = args
            .manage_limit_px
            .as_deref()
            .map(|value| positive_decimal_string(value, "manage-limit-px"))
            .transpose()?;
        let manage_amend_px = args
            .manage_amend_px
            .as_deref()
            .map(|value| positive_decimal_string(value, "manage-amend-px"))
            .transpose()?;
        if risk_amend_stop_px.is_some() && risk_stop_px.is_none() {
            bail!("--risk-amend-stop-px 只能和 --risk-stop-px 一起使用");
        }
        if manage_amend_px.is_some() && manage_limit_px.is_none() {
            bail!("--manage-amend-px 只能和 --manage-limit-px 一起使用");
        }
        if manage_limit_px.is_some() && risk_stop_px.is_some() {
            bail!("普通订单管理 smoke 不能和保护单 smoke 混用，请分开执行");
        }
        if args.execute {
            if !args.symbol_explicit {
                bail!("执行 smoke 必须显式传入 --symbol，避免误用默认交易对");
            }
            if size.is_none() {
                bail!("执行 smoke 必须显式传入 --size，dry-run 查询不需要 size");
            }
            let expected = confirmation_required_for_mode(&mode);
            if args.confirm.trim() != expected {
                bail!("执行 smoke 缺少确认词，请传入 --confirm {expected}");
            }
            if mode == "live" && args.skip_close {
                bail!("实盘 smoke 不允许 --skip-close，避免测试后遗留仓位");
            }
            if inst_type == "SPOT" && side == "sell" {
                bail!("现货 smoke 不支持以 sell 开始，避免把现货余额卖出");
            }
            if risk_stop_px.is_some() {
                if inst_type != "SWAP" {
                    bail!("保护单 smoke 只支持 SWAP");
                }
                if args.skip_close {
                    bail!("保护单 smoke 不允许 --skip-close，避免留下无保护持仓");
                }
            }
            if manage_limit_px.is_some() && inst_type != "SWAP" {
                bail!("普通订单管理 smoke 只支持 SWAP，避免现货限价单产生余额占用或误成交");
            }
        }
        Ok(Self {
            env_path: args.env_path,
            mode,
            symbol,
            inst_type,
            td_mode,
            side,
            size,
            leverage,
            risk_stop_px,
            risk_amend_stop_px,
            manage_limit_px,
            manage_amend_px,
            execute: args.execute,
            skip_close: args.skip_close,
            sleep_ms: args.sleep_ms.clamp(250, 30_000),
        })
    }

    fn confirmation_required(&self) -> &'static str {
        confirmation_required_for_mode(&self.mode)
    }

    fn is_manage_order_smoke(&self) -> bool {
        self.manage_limit_px.is_some()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExchangeOrderContext {
    pos_side: String,
    reduce_only: bool,
}

fn parse_args<I>(tokens: I) -> anyhow::Result<LiveTradeSmokeArgs>
where
    I: IntoIterator<Item = String>,
{
    let mut args = LiveTradeSmokeArgs::default();
    let mut iter = tokens.into_iter();
    while let Some(token) = iter.next() {
        match token.as_str() {
            "--env" => args.env_path = PathBuf::from(next_arg(&mut iter, "--env")?),
            "--mode" => args.mode = next_arg(&mut iter, "--mode")?,
            "--symbol" => {
                args.symbol = next_arg(&mut iter, "--symbol")?;
                args.symbol_explicit = true;
            }
            "--inst-type" => args.inst_type = next_arg(&mut iter, "--inst-type")?,
            "--td-mode" => args.td_mode = Some(next_arg(&mut iter, "--td-mode")?),
            "--side" => args.side = next_arg(&mut iter, "--side")?,
            "--size" => args.size = Some(next_arg(&mut iter, "--size")?),
            "--leverage" => args.leverage = Some(next_arg(&mut iter, "--leverage")?),
            "--risk-stop-px" => args.risk_stop_px = Some(next_arg(&mut iter, "--risk-stop-px")?),
            "--risk-amend-stop-px" => {
                args.risk_amend_stop_px = Some(next_arg(&mut iter, "--risk-amend-stop-px")?)
            }
            "--manage-limit-px" => {
                args.manage_limit_px = Some(next_arg(&mut iter, "--manage-limit-px")?)
            }
            "--manage-amend-px" => {
                args.manage_amend_px = Some(next_arg(&mut iter, "--manage-amend-px")?)
            }
            "--confirm" => args.confirm = next_arg(&mut iter, "--confirm")?,
            "--sleep-ms" => {
                let raw = next_arg(&mut iter, "--sleep-ms")?;
                args.sleep_ms = raw
                    .trim()
                    .parse::<u64>()
                    .map_err(|_| anyhow!("--sleep-ms 必须是正整数毫秒"))?;
            }
            "--execute" => args.execute = true,
            "--skip-close" => args.skip_close = true,
            "--help" | "-h" => bail!("{}", usage()),
            other => bail!("未知参数: {other}\n{}", usage()),
        }
    }
    Ok(args)
}

fn next_arg<I>(iter: &mut I, flag: &str) -> anyhow::Result<String>
where
    I: Iterator<Item = String>,
{
    iter.next()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("{flag} 缺少参数值"))
}

fn usage() -> &'static str {
    "用法: cargo run --manifest-path src-tauri/Cargo.toml --bin live_trade_smoke -- \
     [--env .env] [--mode simulated|live] [--symbol BTC-USDT-SWAP] [--inst-type SWAP] \
     [--td-mode cross|isolated|cash] [--side buy|sell] [--size 0.01] [--leverage 3] \
     [--risk-stop-px 65000] [--risk-amend-stop-px 64500] \
     [--manage-limit-px 60000] [--manage-amend-px 59000] \
     [--execute --confirm OKXQ_SMOKE_EXECUTE] [--sleep-ms 1500]\n\
     默认 dry-run 只查询 OKX 环境并输出 execution_plan；实际下单必须显式 --execute、--symbol、--size 和确认词。"
}

fn normalize_mode(value: &str) -> anyhow::Result<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "simulated" => Ok("simulated".to_string()),
        "live" => Ok("live".to_string()),
        other => bail!("--mode={other} 无效，只支持 simulated 或 live"),
    }
}

fn normalize_inst_type(value: &str) -> anyhow::Result<String> {
    match value.trim().to_ascii_uppercase().as_str() {
        "" | "SWAP" => Ok("SWAP".to_string()),
        "SPOT" => Ok("SPOT".to_string()),
        other => bail!("--inst-type={other} 无效，smoke 只支持 SWAP 或 SPOT"),
    }
}

fn normalize_td_mode(value: Option<&str>, inst_type: &str) -> anyhow::Result<String> {
    let value = value
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| {
            if inst_type == "SWAP" {
                "cross".to_string()
            } else {
                "cash".to_string()
            }
        });
    match value.as_str() {
        "cross" | "isolated" | "cash" => Ok(value),
        other => bail!("--td-mode={other} 无效，只支持 cross、isolated 或 cash"),
    }
}

fn normalize_side(value: &str) -> anyhow::Result<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "buy" | "long" => Ok("buy".to_string()),
        "sell" | "short" => Ok("sell".to_string()),
        other => bail!("--side={other} 无效，只支持 buy/long 或 sell/short"),
    }
}

fn positive_decimal_string(value: &str, field: &str) -> anyhow::Result<String> {
    let value = value.trim();
    let parsed = value
        .parse::<f64>()
        .map_err(|_| anyhow!("--{field} 必须是正数"))?;
    if parsed.is_finite() && parsed > 0.0 {
        Ok(value.to_string())
    } else {
        bail!("--{field} 必须是正数")
    }
}

fn confirmation_required_for_mode(mode: &str) -> &'static str {
    if mode == "live" {
        LIVE_EXECUTE_CONFIRM
    } else {
        SIMULATED_EXECUTE_CONFIRM
    }
}

fn account_position_mode(account_config: &Value) -> String {
    account_config
        .get("posMode")
        .or_else(|| account_config.get("pos_mode"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("")
        .to_string()
}

fn exchange_order_context(
    inst_type: &str,
    account_pos_mode: &str,
    side: &str,
    reduce_only: bool,
) -> anyhow::Result<ExchangeOrderContext> {
    if inst_type != "SWAP" {
        return Ok(ExchangeOrderContext {
            pos_side: String::new(),
            reduce_only: false,
        });
    }
    match account_pos_mode {
        "net_mode" => Ok(ExchangeOrderContext {
            pos_side: String::new(),
            reduce_only,
        }),
        "long_short_mode" => {
            let pos_side = match (side, reduce_only) {
                ("buy", false) | ("sell", true) => "long",
                ("sell", false) | ("buy", true) => "short",
                _ => bail!("无法根据 side={side} reduce_only={reduce_only} 推导 OKX posSide"),
            };
            Ok(ExchangeOrderContext {
                pos_side: pos_side.to_string(),
                reduce_only: false,
            })
        }
        other => bail!("无法从 OKX account config 读取支持的 posMode，当前 posMode={other:?}"),
    }
}

fn smoke_execution_plan(
    normalized: &NormalizedSmokeArgs,
    account_pos_mode: &str,
    ticker: &Value,
) -> anyhow::Result<Value> {
    let mut checks = Vec::new();
    let size = normalized.size.as_deref().unwrap_or("");
    if size.is_empty() {
        checks.push(json!({
            "key": "size",
            "status": if normalized.execute { "block" } else { "info" },
            "message": "dry-run 可以不传 size；执行 smoke 必须显式传入 --size",
        }));
    }

    if normalized.is_manage_order_smoke() {
        return manage_order_execution_plan(normalized, account_pos_mode, ticker, checks);
    }

    let open_context = exchange_order_context(
        &normalized.inst_type,
        account_pos_mode,
        &normalized.side,
        false,
    )?;
    let close_side = reverse_side(&normalized.side)?;
    let close_context =
        exchange_order_context(&normalized.inst_type, account_pos_mode, close_side, true)?;
    let leverage = if normalized.inst_type == "SWAP" {
        normalized.leverage.as_deref()
    } else {
        None
    };
    let risk_order = if let Some(stop_px) = normalized.risk_stop_px.as_deref() {
        let risk_context =
            exchange_order_context(&normalized.inst_type, account_pos_mode, close_side, true)?;
        json!({
            "enabled": true,
            "place": {
                "api": "trade/order-algo",
                "inst_id": normalized.symbol,
                "td_mode": normalized.td_mode,
                "side": close_side,
                "ord_type": "conditional",
                "algo_kind": "stop_loss_market",
                "trigger_price": stop_px,
                "sz": nullable_text(size),
                "pos_side": risk_context.pos_side,
                "reduce_only": risk_context.reduce_only,
            },
            "amend": normalized.risk_amend_stop_px.as_deref().map(|value| json!({
                "api": "trade/amend-algos",
                "new_stop_price": value,
            })),
            "cleanup": {
                "api": "trade/cancel-algos",
                "required": true,
                "reason": "保护单 smoke 会在验证后撤销测试保护单",
            },
        })
    } else {
        json!({ "enabled": false })
    };

    Ok(json!({
        "kind": "open_risk_close",
        "mode": normalized.mode,
        "dry_run": !normalized.execute,
        "checks": checks,
        "leverage": leverage.map(|value| json!({
            "api": "account/set-leverage",
            "inst_id": normalized.symbol,
            "lever": value,
            "td_mode": normalized.td_mode,
            "pos_side": open_context.pos_side,
        })),
        "open": {
            "api": "trade/order",
            "inst_id": normalized.symbol,
            "td_mode": normalized.td_mode,
            "side": normalized.side,
            "ord_type": "market",
            "sz": nullable_text(size),
            "px": null,
            "pos_side": open_context.pos_side,
            "reduce_only": open_context.reduce_only,
        },
        "risk_order": risk_order,
        "close": {
            "enabled": !normalized.skip_close,
            "api": "trade/order",
            "inst_id": normalized.symbol,
            "td_mode": normalized.td_mode,
            "side": close_side,
            "ord_type": "market",
            "sz": nullable_text(size),
            "px": null,
            "pos_side": close_context.pos_side,
            "reduce_only": close_context.reduce_only,
            "reason": if normalized.skip_close {
                "skip_close enabled; no automatic cleanup close will be submitted"
            } else {
                "auto close after open smoke"
            },
        },
    }))
}

fn manage_order_execution_plan(
    normalized: &NormalizedSmokeArgs,
    account_pos_mode: &str,
    ticker: &Value,
    mut checks: Vec<Value>,
) -> anyhow::Result<Value> {
    let open_context = exchange_order_context(
        &normalized.inst_type,
        account_pos_mode,
        &normalized.side,
        false,
    )?;
    let limit_px = normalized.manage_limit_px.as_deref().unwrap_or("");
    if !limit_px.is_empty() {
        checks.push(passive_limit_check(
            &normalized.side,
            limit_px,
            ticker,
            "--manage-limit-px",
        ));
    }
    if let Some(amend_px) = normalized.manage_amend_px.as_deref() {
        checks.push(passive_limit_check(
            &normalized.side,
            amend_px,
            ticker,
            "--manage-amend-px",
        ));
    }
    let size = normalized.size.as_deref().unwrap_or("");
    let leverage = if normalized.inst_type == "SWAP" {
        normalized.leverage.as_deref()
    } else {
        None
    };

    Ok(json!({
        "kind": "manage_limit_order",
        "mode": normalized.mode,
        "dry_run": !normalized.execute,
        "checks": checks,
        "leverage": leverage.map(|value| json!({
            "api": "account/set-leverage",
            "inst_id": normalized.symbol,
            "lever": value,
            "td_mode": normalized.td_mode,
            "pos_side": open_context.pos_side,
        })),
        "place": {
            "api": "trade/order",
            "inst_id": normalized.symbol,
            "td_mode": normalized.td_mode,
            "side": normalized.side,
            "ord_type": "limit",
            "sz": nullable_text(size),
            "px": nullable_text(limit_px),
            "pos_side": open_context.pos_side,
            "reduce_only": open_context.reduce_only,
        },
        "amend": normalized.manage_amend_px.as_deref().map(|value| json!({
            "api": "trade/amend-order",
            "new_price": value,
            "cancel_on_fail": true,
        })),
        "cleanup": {
            "api": "trade/cancel-order",
            "required": true,
            "reason": "普通订单管理 smoke 会撤销测试限价单，避免遗留挂单",
        },
    }))
}

fn nullable_text(value: &str) -> Value {
    if value.trim().is_empty() {
        Value::Null
    } else {
        json!(value)
    }
}

fn passive_limit_check(side: &str, limit_px: &str, ticker: &Value, field: &str) -> Value {
    match validate_passive_limit_price(side, limit_px, ticker, field) {
        Ok(()) => json!({
            "key": field.trim_start_matches("--"),
            "status": "pass",
            "message": "限价单价格为被动价格，适合订单管理 smoke",
        }),
        Err(error) => json!({
            "key": field.trim_start_matches("--"),
            "status": "block",
            "message": error.to_string(),
        }),
    }
}

fn reverse_side(side: &str) -> anyhow::Result<&'static str> {
    match side {
        "buy" => Ok("sell"),
        "sell" => Ok("buy"),
        other => bail!("无法反向 side={other}"),
    }
}

async fn run_manage_limit_order_smoke(
    private_client: &OkxPrivateClient,
    normalized: &NormalizedSmokeArgs,
    account_pos_mode: &str,
    ticker: &Value,
) -> anyhow::Result<Value> {
    let limit_px = normalized
        .manage_limit_px
        .as_deref()
        .ok_or_else(|| anyhow!("普通订单管理 smoke 缺少 --manage-limit-px"))?;
    validate_passive_limit_price(&normalized.side, limit_px, ticker, "--manage-limit-px")?;
    if let Some(amend_px) = normalized.manage_amend_px.as_deref() {
        validate_passive_limit_price(&normalized.side, amend_px, ticker, "--manage-amend-px")?;
    }

    let open_context = exchange_order_context(
        &normalized.inst_type,
        account_pos_mode,
        &normalized.side,
        false,
    )?;
    let mut leverage_response = Value::Null;
    if normalized.inst_type == "SWAP" {
        if let Some(leverage) = normalized.leverage.as_deref() {
            leverage_response = private_client
                .set_leverage(
                    &normalized.symbol,
                    leverage,
                    &normalized.td_mode,
                    &open_context.pos_side,
                )
                .await?;
        }
    }

    let size = normalized
        .size
        .as_deref()
        .ok_or_else(|| anyhow!("执行普通订单管理 smoke 必须显式传入 --size"))?;
    let client_order_id = generate_okx_client_order_id();
    let place_response = private_client
        .place_order(
            &normalized.symbol,
            &normalized.td_mode,
            &normalized.side,
            "limit",
            size,
            limit_px,
            &open_context.pos_side,
            open_context.reduce_only,
            &client_order_id,
        )
        .await?;
    sleep_after_submit(normalized.sleep_ms).await;

    let (pending_after_place, pending_after_place_value) =
        query_pending_orders_value(private_client, normalized).await;
    let order_id = resolve_order_id(&place_response, &pending_after_place, &client_order_id);
    let effective_client_order_id = response_text(&place_response, "clOrdId")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| client_order_id.clone());
    let order_after_place = query_order_value(
        private_client,
        &normalized.symbol,
        order_id.as_deref().unwrap_or(""),
        &effective_client_order_id,
    )
    .await;

    let mut amend_success = true;
    let amend_block = if let Some(amend_px) = normalized.manage_amend_px.as_deref() {
        let request_id = generate_okx_client_order_id();
        match private_client
            .amend_order(
                &normalized.symbol,
                order_id.as_deref().unwrap_or(""),
                &effective_client_order_id,
                "",
                amend_px,
                true,
                &request_id,
            )
            .await
        {
            Ok(response) => {
                sleep_after_submit(normalized.sleep_ms).await;
                json!({
                    "success": true,
                    "request_id": request_id,
                    "new_price": amend_px,
                    "response": response,
                    "pending_after_amend": query_pending_orders_value(private_client, normalized).await.1,
                    "order_after_amend": query_order_value(
                        private_client,
                        &normalized.symbol,
                        order_id.as_deref().unwrap_or(""),
                        &effective_client_order_id,
                    ).await,
                })
            }
            Err(error) => {
                amend_success = false;
                json!({
                    "success": false,
                    "request_id": request_id,
                    "new_price": amend_px,
                    "error": error.to_string(),
                    "cleanup": "改单失败，已继续尝试撤销该普通限价单",
                })
            }
        }
    } else {
        Value::Null
    };

    let cancel_block = match private_client
        .cancel_order(
            &normalized.symbol,
            order_id.as_deref().unwrap_or(""),
            &effective_client_order_id,
        )
        .await
    {
        Ok(response) => {
            sleep_after_submit(normalized.sleep_ms).await;
            json!({
                "success": true,
                "response": response,
                "pending_after_cancel": query_pending_orders_value(private_client, normalized).await.1,
                "order_after_cancel": query_order_value(
                    private_client,
                    &normalized.symbol,
                    order_id.as_deref().unwrap_or(""),
                    &effective_client_order_id,
                ).await,
                "order_history_after_cancel": query_order_history_value(private_client, normalized).await,
            })
        }
        Err(error) => json!({
            "success": false,
            "error": error.to_string(),
            "cleanup": "撤单失败，请立即手动检查 OKX 当前挂单，避免 smoke 限价单遗留",
            "pending_after_cancel_error": query_pending_orders_value(private_client, normalized).await.1,
        }),
    };
    let cancel_success = cancel_block
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    Ok(json!({
        "kind": "manage_limit_order",
        "success": amend_success && cancel_success,
        "client_order_id": client_order_id,
        "order_id": order_id,
        "effective_client_order_id": effective_client_order_id,
        "side": normalized.side,
        "pos_side": open_context.pos_side,
        "reduce_only": open_context.reduce_only,
        "order_type": "limit",
        "limit_price": limit_px,
        "size": size,
        "leverage_response": leverage_response,
        "place_response": place_response,
        "pending_after_place": pending_after_place_value,
        "order_after_place": order_after_place,
        "amend": amend_block,
        "cancel": cancel_block,
    }))
}

async fn query_pending_orders_value(
    private_client: &OkxPrivateClient,
    normalized: &NormalizedSmokeArgs,
) -> (Vec<Value>, Value) {
    match private_client
        .get_pending_orders(Some(&normalized.inst_type), Some(&normalized.symbol))
        .await
    {
        Ok(rows) => {
            let value = json!(rows);
            (rows, value)
        }
        Err(error) => (Vec::new(), json!({ "error": error.to_string() })),
    }
}

async fn query_order_value(
    private_client: &OkxPrivateClient,
    symbol: &str,
    order_id: &str,
    client_order_id: &str,
) -> Value {
    match private_client
        .get_order(symbol, order_id, client_order_id)
        .await
    {
        Ok(value) => value,
        Err(error) => json!({ "error": error.to_string() }),
    }
}

async fn query_order_history_value(
    private_client: &OkxPrivateClient,
    normalized: &NormalizedSmokeArgs,
) -> Value {
    match private_client
        .get_order_history(Some(&normalized.inst_type), Some(&normalized.symbol), 20)
        .await
    {
        Ok(value) => json!(value),
        Err(error) => json!({ "error": error.to_string() }),
    }
}

async fn query_pending_algo_orders_value(
    private_client: &OkxPrivateClient,
    normalized: &NormalizedSmokeArgs,
    algo_id: &str,
) -> (Vec<Value>, Value) {
    match private_client
        .get_conditional_algo_orders_pending(
            Some(&normalized.inst_type),
            Some(&normalized.symbol),
            algo_id,
            20,
        )
        .await
    {
        Ok(rows) => {
            let value = json!(rows);
            (rows, value)
        }
        Err(error) => (Vec::new(), json!({ "error": error.to_string() })),
    }
}

async fn query_algo_order_history_value(
    private_client: &OkxPrivateClient,
    normalized: &NormalizedSmokeArgs,
    state: &str,
) -> Value {
    match private_client
        .get_conditional_algo_orders_history(
            Some(&normalized.inst_type),
            Some(&normalized.symbol),
            Some(state),
            "",
            20,
        )
        .await
    {
        Ok(value) => json!(value),
        Err(error) => json!({ "error": error.to_string() }),
    }
}

fn open_risk_close_smoke_success(risk_order_block: &Value, close_block: &Value) -> bool {
    smoke_block_success(risk_order_block) && smoke_block_success(close_block)
}

fn smoke_block_success(value: &Value) -> bool {
    if value.is_null() {
        return true;
    }
    let Some(object) = value.as_object() else {
        return true;
    };
    if object.contains_key("error") {
        return false;
    }
    object
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

fn resolve_order_id(
    place_response: &Value,
    pending_orders: &[Value],
    client_order_id: &str,
) -> Option<String> {
    response_text(place_response, "ordId")
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            let client_order_id = client_order_id.trim();
            pending_orders.iter().find_map(|item| {
                let order_client_id = response_text(item, "clOrdId")?;
                (order_client_id == client_order_id)
                    .then(|| response_text(item, "ordId"))
                    .flatten()
            })
        })
}

fn resolve_algo_id(
    place_response: &Value,
    pending_orders: &[Value],
    client_order_id: &str,
) -> Option<String> {
    response_text(place_response, "algoId")
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            let client_order_id = client_order_id.trim();
            pending_orders.iter().find_map(|item| {
                let algo_client_order_id = response_text(item, "algoClOrdId")?;
                (algo_client_order_id == client_order_id)
                    .then(|| response_text(item, "algoId"))
                    .flatten()
            })
        })
}

fn validate_passive_limit_price(
    side: &str,
    limit_px: &str,
    ticker: &Value,
    field: &str,
) -> anyhow::Result<()> {
    let limit_px_value = limit_px
        .trim()
        .parse::<f64>()
        .map_err(|_| anyhow!("{field} 必须是正数"))?;
    let last_price = ticker_last_price(ticker)
        .ok_or_else(|| anyhow!("普通订单管理 smoke 无法从 ticker.last 读取当前价格"))?;
    match side {
        "buy" if limit_px_value < last_price => Ok(()),
        "sell" if limit_px_value > last_price => Ok(()),
        "buy" => bail!(
            "{field}={} 不是被动买入价格，必须低于当前 ticker.last={last_price}",
            limit_px
        ),
        "sell" => bail!(
            "{field}={} 不是被动卖出价格，必须高于当前 ticker.last={last_price}",
            limit_px
        ),
        other => bail!("普通订单管理 smoke 不支持 side={other}"),
    }
}

fn ticker_last_price(ticker: &Value) -> Option<f64> {
    ["last", "last_price", "lastPrice"]
        .iter()
        .find_map(|key| response_text(ticker, key))
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn response_text(value: &Value, key: &str) -> Option<String> {
    match value.get(key)? {
        Value::String(item) => {
            let item = item.trim();
            (!item.is_empty()).then_some(item.to_string())
        }
        Value::Number(item) => Some(item.to_string()),
        Value::Bool(item) => Some(item.to_string()),
        _ => None,
    }
}

async fn sleep_after_submit(sleep_ms: u64) {
    tokio::time::sleep(std::time::Duration::from_millis(sleep_ms)).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse<const N: usize>(items: [&str; N]) -> anyhow::Result<LiveTradeSmokeArgs> {
        LiveTradeSmokeArgs::from_env_args(items.map(str::to_string))
    }

    #[test]
    fn parser_defaults_to_safe_dry_run() {
        let args = parse(["live_trade_smoke"]).expect("default args");

        assert!(!args.execute);
        assert_eq!(args.mode, "simulated");
        assert_eq!(args.symbol, DEFAULT_SYMBOL);
        assert_eq!(args.inst_type, "SWAP");
    }

    #[test]
    fn mode_paper_is_not_a_live_trade_alias() {
        let args = parse(["live_trade_smoke", "--mode", "paper"]).expect("parse args");

        let error = NormalizedSmokeArgs::from_args(args)
            .expect_err("live trade smoke must not keep local paper mode aliases")
            .to_string();

        assert!(error.contains("只支持 simulated 或 live"));
    }

    #[test]
    fn mode_demo_simulation_and_real_are_not_live_trade_aliases() {
        for mode in ["demo", "simulation", "real"] {
            let args = parse(["live_trade_smoke", "--mode", mode]).expect("parse args");

            let error = NormalizedSmokeArgs::from_args(args)
                .expect_err("live trade smoke must only accept explicit live modes")
                .to_string();

            assert!(
                error.contains("只支持 simulated 或 live"),
                "unexpected error for mode={mode}: {error}"
            );
        }
    }

    #[test]
    fn execute_requires_explicit_symbol() {
        let args = parse([
            "live_trade_smoke",
            "--execute",
            "--size",
            "1",
            "--confirm",
            SIMULATED_EXECUTE_CONFIRM,
        ])
        .expect("parse args");

        let error = NormalizedSmokeArgs::from_args(args)
            .expect_err("implicit default symbol must be rejected")
            .to_string();

        assert!(error.contains("--symbol"));
    }

    #[test]
    fn execute_requires_size() {
        let args = parse([
            "live_trade_smoke",
            "--execute",
            "--symbol",
            "BTC-USDT-SWAP",
            "--confirm",
            SIMULATED_EXECUTE_CONFIRM,
        ])
        .expect("parse args");

        let error = NormalizedSmokeArgs::from_args(args)
            .expect_err("missing size must be rejected")
            .to_string();

        assert!(error.contains("--size"));
    }

    #[test]
    fn live_execute_requires_live_confirmation() {
        let args = parse([
            "live_trade_smoke",
            "--mode",
            "live",
            "--execute",
            "--symbol",
            "BTC-USDT-SWAP",
            "--size",
            "1",
            "--confirm",
            SIMULATED_EXECUTE_CONFIRM,
        ])
        .expect("parse args");

        let error = NormalizedSmokeArgs::from_args(args)
            .expect_err("live mode must require live confirm")
            .to_string();

        assert!(error.contains(LIVE_EXECUTE_CONFIRM));
    }

    #[test]
    fn live_execute_rejects_skip_close() {
        let args = parse([
            "live_trade_smoke",
            "--mode",
            "live",
            "--execute",
            "--symbol",
            "BTC-USDT-SWAP",
            "--size",
            "1",
            "--confirm",
            LIVE_EXECUTE_CONFIRM,
            "--skip-close",
        ])
        .expect("parse args");

        let error = NormalizedSmokeArgs::from_args(args)
            .expect_err("live skip close must be rejected")
            .to_string();

        assert!(error.contains("--skip-close"));
    }

    #[test]
    fn normalizes_swap_symbol_and_side() {
        let args = parse([
            "live_trade_smoke",
            "--symbol",
            "btc-usdt",
            "--side",
            "short",
            "--size",
            "2",
        ])
        .expect("parse args");

        let normalized = NormalizedSmokeArgs::from_args(args).expect("normalize args");

        assert_eq!(normalized.symbol, "BTC-USDT-SWAP");
        assert_eq!(normalized.side, "sell");
        assert_eq!(normalized.td_mode, "cross");
        assert_eq!(normalized.size.as_deref(), Some("2"));
    }

    #[test]
    fn risk_order_args_are_explicit_opt_in() {
        let args = parse([
            "live_trade_smoke",
            "--symbol",
            "BTC-USDT-SWAP",
            "--size",
            "1",
            "--risk-stop-px",
            "65000",
            "--risk-amend-stop-px",
            "64500",
        ])
        .expect("parse args");

        let normalized = NormalizedSmokeArgs::from_args(args).expect("normalize args");

        assert_eq!(normalized.risk_stop_px.as_deref(), Some("65000"));
        assert_eq!(normalized.risk_amend_stop_px.as_deref(), Some("64500"));
    }

    #[test]
    fn risk_amend_requires_risk_stop() {
        let args = parse([
            "live_trade_smoke",
            "--symbol",
            "BTC-USDT-SWAP",
            "--risk-amend-stop-px",
            "64500",
        ])
        .expect("parse args");

        let error = NormalizedSmokeArgs::from_args(args)
            .expect_err("risk amend without risk stop should be rejected")
            .to_string();

        assert!(error.contains("--risk-stop-px"));
    }

    #[test]
    fn manage_order_args_are_explicit_opt_in() {
        let args = parse([
            "live_trade_smoke",
            "--symbol",
            "BTC-USDT-SWAP",
            "--size",
            "1",
            "--manage-limit-px",
            "60000",
            "--manage-amend-px",
            "59000",
        ])
        .expect("parse args");

        let normalized = NormalizedSmokeArgs::from_args(args).expect("normalize args");

        assert!(normalized.is_manage_order_smoke());
        assert_eq!(normalized.manage_limit_px.as_deref(), Some("60000"));
        assert_eq!(normalized.manage_amend_px.as_deref(), Some("59000"));
    }

    #[test]
    fn manage_amend_requires_manage_limit() {
        let args = parse([
            "live_trade_smoke",
            "--symbol",
            "BTC-USDT-SWAP",
            "--manage-amend-px",
            "59000",
        ])
        .expect("parse args");

        let error = NormalizedSmokeArgs::from_args(args)
            .expect_err("manage amend without manage limit should be rejected")
            .to_string();

        assert!(error.contains("--manage-limit-px"));
    }

    #[test]
    fn manage_order_rejects_combining_with_risk_smoke() {
        let args = parse([
            "live_trade_smoke",
            "--symbol",
            "BTC-USDT-SWAP",
            "--size",
            "1",
            "--manage-limit-px",
            "60000",
            "--risk-stop-px",
            "55000",
        ])
        .expect("parse args");

        let error = NormalizedSmokeArgs::from_args(args)
            .expect_err("manage smoke and risk smoke should be separate")
            .to_string();

        assert!(error.contains("不能和保护单 smoke 混用"));
    }

    #[test]
    fn execute_manage_order_rejects_spot() {
        let args = parse([
            "live_trade_smoke",
            "--execute",
            "--symbol",
            "BTC-USDT",
            "--inst-type",
            "SPOT",
            "--size",
            "0.01",
            "--manage-limit-px",
            "60000",
            "--confirm",
            SIMULATED_EXECUTE_CONFIRM,
        ])
        .expect("parse args");

        let error = NormalizedSmokeArgs::from_args(args)
            .expect_err("spot manage smoke should be rejected")
            .to_string();

        assert!(error.contains("只支持 SWAP"));
    }

    #[test]
    fn execute_risk_smoke_rejects_skip_close() {
        let args = parse([
            "live_trade_smoke",
            "--execute",
            "--symbol",
            "BTC-USDT-SWAP",
            "--size",
            "1",
            "--risk-stop-px",
            "65000",
            "--skip-close",
            "--confirm",
            SIMULATED_EXECUTE_CONFIRM,
        ])
        .expect("parse args");

        let error = NormalizedSmokeArgs::from_args(args)
            .expect_err("risk smoke with skip close should be rejected")
            .to_string();

        assert!(error.contains("--skip-close"));
    }

    #[test]
    fn resolve_algo_id_falls_back_to_pending_algo_client_id() {
        let algo_id = resolve_algo_id(
            &json!({"algoClOrdId": "client-risk"}),
            &[json!({"algoId": "algo-1", "algoClOrdId": "client-risk"})],
            "client-risk",
        );

        assert_eq!(algo_id.as_deref(), Some("algo-1"));
    }

    #[test]
    fn resolve_order_id_falls_back_to_pending_client_id() {
        let order_id = resolve_order_id(
            &json!({"clOrdId": "client-order"}),
            &[json!({"ordId": "order-1", "clOrdId": "client-order"})],
            "client-order",
        );

        assert_eq!(order_id.as_deref(), Some("order-1"));
    }

    #[test]
    fn manage_limit_price_must_be_passive_against_ticker_last() {
        let ticker = json!({"last": "100"});

        assert!(validate_passive_limit_price("buy", "99", &ticker, "--manage-limit-px").is_ok());
        assert!(validate_passive_limit_price("sell", "101", &ticker, "--manage-limit-px").is_ok());

        let buy_error = validate_passive_limit_price("buy", "100", &ticker, "--manage-limit-px")
            .expect_err("crossing buy should be rejected")
            .to_string();
        let sell_error = validate_passive_limit_price("sell", "100", &ticker, "--manage-limit-px")
            .expect_err("crossing sell should be rejected")
            .to_string();

        assert!(buy_error.contains("被动买入价格"));
        assert!(sell_error.contains("被动卖出价格"));
    }

    #[test]
    fn hedge_mode_pos_side_matches_live_engine_rules() {
        assert_eq!(
            exchange_order_context("SWAP", "long_short_mode", "buy", false).unwrap(),
            ExchangeOrderContext {
                pos_side: "long".to_string(),
                reduce_only: false,
            }
        );
        assert_eq!(
            exchange_order_context("SWAP", "long_short_mode", "sell", true).unwrap(),
            ExchangeOrderContext {
                pos_side: "long".to_string(),
                reduce_only: false,
            }
        );
    }

    #[test]
    fn net_mode_keeps_reduce_only_for_close() {
        assert_eq!(
            exchange_order_context("SWAP", "net_mode", "sell", true).unwrap(),
            ExchangeOrderContext {
                pos_side: String::new(),
                reduce_only: true,
            }
        );
    }

    #[test]
    fn dry_run_execution_plan_previews_swap_open_risk_and_close_requests() {
        let args = parse([
            "live_trade_smoke",
            "--symbol",
            "BTC-USDT-SWAP",
            "--side",
            "long",
            "--size",
            "1",
            "--leverage",
            "3",
            "--risk-stop-px",
            "65000",
            "--risk-amend-stop-px",
            "64500",
        ])
        .expect("parse args");
        let normalized = NormalizedSmokeArgs::from_args(args).expect("normalize args");

        let plan = smoke_execution_plan(&normalized, "long_short_mode", &json!({"last": "70000"}))
            .expect("plan should build");

        assert_eq!(plan["kind"], "open_risk_close");
        assert_eq!(plan["open"]["side"], "buy");
        assert_eq!(plan["open"]["pos_side"], "long");
        assert_eq!(plan["open"]["reduce_only"], false);
        assert_eq!(plan["leverage"]["lever"], "3");
        assert_eq!(plan["risk_order"]["place"]["side"], "sell");
        assert_eq!(plan["risk_order"]["place"]["pos_side"], "long");
        assert_eq!(plan["risk_order"]["amend"]["new_stop_price"], "64500");
        assert_eq!(plan["close"]["side"], "sell");
        assert_eq!(plan["close"]["pos_side"], "long");
    }

    #[test]
    fn dry_run_manage_limit_plan_reports_passive_price_check() {
        let args = parse([
            "live_trade_smoke",
            "--symbol",
            "BTC-USDT-SWAP",
            "--side",
            "buy",
            "--size",
            "1",
            "--manage-limit-px",
            "60000",
            "--manage-amend-px",
            "59000",
        ])
        .expect("parse args");
        let normalized = NormalizedSmokeArgs::from_args(args).expect("normalize args");

        let plan = smoke_execution_plan(&normalized, "net_mode", &json!({"last": "70000"}))
            .expect("plan should build");

        assert_eq!(plan["kind"], "manage_limit_order");
        assert_eq!(plan["place"]["ord_type"], "limit");
        assert_eq!(plan["place"]["px"], "60000");
        assert_eq!(plan["amend"]["new_price"], "59000");
        assert_eq!(plan["cleanup"]["required"], true);
        assert_eq!(plan["checks"][0]["status"], "pass");
        assert_eq!(plan["checks"][1]["status"], "pass");
    }

    #[test]
    fn open_risk_close_smoke_success_requires_risk_and_close_success() {
        assert!(open_risk_close_smoke_success(
            &Value::Null,
            &json!({"success": true})
        ));

        assert!(!open_risk_close_smoke_success(
            &json!({"success": false, "error": "risk amend failed"}),
            &json!({"success": true})
        ));
        assert!(!open_risk_close_smoke_success(
            &json!({"success": true}),
            &json!({"success": false, "error": "close failed"})
        ));
        assert!(!open_risk_close_smoke_success(
            &json!({"error": "risk failed without explicit success"}),
            &json!({"success": true})
        ));
    }

    #[test]
    fn dry_run_manage_limit_plan_marks_aggressive_price_as_blocked() {
        let args = parse([
            "live_trade_smoke",
            "--symbol",
            "BTC-USDT-SWAP",
            "--side",
            "buy",
            "--manage-limit-px",
            "80000",
        ])
        .expect("parse args");
        let normalized = NormalizedSmokeArgs::from_args(args).expect("normalize args");

        let plan = smoke_execution_plan(&normalized, "net_mode", &json!({"last": "70000"}))
            .expect("plan should build");

        assert_eq!(plan["place"]["sz"], Value::Null);
        assert_eq!(plan["checks"][0]["status"], "info");
        assert_eq!(plan["checks"][1]["status"], "block");
        assert!(plan["checks"][1]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("不是被动买入价格"));
    }
}
