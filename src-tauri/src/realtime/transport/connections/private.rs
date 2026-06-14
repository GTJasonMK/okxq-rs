use anyhow::{anyhow, Result};
use futures_util::{Sink, SinkExt, StreamExt};
use serde_json::json;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};

use crate::{config::ApiCredentials, realtime::RealtimeManager};

use super::super::super::{
    normalize::{private_business_ws_url, private_ws_url, sign_private_ws_login},
    WS_IDLE_TIMEOUT,
};

#[derive(Clone, Copy)]
enum PrivateWsKind {
    Regular,
    Business,
}

impl PrivateWsKind {
    fn url(self, mode: &str) -> Result<&'static str> {
        match self {
            Self::Regular => Ok(private_ws_url(mode)?),
            Self::Business => Ok(private_business_ws_url(mode)?),
        }
    }

    fn connect_mode(self, mode: &str) -> String {
        match self {
            Self::Regular => mode.to_string(),
            Self::Business => format!("business:{mode}"),
        }
    }

    fn closed_prefix(self) -> &'static str {
        match self {
            Self::Regular => "OKX private websocket closed",
            Self::Business => "OKX business private websocket closed",
        }
    }

    fn ended_message(self) -> &'static str {
        match self {
            Self::Regular => "OKX private websocket stream ended",
            Self::Business => "OKX business private websocket stream ended",
        }
    }
}

impl RealtimeManager {
    pub(in crate::realtime::transport) async fn run_private_connection(
        &self,
        mode: &str,
        generation: u64,
        credentials: ApiCredentials,
    ) -> Result<()> {
        self.run_private_ws_connection(PrivateWsKind::Regular, mode, generation, credentials)
            .await
    }

    pub(in crate::realtime::transport) async fn run_private_business_connection(
        &self,
        mode: &str,
        generation: u64,
        credentials: ApiCredentials,
    ) -> Result<()> {
        self.run_private_ws_connection(PrivateWsKind::Business, mode, generation, credentials)
            .await
    }

    async fn run_private_ws_connection(
        &self,
        kind: PrivateWsKind,
        mode: &str,
        generation: u64,
        credentials: ApiCredentials,
    ) -> Result<()> {
        let url = kind.url(mode)?;
        let connect_mode = kind.connect_mode(mode);
        let stream = self.governed_ws_connect(url, &connect_mode).await?;
        let (mut write, mut read) = stream.split();
        let timestamp = chrono::Utc::now().timestamp().to_string();
        let sign = sign_private_ws_login(&credentials.secret_key, &timestamp)?;

        self.governed_ws_send_text(
            &mut write,
            "ws.login",
            json!({
                "op": "login",
                "args": [{
                    "apiKey": credentials.api_key,
                    "passphrase": credentials.passphrase,
                    "timestamp": timestamp,
                    "sign": sign
                }]
            }),
            &connect_mode,
            None,
        )
        .await?;

        let mut subscribed = false;
        loop {
            if !self
                .should_continue_private_ws(kind, mode, generation)
                .await
            {
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
                        self.mark_private_ws_message(kind, mode, generation).await;
                        continue;
                    }
                    let Some(next_subscribed) = self
                        .handle_private_ws_payload(
                            kind,
                            &mut write,
                            mode,
                            generation,
                            &text,
                            subscribed,
                            &connect_mode,
                        )
                        .await?
                    else {
                        return Ok(());
                    };
                    subscribed = next_subscribed;
                }
                Ok(Some(Ok(Message::Binary(bytes)))) => {
                    if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                        let Some(next_subscribed) = self
                            .handle_private_ws_payload(
                                kind,
                                &mut write,
                                mode,
                                generation,
                                &text,
                                subscribed,
                                &connect_mode,
                            )
                            .await?
                        else {
                            return Ok(());
                        };
                        subscribed = next_subscribed;
                    }
                }
                Ok(Some(Ok(Message::Ping(payload)))) => {
                    write.send(Message::Pong(payload)).await?;
                    self.mark_private_ws_message(kind, mode, generation).await;
                }
                Ok(Some(Ok(Message::Pong(_)))) => {
                    self.mark_private_ws_message(kind, mode, generation).await;
                }
                Ok(Some(Ok(Message::Close(frame)))) => {
                    return Err(anyhow!("{}: {:?}", kind.closed_prefix(), frame));
                }
                Ok(Some(Ok(_))) => {}
                Ok(Some(Err(error))) => return Err(error.into()),
                Ok(None) => return Err(anyhow!(kind.ended_message())),
                Err(_) => {
                    write.send(Message::Text("ping".into())).await?;
                }
            }
        }
    }

    async fn handle_private_ws_payload<S>(
        &self,
        kind: PrivateWsKind,
        write: &mut S,
        mode: &str,
        generation: u64,
        text: &str,
        subscribed: bool,
        connect_mode: &str,
    ) -> Result<Option<bool>>
    where
        S: Sink<Message, Error = WsError> + Unpin,
    {
        let should_subscribe = match kind {
            PrivateWsKind::Regular => {
                self.handle_private_text_message(mode, generation, text)
                    .await?
            }
            PrivateWsKind::Business => {
                self.handle_private_business_text_message(mode, generation, text)
                    .await?
            }
        };
        if !should_subscribe || subscribed {
            return Ok(Some(subscribed));
        }

        let args = match kind {
            PrivateWsKind::Regular => self.active_private_args(mode).await,
            PrivateWsKind::Business => self.active_private_business_args(mode).await,
        };
        if args.is_empty() {
            return Ok(None);
        }
        self.governed_ws_send_text(
            write,
            "ws.subscribe",
            json!({"op": "subscribe", "args": args}),
            connect_mode,
            None,
        )
        .await?;
        Ok(Some(true))
    }

    async fn should_continue_private_ws(
        &self,
        kind: PrivateWsKind,
        mode: &str,
        generation: u64,
    ) -> bool {
        match kind {
            PrivateWsKind::Regular => self.should_continue_private(mode, generation).await,
            PrivateWsKind::Business => {
                self.should_continue_private_business(mode, generation)
                    .await
            }
        }
    }

    async fn mark_private_ws_message(&self, kind: PrivateWsKind, mode: &str, generation: u64) {
        match kind {
            PrivateWsKind::Regular => self.mark_private_message(mode, generation).await,
            PrivateWsKind::Business => self.mark_private_business_message(mode, generation).await,
        }
    }
}
