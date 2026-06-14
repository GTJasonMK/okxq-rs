use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;

use crate::{okx_network::OKX_BUSINESS_WS_URL, realtime::RealtimeManager};

use super::super::super::{normalize::candle_channel, WS_IDLE_TIMEOUT};

impl RealtimeManager {
    pub(in crate::realtime::transport) async fn run_candle_connection(
        &self,
        generation: u64,
        subscriptions: Vec<(String, String)>,
    ) -> Result<()> {
        let stream = self
            .governed_ws_connect(OKX_BUSINESS_WS_URL, "business")
            .await?;
        let (mut write, mut read) = stream.split();

        let args = subscriptions
            .iter()
            .map(|(inst_id, timeframe)| {
                json!({"channel": candle_channel(timeframe), "instId": inst_id})
            })
            .collect::<Vec<_>>();
        self.governed_ws_send_text(
            &mut write,
            "ws.subscribe",
            json!({"op": "subscribe", "args": args}),
            "business",
            None,
        )
        .await?;

        self.mark_candles_connected(generation).await;

        loop {
            if !self.should_continue_candles(generation).await {
                return Ok(());
            }

            match timeout(WS_IDLE_TIMEOUT, read.next()).await {
                Ok(Some(Ok(Message::Text(text)))) => {
                    let text = text.to_string();
                    if text.trim() == "ping" {
                        write.send(Message::Text("pong".into())).await?;
                        continue;
                    }
                    if text.trim() == "pong" {
                        self.mark_candle_message(generation).await;
                        continue;
                    }
                    self.handle_candle_text_message(generation, &text).await?;
                }
                Ok(Some(Ok(Message::Binary(bytes)))) => {
                    if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                        self.handle_candle_text_message(generation, &text).await?;
                    }
                }
                Ok(Some(Ok(Message::Ping(payload)))) => {
                    write.send(Message::Pong(payload)).await?;
                    self.mark_candle_message(generation).await;
                }
                Ok(Some(Ok(Message::Pong(_)))) => {
                    self.mark_candle_message(generation).await;
                }
                Ok(Some(Ok(Message::Close(frame)))) => {
                    return Err(anyhow!("OKX business websocket closed: {:?}", frame));
                }
                Ok(Some(Ok(_))) => {}
                Ok(Some(Err(error))) => return Err(error.into()),
                Ok(None) => return Err(anyhow!("OKX business websocket stream ended")),
                Err(_) => {
                    write.send(Message::Text("ping".into())).await?;
                }
            }
        }
    }
}
