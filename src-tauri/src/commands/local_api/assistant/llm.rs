use std::time::Duration;

use futures_util::StreamExt;
use tauri::Emitter;

use super::*;

pub(super) const ASSISTANT_DELTA_EVENT: &str = "okxq-assistant-delta";

pub(super) async fn stream_llm_chat(
    state: &AppState,
    messages: &[Value],
    tools: &Value,
) -> AppResult<(String, Vec<Value>)> {
    let cfg = state.config.read().await.assistant.clone();
    if !cfg.enabled {
        return Err(AppError::Validation("AI 助手当前未启用。".to_string()));
    }
    if !cfg.is_configured() {
        return Err(AppError::Validation(
            "AI 助手未完成配置，请先在设置页填写 AI Key 和模型参数。".to_string(),
        ));
    }
    let endpoint = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));
    let payload = json!({
        "model": cfg.model,
        "messages": messages,
        "tools": tools,
        "tool_choice": "auto",
        "stream": true,
        "temperature": 0.2
    });
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|error| AppError::Runtime(error.to_string()))?;
    let response = client
        .post(&endpoint)
        .bearer_auth(&cfg.api_key)
        .json(&payload)
        .send()
        .await
        .map_err(|error| AppError::Runtime(format!("AI 上游请求失败: {error}")))?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Runtime(format!("AI 上游 HTTP {status}: {body}")));
    }

    let mut stream = response.bytes_stream();
    let mut accumulated_content = String::new();
    let mut tool_call_buffer: std::collections::BTreeMap<usize, (String, String, String)> =
        std::collections::BTreeMap::new();
    while let Some(chunk_result) = stream.next().await {
        let chunk =
            chunk_result.map_err(|error| AppError::Runtime(format!("流读取失败: {error}")))?;
        let text = String::from_utf8_lossy(&chunk);
        for line in text.lines() {
            let line = line.trim();
            if !line.starts_with("data: ") {
                continue;
            }
            let data = line[6..].trim();
            if data.is_empty() || data == "[DONE]" {
                continue;
            }
            let Ok(event) = serde_json::from_str::<Value>(data) else {
                continue;
            };
            for choice in event
                .get("choices")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let delta = choice.get("delta").or_else(|| choice.get("message"));
                if let Some(delta) = delta {
                    if let Some(content) = delta.get("content").and_then(Value::as_str) {
                        if !content.is_empty() {
                            accumulated_content.push_str(content);
                            let _ = state.app_handle.emit(
                                ASSISTANT_DELTA_EVENT,
                                json!({"type": "text", "content": content}),
                            );
                        }
                    }
                    if let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
                        for tc in tool_calls {
                            let idx = tc.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                            let entry = tool_call_buffer.entry(idx).or_default();
                            if let Some(id) = tc.get("id").and_then(Value::as_str) {
                                entry.0 = id.to_string();
                            }
                            if let Some(func) = tc.get("function") {
                                if let Some(name) = func.get("name").and_then(Value::as_str) {
                                    entry.1 = name.to_string();
                                }
                                if let Some(args) = func.get("arguments").and_then(Value::as_str) {
                                    entry.2.push_str(args);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let mut finalized_tool_calls: Vec<Value> = Vec::new();
    for (_idx, (id, name, args)) in tool_call_buffer {
        if !name.is_empty() {
            let arguments: Value = serde_json::from_str(&args).unwrap_or_else(|_| json!({}));
            let _ = state.app_handle.emit(
                ASSISTANT_DELTA_EVENT,
                json!({"type": "tool_call", "name": name, "arguments": arguments}),
            );
            finalized_tool_calls.push(json!({
                "id": id,
                "type": "function",
                "function": {
                    "name": name,
                    "arguments": args
                }
            }));
        }
    }

    Ok((accumulated_content, finalized_tool_calls))
}

pub(super) async fn complete_llm_chat(
    cfg: &crate::config::AssistantConfig,
    messages: &[Value],
    tools: &Value,
) -> AppResult<Value> {
    let endpoint = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));
    let payload = json!({
        "model": cfg.model,
        "messages": messages,
        "tools": tools,
        "tool_choice": tools.as_array().map(|a| if a.is_empty() { "none" } else { "auto" }).unwrap_or("none"),
        "stream": false,
        "temperature": 0.2
    });
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|error| AppError::Runtime(error.to_string()))?;
    let response = client
        .post(&endpoint)
        .bearer_auth(&cfg.api_key)
        .json(&payload)
        .send()
        .await
        .map_err(|error| AppError::Runtime(error.to_string()))?;
    let status = response.status();
    let body = response
        .json::<Value>()
        .await
        .map_err(|error| AppError::Runtime(format!("AI 响应解析失败: {error}")))?;
    if !status.is_success() {
        return Err(AppError::Runtime(format!(
            "AI 上游 HTTP {status}: {}",
            body.get("error")
                .and_then(|item| item.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("unknown error")
        )));
    }
    Ok(body)
}
