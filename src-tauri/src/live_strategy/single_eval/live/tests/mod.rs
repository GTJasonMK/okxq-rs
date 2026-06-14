use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::json;
use sqlx::Row;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
};

use super::*;
use crate::{
    config::ApiCredentials,
    live_strategy::{
        arrival::ArrivalQuote, decision::StrategyIntentAction, types::LiveStrategyConfig,
        LiveStrategyRuntime,
    },
    okx::{OkxPrivateClient, OkxPublicClient},
    strategy_engine::StrategyActionRecord,
};

mod exchange_orders;
mod order_management;
mod outcome;
mod planned_exits;
mod risk_orders;

fn test_config() -> LiveStrategyConfig {
    LiveStrategyConfig {
        strategy_id: "live_action_risk_test".to_string(),
        strategy_name: "Live Action Risk Test".to_string(),
        symbol: "BTC-USDT-SWAP".to_string(),
        timeframe: "15m".to_string(),
        inst_type: "SWAP".to_string(),
        mode: "simulated".to_string(),
        initial_capital: 1000.0,
        position_size: 0.20,
        stop_loss: 0.0,
        take_profit: 0.0,
        risk_timeframe: "1m".to_string(),
        check_interval: 60,
        params: json!({
            "contract_mode": true,
            "leverage": 1,
            "max_same_direction_exposure_pct": 0.60,
        }),
        project_root: PathBuf::from("."),
        risk_control_enabled: false,
        max_single_loss_ratio: 0.0,
        max_position_pct: 0.0,
        max_order_value: 0.0,
    }
}

fn temp_db_path(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir()
        .join(format!("okxq_{name}_{}_{}", std::process::id(), suffix))
        .join("market.db")
}

async fn insert_test_local_fill(
    pool: &SqlitePool,
    trade_id: &str,
    inst_id: &str,
    side: &str,
    fill_sz: &str,
    order_id: &str,
    client_order_id: &str,
    mode: &str,
) {
    sqlx::query(
        r#"
        INSERT INTO local_fills (
          trade_id, inst_id, ccy, side, fill_px, fill_sz, fee, fee_ccy,
          ts, mode, source, order_id, client_order_id, strategy_id, run_id
        ) VALUES (?, ?, 'USDT', ?, '100', ?, '0', 'USDT',
                  1780000000000, ?, 'unit_test', ?, ?, 'strategy-1', 'run-fill')
        "#,
    )
    .bind(trade_id)
    .bind(inst_id)
    .bind(side)
    .bind(fill_sz)
    .bind(mode)
    .bind(order_id)
    .bind(client_order_id)
    .execute(pool)
    .await
    .expect("test local fill should insert");
}

async fn start_mock_okx_server() -> (String, oneshot::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock server should bind");
    let addr = listener.local_addr().expect("mock server address");
    let (stop_tx, mut stop_rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut stop_rx => break,
                accepted = listener.accept() => {
                    let Ok((mut stream, _)) = accepted else {
                        break;
                    };
                    tokio::spawn(async move {
                        let mut request = vec![0_u8; 4096];
                        let bytes_read = stream.read(&mut request).await.unwrap_or(0);
                        let request_text = String::from_utf8_lossy(&request[..bytes_read]);
                        let body = mock_okx_response(&request_text);
                        let response = format!(
                            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        let _ = stream.write_all(response.as_bytes()).await;
                        let _ = stream.shutdown().await;
                    });
                }
            }
        }
    });
    (format!("http://{addr}"), stop_tx)
}

async fn start_recording_mock_okx_server() -> (String, oneshot::Sender<()>, Arc<Mutex<Vec<String>>>)
{
    start_recording_mock_okx_server_with_order_failure(false).await
}

async fn start_recording_mock_okx_server_with_order_failure(
    fail_order: bool,
) -> (String, oneshot::Sender<()>, Arc<Mutex<Vec<String>>>) {
    let order_responses = if fail_order {
        vec![mock_okx_order_reject_response()]
    } else {
        Vec::new()
    };
    start_recording_mock_okx_server_with_order_responses(order_responses).await
}

