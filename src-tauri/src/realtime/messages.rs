use anyhow::{anyhow, Result};
use serde_json::Value;

use super::{normalize::*, RealtimeManager};

#[derive(Clone, Copy)]
enum PrivateMessageKind {
    Regular,
    Business,
}

enum PrivateWsPrefix {
    Subscribe,
    Ignore,
    Data(Value),
}

impl PrivateMessageKind {
    fn error_message(self) -> &'static str {
        match self {
            Self::Regular => "OKX private websocket error",
            Self::Business => "OKX business private websocket error",
        }
    }

    fn login_error_message(self) -> &'static str {
        match self {
            Self::Regular => "OKX private websocket login failed",
            Self::Business => "OKX business private websocket login failed",
        }
    }
}

impl RealtimeManager {
    pub(super) async fn handle_text_message(&self, generation: u64, text: &str) -> Result<()> {
        let payload = serde_json::from_str::<Value>(text)?;
        self.mark_message(generation).await;

        if payload
            .get("event")
            .and_then(Value::as_str)
            .is_some_and(|event| event == "error")
        {
            let message = payload
                .get("msg")
                .and_then(Value::as_str)
                .unwrap_or("OKX websocket error")
                .to_string();
            self.set_last_error(generation, Some(message.clone())).await;
            return Err(anyhow!(message));
        }

        let channel = payload
            .get("arg")
            .and_then(|arg| arg.get("channel"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let arg_inst_id = payload
            .get("arg")
            .and_then(|arg| arg.get("instId"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let data = payload
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        match channel {
            "tickers" => {
                for item in data {
                    if let Some(ticker) = normalize_ticker(item, arg_inst_id) {
                        self.emit_ticker(ticker).await;
                    }
                }
            }
            "trades" => {
                for item in data {
                    if let Some(trade) = normalize_trade(item, arg_inst_id) {
                        self.emit_trade(trade).await;
                    }
                }
            }
            channel if channel.starts_with("books") => {
                for item in data {
                    if let Some(orderbook) = normalize_orderbook(item, arg_inst_id, channel) {
                        self.emit_orderbook(orderbook).await;
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    pub(super) async fn handle_candle_text_message(
        &self,
        generation: u64,
        text: &str,
    ) -> Result<()> {
        let payload = serde_json::from_str::<Value>(text)?;
        self.mark_candle_message(generation).await;

        if payload
            .get("event")
            .and_then(Value::as_str)
            .is_some_and(|event| event == "error")
        {
            let message = payload
                .get("msg")
                .and_then(Value::as_str)
                .unwrap_or("OKX business websocket error")
                .to_string();
            self.set_candle_last_error(generation, Some(message.clone()))
                .await;
            return Err(anyhow!(message));
        }

        let channel = payload
            .get("arg")
            .and_then(|arg| arg.get("channel"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if !channel.starts_with("candle") {
            return Ok(());
        }

        let arg_inst_id = payload
            .get("arg")
            .and_then(|arg| arg.get("instId"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let timeframe = timeframe_from_channel(channel)?;
        let data = payload
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        for item in data {
            if let Some(candle) = normalize_candle(item, arg_inst_id, &timeframe) {
                self.emit_candle(candle).await;
            }
        }

        Ok(())
    }

    pub(super) async fn handle_private_text_message(
        &self,
        mode: &str,
        generation: u64,
        text: &str,
    ) -> Result<bool> {
        self.handle_private_text_message_for_kind(
            PrivateMessageKind::Regular,
            mode,
            generation,
            text,
        )
        .await
    }

    pub(super) async fn handle_private_business_text_message(
        &self,
        mode: &str,
        generation: u64,
        text: &str,
    ) -> Result<bool> {
        self.handle_private_text_message_for_kind(
            PrivateMessageKind::Business,
            mode,
            generation,
            text,
        )
        .await
    }

    async fn handle_private_text_message_for_kind(
        &self,
        kind: PrivateMessageKind,
        mode: &str,
        generation: u64,
        text: &str,
    ) -> Result<bool> {
        let payload = match self
            .handle_private_ws_event_prefix(kind, mode, generation, text)
            .await?
        {
            PrivateWsPrefix::Subscribe => return Ok(true),
            PrivateWsPrefix::Ignore => return Ok(false),
            PrivateWsPrefix::Data(payload) => payload,
        };

        let channel = payload
            .get("arg")
            .and_then(|arg| arg.get("channel"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let data = payload
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        match (kind, channel) {
            (PrivateMessageKind::Regular, "account") => {
                for item in data {
                    self.emit_account(mode, item).await;
                }
            }
            (PrivateMessageKind::Regular, "orders") => {
                for item in data {
                    if let Some(order) = normalize_private_order(item, mode) {
                        self.emit_private_order(order).await;
                    }
                }
            }
            (PrivateMessageKind::Regular, "fills") => {
                for item in data {
                    if let Some(fill) = normalize_private_fill(item, mode) {
                        self.emit_private_fill(fill).await;
                    }
                }
            }
            (PrivateMessageKind::Regular, "positions") => {
                for item in data {
                    if let Some(position) = normalize_private_position(item, mode) {
                        self.emit_private_position(position).await;
                    }
                }
            }
            (PrivateMessageKind::Business, "orders-algo") => {
                for item in data {
                    if let Some(order) = normalize_private_algo_order(item, mode) {
                        self.emit_private_algo_order(order).await;
                    }
                }
            }
            _ => {}
        }

        Ok(false)
    }

    async fn handle_private_ws_event_prefix(
        &self,
        kind: PrivateMessageKind,
        mode: &str,
        generation: u64,
        text: &str,
    ) -> Result<PrivateWsPrefix> {
        let payload = serde_json::from_str::<Value>(text)?;
        self.mark_private_message_for_kind(kind, mode, generation)
            .await;

        if payload
            .get("event")
            .and_then(Value::as_str)
            .is_some_and(|event| event == "error")
        {
            if matches!(kind, PrivateMessageKind::Regular) && is_nonfatal_private_ws_error(&payload)
            {
                tracing::warn!(
                    mode,
                    payload = %payload,
                    "OKX private websocket returned non-fatal channel error"
                );
                return Ok(PrivateWsPrefix::Ignore);
            }
            let message = payload
                .get("msg")
                .and_then(Value::as_str)
                .unwrap_or(kind.error_message())
                .to_string();
            self.set_private_ws_last_error(kind, mode, generation, Some(message.clone()))
                .await;
            return Err(anyhow!(message));
        }

        if payload
            .get("event")
            .and_then(Value::as_str)
            .is_some_and(|event| event == "login")
        {
            let code = payload.get("code").and_then(Value::as_str).unwrap_or("0");
            if code != "0" {
                let message = payload
                    .get("msg")
                    .and_then(Value::as_str)
                    .unwrap_or(kind.login_error_message())
                    .to_string();
                self.set_private_ws_last_error(kind, mode, generation, Some(message.clone()))
                    .await;
                return Err(anyhow!(message));
            }
            self.mark_private_connected_for_kind(kind, mode, generation)
                .await;
            return Ok(PrivateWsPrefix::Subscribe);
        }

        if payload.get("event").is_some() {
            return Ok(PrivateWsPrefix::Ignore);
        }
        Ok(PrivateWsPrefix::Data(payload))
    }

    async fn mark_private_message_for_kind(
        &self,
        kind: PrivateMessageKind,
        mode: &str,
        generation: u64,
    ) {
        match kind {
            PrivateMessageKind::Regular => self.mark_private_message(mode, generation).await,
            PrivateMessageKind::Business => {
                self.mark_private_business_message(mode, generation).await
            }
        }
    }

    async fn mark_private_connected_for_kind(
        &self,
        kind: PrivateMessageKind,
        mode: &str,
        generation: u64,
    ) {
        match kind {
            PrivateMessageKind::Regular => self.mark_private_connected(mode, generation).await,
            PrivateMessageKind::Business => {
                self.mark_private_business_connected(mode, generation).await
            }
        }
    }

    async fn set_private_ws_last_error(
        &self,
        kind: PrivateMessageKind,
        mode: &str,
        generation: u64,
        error: Option<String>,
    ) {
        match kind {
            PrivateMessageKind::Regular => {
                self.set_private_last_error(mode, generation, error).await
            }
            PrivateMessageKind::Business => {
                self.set_private_business_last_error(mode, generation, error)
                    .await
            }
        }
    }
}

fn is_nonfatal_private_ws_error(payload: &Value) -> bool {
    let code = payload.get("code").and_then(Value::as_str).unwrap_or("");
    let message = payload
        .get("msg")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_lowercase();
    code == "64003" && message.contains("trading fee tier")
}
