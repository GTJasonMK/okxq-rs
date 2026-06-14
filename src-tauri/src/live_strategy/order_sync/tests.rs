use std::{
    path::PathBuf,
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
        arrival::ArrivalQuote,
        decision::StrategyPlannedExitIntent,
        storage::{
            insert_live_attached_algo_order, insert_live_exchange_order,
            insert_live_planned_exit_plan, mark_live_planned_exit_submitted,
        },
        types::LiveStrategyConfig,
    },
    okx::OkxPrivateClient,
    strategy_engine::StrategyActionRecord,
};

mod algo_sync;
mod exchange_sync;
mod fill_sync;
mod planned_exit_sync;
mod private_ws;
mod states;

async fn start_order_detail_mock_server() -> (String, oneshot::Sender<()>) {
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
                        let body = if request_text.contains("/api/v5/trade/fills") {
                            json!({
                                "code": "0",
                                "msg": "",
                                "data": [{
                                    "instId": "BTC-USDT-SWAP",
                                    "tradeId": "trade-exit",
                                    "ordId": "exit-order",
                                    "clOrdId": "exitclientorder",
                                    "side": "sell",
                                    "fillPx": "100.5",
                                    "fillSz": "1",
                                    "fee": "-0.01",
                                    "feeCcy": "USDT",
                                    "ts": "1780000900123"
                                }]
                            }).to_string()
                        } else if request_text.contains("/api/v5/trade/orders-algo-pending") {
                            json!({
                                "code": "0",
                                "msg": "",
                                "data": [{
                                    "instId": "BTC-USDT-SWAP",
                                    "instType": "SWAP",
                                    "algoId": "algo-live",
                                    "algoClOrdId": "clalgolive",
                                    "ordType": "conditional",
                                    "state": "live",
                                    "sz": "1",
                                    "actualSz": "0"
                                }]
                            }).to_string()
                        } else if request_text.contains("/api/v5/trade/orders-algo-history") {
                            json!({"code": "0", "msg": "", "data": []}).to_string()
                        } else if request_text.contains("/api/v5/trade/order") {
                            json!({
                                "code": "0",
                                "msg": "",
                                "data": [{
                                    "instId": "BTC-USDT-SWAP",
                                    "ordId": "exit-order",
                                    "clOrdId": "exitclientorder",
                                    "state": "filled",
                                    "avgPx": "100.5",
                                    "accFillSz": "1"
                                }]
                            }).to_string()
                        } else {
                            json!({"code": "0", "msg": "", "data": []}).to_string()
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
    (format!("http://{addr}"), stop_tx)
}

async fn start_order_not_found_mock_server() -> (String, oneshot::Sender<()>) {
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
                        let body = if request_text.contains("/api/v5/trade/order") {
                            json!({
                                "code": "51603",
                                "msg": "Order does not exist",
                                "data": []
                            }).to_string()
                        } else {
                            json!({"code": "0", "msg": "", "data": []}).to_string()
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
    (format!("http://{addr}"), stop_tx)
}

async fn start_order_not_found_with_history_mock_server() -> (String, oneshot::Sender<()>) {
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
                        let body = if request_text.contains("/api/v5/trade/orders-history") {
                            json!({
                                "code": "0",
                                "msg": "",
                                "data": [
                                    {
                                        "instId": "BTC-USDT-SWAP",
                                        "ordId": "cancel-history-order",
                                        "clOrdId": "cancelhistoryclient",
                                        "state": "canceled",
                                        "avgPx": "",
                                        "accFillSz": "0"
                                    },
                                    {
                                        "instId": "BTC-USDT-SWAP",
                                        "ordId": "modify-history-order",
                                        "clOrdId": "modifyhistoryclient",
                                        "state": "filled",
                                        "avgPx": "101.5",
                                        "accFillSz": "2"
                                    },
                                    {
                                        "instId": "BTC-USDT-SWAP",
                                        "ordId": "history-exit-order",
                                        "clOrdId": "historyexitclient",
                                        "state": "filled",
                                        "avgPx": "102.25",
                                        "accFillSz": "1"
                                    }
                                ]
                            }).to_string()
                        } else if request_text.contains("/api/v5/trade/order") {
                            json!({
                                "code": "51603",
                                "msg": "Order does not exist",
                                "data": []
                            }).to_string()
                        } else {
                            json!({"code": "0", "msg": "", "data": []}).to_string()
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
    (format!("http://{addr}"), stop_tx)
}

fn test_config() -> LiveStrategyConfig {
    LiveStrategyConfig {
        strategy_id: "order_sync_runtime_test".to_string(),
        strategy_name: "Order Sync Runtime Test".to_string(),
        symbol: "BTC-USDT-SWAP".to_string(),
        timeframe: "15m".to_string(),
        inst_type: "SWAP".to_string(),
        mode: "simulated".to_string(),
        initial_capital: 1000.0,
        position_size: 0.2,
        stop_loss: 0.0,
        take_profit: 0.0,
        risk_timeframe: "1m".to_string(),
        check_interval: 60,
        params: json!({}),
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