async fn start_recording_mock_okx_server_with_order_responses(
    order_responses: Vec<String>,
) -> (String, oneshot::Sender<()>, Arc<Mutex<Vec<String>>>) {
    start_recording_mock_okx_server_with_trade_responses(
        order_responses,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .await
}

async fn start_recording_mock_okx_server_with_algo_responses(
    algo_responses: Vec<String>,
) -> (String, oneshot::Sender<()>, Arc<Mutex<Vec<String>>>) {
    start_recording_mock_okx_server_with_trade_responses(
        Vec::new(),
        Vec::new(),
        Vec::new(),
        algo_responses,
    )
    .await
}

async fn start_recording_mock_okx_server_with_trade_responses(
    order_responses: Vec<String>,
    cancel_responses: Vec<String>,
    amend_responses: Vec<String>,
    algo_responses: Vec<String>,
) -> (String, oneshot::Sender<()>, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock server should bind");
    let addr = listener.local_addr().expect("mock server address");
    let requests = Arc::new(Mutex::new(Vec::<String>::new()));
    let server_requests = Arc::clone(&requests);
    let order_responses = Arc::new(Mutex::new(VecDeque::from(order_responses)));
    let cancel_responses = Arc::new(Mutex::new(VecDeque::from(cancel_responses)));
    let amend_responses = Arc::new(Mutex::new(VecDeque::from(amend_responses)));
    let algo_responses = Arc::new(Mutex::new(VecDeque::from(algo_responses)));
    let (stop_tx, mut stop_rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut stop_rx => break,
                accepted = listener.accept() => {
                    let Ok((mut stream, _)) = accepted else {
                        break;
                    };
                    let request_log = Arc::clone(&server_requests);
                    let order_response_queue = Arc::clone(&order_responses);
                    let cancel_response_queue = Arc::clone(&cancel_responses);
                    let amend_response_queue = Arc::clone(&amend_responses);
                    let algo_response_queue = Arc::clone(&algo_responses);
                    tokio::spawn(async move {
                        let mut request = vec![0_u8; 8192];
                        let bytes_read = stream.read(&mut request).await.unwrap_or(0);
                        let request_text = String::from_utf8_lossy(&request[..bytes_read]).to_string();
                        request_log.lock().expect("request log").push(request_text.clone());
                        let body = if request_text.contains("/api/v5/trade/order-algo") {
                            algo_response_queue
                                .lock()
                                .expect("algo response queue")
                                .pop_front()
                                .unwrap_or_else(|| mock_okx_response(&request_text))
                        } else if request_text.contains("/api/v5/trade/cancel-algos") {
                            algo_response_queue
                                .lock()
                                .expect("algo response queue")
                                .pop_front()
                                .unwrap_or_else(|| mock_okx_response(&request_text))
                        } else if request_text.contains("/api/v5/trade/amend-algos") {
                            algo_response_queue
                                .lock()
                                .expect("algo response queue")
                                .pop_front()
                                .unwrap_or_else(|| mock_okx_response(&request_text))
                        } else if request_text.contains("/api/v5/trade/cancel-order") {
                            cancel_response_queue
                                .lock()
                                .expect("cancel response queue")
                                .pop_front()
                                .unwrap_or_else(|| mock_okx_response(&request_text))
                        } else if request_text.contains("/api/v5/trade/amend-order") {
                            amend_response_queue
                                .lock()
                                .expect("amend response queue")
                                .pop_front()
                                .unwrap_or_else(|| mock_okx_response(&request_text))
                        } else if request_text.contains("/api/v5/trade/order") {
                            order_response_queue
                                .lock()
                                .expect("order response queue")
                                .pop_front()
                                .unwrap_or_else(|| mock_okx_response(&request_text))
                        } else {
                            mock_okx_response(&request_text)
                        };
                        let response = format!(
                            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        let _ = stream.write_all(response.as_bytes()).await;
                        let _ = stream.shutdown().await;
                    });
                }
            }
        }
    });
    (format!("http://{addr}"), stop_tx, requests)
}

