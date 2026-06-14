use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;

use crate::{okx_network::OKX_PUBLIC_WS_URL, realtime::RealtimeManager};

use super::super::super::WS_IDLE_TIMEOUT;

impl RealtimeManager {
    pub(in crate::realtime::transport) async fn run_connection(
        &self,
        generation: u64,
        args: Vec<Value>,
    ) -> Result<()> {
        let stream = self
            .governed_ws_connect(OKX_PUBLIC_WS_URL, "public")
            .await?;
        let (mut write, mut read) = stream.split();

        self.governed_ws_send_text(
            &mut write,
            "ws.subscribe",
            json!({"op": "subscribe", "args": args}),
            "public",
            None,
        )
        .await?;

        self.mark_connected(generation).await;

        loop {
            if !self.should_continue(generation).await {
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
                        self.mark_message(generation).await;
                        continue;
                    }
                    self.handle_text_message(generation, &text).await?;
                }
                Ok(Some(Ok(Message::Binary(bytes)))) => {
                    if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                        self.handle_text_message(generation, &text).await?;
                    }
                }
                Ok(Some(Ok(Message::Ping(payload)))) => {
                    write.send(Message::Pong(payload)).await?;
                    self.mark_message(generation).await;
                }
                Ok(Some(Ok(Message::Pong(_)))) => {
                    self.mark_message(generation).await;
                }
                Ok(Some(Ok(Message::Close(frame)))) => {
                    return Err(anyhow!("OKX websocket closed: {:?}", frame));
                }
                Ok(Some(Ok(_))) => {}
                Ok(Some(Err(error))) => return Err(error.into()),
                Ok(None) => return Err(anyhow!("OKX websocket stream ended")),
                Err(_) => {
                    write.send(Message::Text("ping".into())).await?;
                }
            }
        }
    }
}
