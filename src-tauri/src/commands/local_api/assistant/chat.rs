use tauri::Emitter;

use super::llm::{complete_llm_chat, stream_llm_chat, ASSISTANT_DELTA_EVENT};
use super::tools::{build_system_prompt, build_tools, execute_tool};
use super::*;

async fn assistant_chat_loop(
    state: &AppState,
    user_message: &str,
    market_context: Value,
) -> AppResult<String> {
    let cfg = state.config.read().await.assistant.clone();
    let system_content = build_system_prompt(&market_context);
    let mut messages: Vec<Value> = vec![
        json!({"role": "system", "content": system_content}),
        json!({"role": "user", "content": user_message}),
    ];
    let tools = build_tools();
    let max_rounds = 5;
    let mut final_text = String::new();

    for round in 0..max_rounds {
        let stream_result = stream_llm_chat(state, &messages, &tools).await;
        let (text_delta, tool_calls) = match stream_result {
            Ok(result) => result,
            Err(_stream_err) => {
                let body = complete_llm_chat(&cfg, &messages, &tools).await?;
                let choice = body
                    .get("choices")
                    .and_then(Value::as_array)
                    .and_then(|arr| arr.first());
                let message = choice.and_then(|c| c.get("message"));
                let text = message
                    .and_then(|m| m.get("content"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if !text.is_empty() {
                    let _ = state.app_handle.emit(
                        ASSISTANT_DELTA_EVENT,
                        json!({"type": "text", "content": &text}),
                    );
                }
                let tcs: Vec<Value> = message
                    .and_then(|m| m.get("tool_calls"))
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                (text, tcs)
            }
        };

        if !text_delta.is_empty() {
            final_text = text_delta;
        }
        if tool_calls.is_empty() {
            break;
        }

        let mut tool_results: Vec<Value> = Vec::new();
        for tc in &tool_calls {
            let tc_id = tc.get("id").and_then(Value::as_str).unwrap_or("");
            let func = tc.get("function");
            let name = func
                .and_then(|f| f.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("");
            let args_str = func
                .and_then(|f| f.get("arguments"))
                .and_then(Value::as_str)
                .unwrap_or("{}");
            let args: Value = serde_json::from_str(args_str).unwrap_or_else(|_| json!({}));
            let tool_result = match execute_tool(state, name, &args).await {
                Ok(result) => result,
                Err(error) => json!({"error": error.to_string()}).to_string(),
            };
            tool_results.push(json!({
                "role": "tool",
                "tool_call_id": tc_id,
                "content": tool_result
            }));
        }

        messages.push(json!({
            "role": "assistant",
            "content": if final_text.is_empty() { Value::Null } else { json!(final_text) },
            "tool_calls": tool_calls
        }));
        messages.extend(tool_results);

        if round == max_rounds - 1 {
            let body = complete_llm_chat(&cfg, &messages, &json!([])).await?;
            let final_answer = body
                .get("choices")
                .and_then(Value::as_array)
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("message"))
                .and_then(|m| m.get("content"))
                .and_then(Value::as_str)
                .unwrap_or("");
            if !final_answer.is_empty() {
                final_text = final_answer.to_string();
                let _ = state.app_handle.emit(
                    ASSISTANT_DELTA_EVENT,
                    json!({"type": "text", "content": final_answer}),
                );
            }
            break;
        }
    }

    if final_text.is_empty() {
        final_text = "分析完成，未产生文本输出。请查看工具调用结果。".to_string();
    }
    let _ = state
        .app_handle
        .emit(ASSISTANT_DELTA_EVENT, json!({"type": "done"}));
    Ok(final_text)
}

pub(crate) async fn assistant_agent_chat(
    state: &AppState,
    req: &LocalApiRequest,
) -> AppResult<Value> {
    let message = request_string(req, "message", "");
    if message.trim().is_empty() {
        return Err(AppError::Validation("message is required".to_string()));
    }
    let inst_id = request_string(req, "inst_id", "");
    let inst_type = request_string(req, "inst_type", "SPOT");
    let mode = request_trading_mode(state, req).await?;
    let market_context = req
        .body
        .get("market_context")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let mut session_id = request_string(req, "session_id", "");
    if session_id.trim().is_empty() || fetch_assistant_session(state, &session_id).await?.is_none()
    {
        let title = request_string(req, "title", "").trim().to_string();
        let title = if title.is_empty() {
            message.chars().take(24).collect::<String>()
        } else {
            title
        };
        let session = create_assistant_session_record(
            state,
            &title,
            &inst_id,
            &inst_type,
            &mode,
            json!({"market_context": market_context.clone()}),
        )
        .await?;
        session_id = value_string_at(&session, "session_id", "");
    }
    append_assistant_message(
        state,
        &session_id,
        "user",
        &message,
        json!({"market_context": market_context.clone()}),
    )
    .await?;
    let answer = assistant_chat_loop(state, &message, market_context).await?;
    let assistant_message =
        append_assistant_message(state, &session_id, "assistant", &answer, json!({})).await?;
    let detail = assistant_session_detail_value(state, &session_id).await?;
    Ok(code_ok(json!({
        "streaming": true,
        "session_id": session_id,
        "assistant_message": assistant_message,
        "tool_steps": [],
        "session": detail.get("session").cloned().unwrap_or_else(|| json!({})),
        "detail": detail
    })))
}