fn mock_okx_order_reject_response() -> String {
    json!({
        "code": "0",
        "msg": "",
        "data": [{
            "ordId": "",
            "clOrdId": "",
            "sCode": "51000",
            "sMsg": "unit test order rejected"
        }]
    })
    .to_string()
}

fn mock_okx_order_submit_response(order_id: &str) -> String {
    json!({
        "code": "0",
        "msg": "",
        "data": [{
            "ordId": order_id,
            "clOrdId": "",
            "sCode": "0",
            "sMsg": ""
        }]
    })
    .to_string()
}

fn mock_okx_algo_order_reject_response() -> String {
    json!({
        "code": "0",
        "msg": "",
        "data": [{
            "algoId": "",
            "algoClOrdId": "",
            "sCode": "51000",
            "sMsg": "unit test algo order rejected"
        }]
    })
    .to_string()
}

fn mock_okx_response(request: &str) -> String {
    if request.contains("/api/v5/public/instruments") {
        return json!({
            "code": "0",
            "msg": "",
            "data": [
                {
                    "instId": "BTC-USDT-SWAP",
                    "instType": "SWAP",
                    "state": "live",
                    "minSz": "0.01",
                    "lotSz": "0.01",
                    "tickSz": "0.01",
                    "ctVal": "0.01",
                    "ctValCcy": "BTC"
                },
                {
                    "instId": "ETH-USDT-SWAP",
                    "instType": "SWAP",
                    "state": "live",
                    "minSz": "0.01",
                    "lotSz": "0.01",
                    "tickSz": "0.01",
                    "ctVal": "0.01",
                    "ctValCcy": "ETH"
                }
            ]
        })
        .to_string();
    }
    if request.contains("/api/v5/account/positions") {
        if request.contains("BTC-USDT-SWAP") {
            return json!({
                "code": "0",
                "msg": "",
                "data": [{
                    "instId": "BTC-USDT-SWAP",
                    "instType": "SWAP",
                    "posSide": "long",
                    "pos": "5",
                    "availPos": "5",
                    "avgPx": "100",
                    "markPx": "100"
                }]
            })
            .to_string();
        }
        return json!({
            "code": "0",
            "msg": "",
            "data": [{
                "instId": "ETH-USDT-SWAP",
                "instType": "SWAP",
                "posSide": "long",
                "pos": "5",
                "avgPx": "100",
                "markPx": "100"
            }]
        })
        .to_string();
    }
    if request.contains("/api/v5/account/balance") {
        return json!({
            "code": "0",
            "msg": "",
            "data": [{
                "details": [{
                    "ccy": "BTC",
                    "cashBal": "2",
                    "eq": "2",
                    "frozenBal": "2"
                }]
            }]
        })
        .to_string();
    }
    if request.contains("/api/v5/account/max-avail-size") {
        return json!({
            "code": "0",
            "msg": "",
            "data": [{
                "instId": "BTC-USDT",
                "availSell": ""
            }]
        })
        .to_string();
    }
    if request.contains("/api/v5/market/books") {
        return json!({
            "code": "0",
            "msg": "",
            "data": [{
                "instId": "BTC-USDT-SWAP",
                "asks": [["100.5", "1", "0", "1"]],
                "bids": [["99.5", "1", "0", "1"]],
                "ts": "1780000000000"
            }]
        })
        .to_string();
    }
    if request.contains("/api/v5/account/config") {
        return json!({
            "code": "0",
            "msg": "",
            "data": [{"posMode": "long_short_mode"}]
        })
        .to_string();
    }
    if request.contains("/api/v5/account/set-leverage") {
        return json!({
            "code": "0",
            "msg": "",
            "data": [{
                "instId": "BTC-USDT-SWAP",
                "lever": "7",
                "mgnMode": "isolated",
                "posSide": "long"
            }]
        })
        .to_string();
    }
    if request.contains("/api/v5/trade/cancel-algos") {
        let (algo_id, algo_client_order_id) = request_json_body(request)
            .and_then(|body| body.as_array().and_then(|items| items.first()).cloned())
            .map(|item| {
                (
                    item.get("algoId")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                        .unwrap_or_else(|| "algo-1".to_string()),
                    item.get("algoClOrdId")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                        .unwrap_or_default(),
                )
            })
            .unwrap_or_else(|| ("algo-1".to_string(), String::new()));
        return json!({
            "code": "0",
            "msg": "",
            "data": [{
                "algoId": algo_id,
                "algoClOrdId": algo_client_order_id,
                "sCode": "0",
                "sMsg": ""
            }]
        })
        .to_string();
    }
    if request.contains("/api/v5/trade/amend-algos") {
        let (algo_id, algo_client_order_id, request_id) = request_json_body(request)
            .map(|body| {
                (
                    body.get("algoId")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("algo-1")
                        .to_string(),
                    body.get("algoClOrdId")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("clalgo1")
                        .to_string(),
                    body.get("reqId")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                )
            })
            .unwrap_or_else(|| ("algo-1".to_string(), "clalgo1".to_string(), String::new()));
        return json!({
            "code": "0",
            "msg": "",
            "data": [{
                "algoId": algo_id,
                "algoClOrdId": algo_client_order_id,
                "reqId": request_id,
                "sCode": "0",
                "sMsg": ""
            }]
        })
        .to_string();
    }
    if request.contains("/api/v5/trade/cancel-order") {
        let (order_id, client_order_id) = request_json_body(request)
            .map(|body| {
                (
                    body.get("ordId")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("cancel-target-order")
                        .to_string(),
                    body.get("clOrdId")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("canceltargetclient")
                        .to_string(),
                )
            })
            .unwrap_or_else(|| {
                (
                    "cancel-target-order".to_string(),
                    "canceltargetclient".to_string(),
                )
            });
        return json!({
            "code": "0",
            "msg": "",
            "data": [{
                "ordId": order_id,
                "clOrdId": client_order_id,
                "sCode": "0",
                "sMsg": ""
            }]
        })
        .to_string();
    }
    if request.contains("/api/v5/trade/amend-order") {
        let (order_id, client_order_id, request_id) = request_json_body(request)
            .map(|body| {
                (
                    body.get("ordId")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("modify-target-order")
                        .to_string(),
                    body.get("clOrdId")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("modifytargetclient")
                        .to_string(),
                    body.get("reqId")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("modifyreq1")
                        .to_string(),
                )
            })
            .unwrap_or_else(|| {
                (
                    "modify-target-order".to_string(),
                    "modifytargetclient".to_string(),
                    "modifyreq1".to_string(),
                )
            });
        return json!({
            "code": "0",
            "msg": "",
            "data": [{
                "ordId": order_id,
                "clOrdId": client_order_id,
                "reqId": request_id,
                "sCode": "0",
                "sMsg": ""
            }]
        })
        .to_string();
    }
    if request.contains("/api/v5/trade/order-algo") {
        let client_order_id = request_json_body(request)
            .and_then(|body| {
                body.get("algoClOrdId")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "mock-algo-client-order".to_string());
        return json!({
            "code": "0",
            "msg": "",
            "data": [{
                "algoId": "algo-1",
                "algoClOrdId": client_order_id,
                "sCode": "0",
                "sMsg": ""
            }]
        })
        .to_string();
    }
    if request.contains("/api/v5/trade/order") {
        let client_order_id = request_json_body(request)
            .and_then(|body| {
                body.get("clOrdId")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "mock-client-order".to_string());
        return json!({
            "code": "0",
            "msg": "",
            "data": [{
                "ordId": "submitted-1",
                "clOrdId": client_order_id,
                "sCode": "0",
                "sMsg": ""
            }]
        })
        .to_string();
    }
    json!({"code": "0", "msg": "", "data": []}).to_string()
}

fn request_json_body(request: &str) -> Option<serde_json::Value> {
    let body = request.split("\r\n\r\n").nth(1)?.trim();
    if body.is_empty() {
        return None;
    }
    serde_json::from_str(body).ok()
}

fn is_exchange_order_request(request: &str) -> bool {
    request.contains("/api/v5/trade/order")
        && !request.contains("/api/v5/trade/order-algo")
        && !request.contains("/api/v5/trade/cancel-order")
        && !request.contains("/api/v5/trade/amend-order")
}
