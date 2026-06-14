use std::time::{Duration, Instant};

use anyhow::Result;
use futures_util::{Sink, SinkExt};
use serde_json::Value;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};

use crate::{
    okx_network::{connect_okx_websocket, effective_proxy_label, OkxWsStream},
    okx_outbound::OKXOutboundEvent,
};

use super::super::RealtimeManager;

impl RealtimeManager {
    pub(super) async fn governed_ws_connect(&self, url: &str, mode: &str) -> Result<OkxWsStream> {
        let op_key = "ws.connect";
        let start = Instant::now();
        let _permit = self
            .outbound_governor
            .acquire_request_permit(op_key, None)
            .await;
        self.outbound_governor.acquire(op_key, None).await;

        let proxy_url = self.proxy_url().await;
        let proxy_label = effective_proxy_label(&proxy_url);
        tracing::debug!(
            op_key,
            mode,
            url,
            proxy = proxy_label.as_str(),
            "connecting OKX websocket"
        );
        match connect_okx_websocket(url, &proxy_url).await {
            Ok((stream, _response)) => {
                self.record_ws_outbound(op_key, mode, None, "ok", start.elapsed());
                Ok(stream)
            }
            Err(error) => {
                self.record_ws_outbound(op_key, mode, None, "network_error", start.elapsed());
                tracing::warn!(
                    op_key,
                    mode,
                    url,
                    proxy = proxy_label.as_str(),
                    error = %error,
                    "OKX websocket connect failed"
                );
                Err(error)
            }
        }
    }

    pub(super) async fn governed_ws_send_text<S>(
        &self,
        write: &mut S,
        op_key: &str,
        payload: Value,
        mode: &str,
        inst_id: Option<&str>,
    ) -> Result<()>
    where
        S: Sink<Message, Error = WsError> + Unpin,
    {
        let start = Instant::now();
        let _permit = self
            .outbound_governor
            .acquire_request_permit(op_key, inst_id)
            .await;
        self.outbound_governor.acquire(op_key, inst_id).await;
        let payload_text = payload.to_string();

        tracing::debug!(
            op_key,
            mode,
            inst_id = inst_id.unwrap_or(""),
            "sending OKX websocket control message"
        );
        match write.send(Message::Text(payload_text.into())).await {
            Ok(()) => {
                self.record_ws_outbound(op_key, mode, inst_id, "ok", start.elapsed());
                Ok(())
            }
            Err(error) => {
                self.record_ws_outbound(op_key, mode, inst_id, "network_error", start.elapsed());
                tracing::warn!(
                    op_key,
                    mode,
                    inst_id = inst_id.unwrap_or(""),
                    error = %error,
                    "OKX websocket control message failed"
                );
                Err(error.into())
            }
        }
    }

    fn record_ws_outbound(
        &self,
        op_key: &str,
        mode: &str,
        inst_id: Option<&str>,
        result: &str,
        elapsed: Duration,
    ) {
        let rule = self.rate_rules.get(op_key);
        let now_ts = chrono::Utc::now().timestamp_millis() as f64 / 1000.0;
        let rule_key = rule
            .map(|item| item.rule_key.as_str())
            .unwrap_or("ws_unknown");
        let target_group = rule
            .map(|item| item.target_group.as_str())
            .unwrap_or("ws_control");
        let channel = rule.map(|item| item.channel.as_str()).unwrap_or("ws");
        let scope_key = match rule_key {
            "ws_conn_ops" => format!("ws_conn:{mode}"),
            "ws_connect_ip" => "ws_connect_ip".to_string(),
            _ => rule_key.to_string(),
        };

        self.outbound_timeline.record(OKXOutboundEvent {
            ts: now_ts,
            op_key: op_key.to_string(),
            channel: channel.to_string(),
            target_group: target_group.to_string(),
            rule_key: rule_key.to_string(),
            scope_key,
            inst_id: inst_id.unwrap_or("").to_string(),
            mode: mode.to_string(),
            result: result.to_string(),
            latency_ms: elapsed.as_millis() as i64,
        });
    }
}
