use std::{
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    thread::{self, JoinHandle},
};

use serde_json::{json, Value};

use crate::{
    error::{AppError, AppResult},
    trading_semantics::{
        normalize_runtime_order_type_text, LIVE_ALGO_TARGET_ORDER_TYPES,
        RUNTIME_ACTIONS_V1_ENGINE_CONTROLLED_FIELDS, RUNTIME_ACTIONS_V1_JSON_BOOL_FIELDS,
        RUNTIME_ACTIONS_V1_JSON_NUMBER_FIELDS, RUNTIME_ACTIONS_V1_JSON_TEXT_FIELDS,
        RUNTIME_ACTIONS_V1_REMOVED_ACTION_ALIAS_FIELDS, RUNTIME_ACTIONS_V1_REMOVED_FIELD_ALIASES,
        RUNTIME_ACTIONS_V1_SUPPORTED_ACTIONS, RUNTIME_ACTIONS_V1_TARGET_ORDER_KINDS,
    },
};

use super::{
    files::{normalize_strategy_file_name, strategy_file_path},
    types::{
        RuntimeStrategyAction, RuntimeStrategyDecision, RuntimeStrategyExecutionLog,
        RuntimeStrategyMeta,
    },
};

const RUNTIME_ACTIONS_V1_REMOVED_TOP_LEVEL_FIELDS: &[&str] =
    &["orders", "risk_orders", "signals", "portfolio_layers"];

fn find_runner_script(project_root: &Path) -> String {
    project_root
        .join("src-tauri/python/strategy_runner.py")
        .display()
        .to_string()
}

fn find_python_executable(runner_path: &str) -> String {
    let Some(project_root) = project_root_from_runner_path(runner_path) else {
        return "python3".to_string();
    };
    let venv_python = project_root.join(".venv/bin/python");
    if venv_python.is_file() {
        return venv_python.display().to_string();
    }
    "python3".to_string()
}

fn project_root_from_runner_path(runner_path: &str) -> Option<PathBuf> {
    let path = Path::new(runner_path);
    let python_dir = path.parent()?;
    let src_tauri_dir = python_dir.parent()?;
    src_tauri_dir.parent().map(Path::to_path_buf)
}

fn existing_strategy_file_path(project_root: &Path, file_name: &str) -> AppResult<PathBuf> {
    let file_path = strategy_file_path(project_root, file_name)?;
    if !file_path.exists() {
        return Err(AppError::Validation(format!(
            "运行策略文件不存在: {}",
            file_path.display()
        )));
    }
    Ok(file_path)
}

fn runtime_compute_command(
    file_path: &Path,
    config: &Value,
    candles: &[Value],
    context: Option<&Value>,
    context_ref: Option<&str>,
    progress_events: bool,
    strategy_log_events: bool,
) -> Value {
    let mut command = json!({
        "action": "compute",
        "file_path": file_path.display().to_string(),
        "config": config,
        "candles": candles
    });
    if let Some(context) = context {
        command["context"] = context.clone();
    }
    if let Some(context_ref) = context_ref.filter(|value| !value.trim().is_empty()) {
        command["context_ref"] = json!(context_ref);
    }
    if progress_events {
        command["progress_events"] = json!(true);
    }
    if strategy_log_events {
        command["strategy_log_events"] = json!(true);
    }
    command
}

/// 调用 Python sidecar 发送一个命令并读取响应。
pub fn call_python_runner(runner_path: &str, command: Value) -> AppResult<Value> {
    call_python_runner_with_progress(runner_path, command, |_| {})
}

/// 调用 Python sidecar 并解析 stdout 中的进度事件。
///
/// stdout 协议允许多行 JSON：`{"event":"progress", ...}` 表示运行进度，
/// 最后一条非 progress JSON 是命令结果。stderr 仍只用于日志。
pub fn call_python_runner_with_progress<F>(
    runner_path: &str,
    command: Value,
    mut on_progress: F,
) -> AppResult<Value>
where
    F: FnMut(&Value),
{
    call_python_runner_with_events(runner_path, command, |event| {
        if event.get("event").and_then(Value::as_str) == Some("progress") {
            on_progress(event);
        }
    })
}

pub fn call_python_runner_with_events<F>(
    runner_path: &str,
    command: Value,
    mut on_event: F,
) -> AppResult<Value>
where
    F: FnMut(&Value),
{
    let python_executable = find_python_executable(runner_path);
    let mut child = Command::new(&python_executable)
        .arg(runner_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| AppError::Runtime(format!("无法启动 Python 策略执行器: {error}")))?;

    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| AppError::Runtime("无法打开 Python 进程 stdin".to_string()))?;
        let command_line = serde_json::to_string(&command)? + "\n";
        stdin
            .write_all(command_line.as_bytes())
            .map_err(|error| AppError::Runtime(format!("写入 Python 进程失败: {error}")))?;
        stdin
            .flush()
            .map_err(|error| AppError::Runtime(format!("刷新 Python 进程 stdin 失败: {error}")))?;
    }

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Runtime("无法打开 Python 进程 stdout".to_string()))?;
    let stderr = child.stderr.take();
    let stderr_handle = thread::spawn(move || {
        let mut text = String::new();
        if let Some(mut stderr) = stderr {
            let _ = stderr.read_to_string(&mut text);
        }
        text
    });

    let mut stdout_text = String::new();
    let mut final_result = None;
    for line in BufReader::new(stdout).lines() {
        let line = line.map_err(|error| {
            AppError::Runtime(format!("读取 Python 策略执行器 stdout 失败: {error}"))
        })?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        stdout_text.push_str(trimmed);
        stdout_text.push('\n');
        let value: Value = serde_json::from_str(trimmed).map_err(|error| {
            AppError::Runtime(format!(
                "解析 Python 策略执行器输出失败: {error}。原始输出: {trimmed}"
            ))
        })?;
        if is_python_runner_event(&value) {
            on_event(&value);
        } else {
            final_result = Some(value);
        }
    }

    child
        .wait()
        .map_err(|error| AppError::Runtime(format!("等待 Python 进程结束失败: {error}")))?;

    let stderr_text = stderr_handle.join().unwrap_or_default();
    if !stderr_text.trim().is_empty() {
        tracing::warn!("Python 策略执行器 stderr: {}", stderr_text.trim());
    }

    let Some(result) = final_result else {
        return Err(AppError::Runtime(format!(
            "Python 策略执行器无最终输出。stdout: {} stderr: {}",
            stdout_text.trim(),
            stderr_text.trim()
        )));
    };

    ensure_python_result_ok(result)
}

fn is_python_runner_event(value: &Value) -> bool {
    matches!(
        value.get("event").and_then(Value::as_str),
        Some("progress" | "strategy_log")
    )
}

pub(crate) struct PythonRunnerSession {
    project_root: PathBuf,
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    stderr_handle: Option<JoinHandle<String>>,
}

impl PythonRunnerSession {
    pub(crate) fn new(project_root: &Path) -> AppResult<Self> {
        let runner_path = find_runner_script(project_root);
        let python_executable = find_python_executable(&runner_path);
        let mut child = Command::new(&python_executable)
            .arg(&runner_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| AppError::Runtime(format!("无法启动 Python 策略执行器: {error}")))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AppError::Runtime("无法打开 Python 进程 stdin".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AppError::Runtime("无法打开 Python 进程 stdout".to_string()))?;
        let stderr = child.stderr.take();
        let stderr_handle = thread::spawn(move || {
            let mut text = String::new();
            if let Some(mut stderr) = stderr {
                let _ = stderr.read_to_string(&mut text);
            }
            text
        });

        Ok(Self {
            project_root: project_root.to_path_buf(),
            child,
            stdin,
            stdout: BufReader::new(stdout),
            stderr_handle: Some(stderr_handle),
        })
    }

    pub(crate) fn compute_runtime_decision_with_context_and_events<F>(
        &mut self,
        file_name: &str,
        config: &Value,
        candles: &[Value],
        context: &Value,
        mut on_event: F,
    ) -> AppResult<RuntimeStrategyDecision>
    where
        F: FnMut(&Value),
    {
        let file_path = existing_strategy_file_path(&self.project_root, file_name)?;
        let command =
            runtime_compute_command(&file_path, config, candles, Some(context), None, true, true);
        let result = self.call_python_runner_with_events(command, |event| on_event(event))?;
        runtime_decision_from_result(&result)
    }

    pub(crate) fn cache_runtime_context(
        &mut self,
        context_id: &str,
        context: &Value,
    ) -> AppResult<Value> {
        let command = json!({
            "action": "cache_context",
            "context_id": context_id,
            "context": context,
        });
        self.call_python_runner_with_events(command, |_| {})
    }

    pub(crate) fn compute_runtime_decision_with_context_ref_and_events<F>(
        &mut self,
        file_name: &str,
        config: &Value,
        candles: &[Value],
        context_ref: &str,
        context: &Value,
        mut on_event: F,
    ) -> AppResult<RuntimeStrategyDecision>
    where
        F: FnMut(&Value),
    {
        let file_path = existing_strategy_file_path(&self.project_root, file_name)?;
        let command = runtime_compute_command(
            &file_path,
            config,
            candles,
            Some(context),
            Some(context_ref),
            true,
            true,
        );
        let result = self.call_python_runner_with_events(command, |event| on_event(event))?;
        runtime_decision_from_result(&result)
    }

    fn call_python_runner_with_events<F>(
        &mut self,
        command: Value,
        mut on_event: F,
    ) -> AppResult<Value>
    where
        F: FnMut(&Value),
    {
        let command_line = serde_json::to_string(&command)? + "\n";
        self.stdin
            .write_all(command_line.as_bytes())
            .map_err(|error| AppError::Runtime(format!("写入 Python 进程失败: {error}")))?;
        self.stdin
            .flush()
            .map_err(|error| AppError::Runtime(format!("刷新 Python 进程 stdin 失败: {error}")))?;

        let mut stdout_text = String::new();
        loop {
            let mut line = String::new();
            let bytes_read = self.stdout.read_line(&mut line).map_err(|error| {
                AppError::Runtime(format!("读取 Python 策略执行器 stdout 失败: {error}"))
            })?;
            if bytes_read == 0 {
                return Err(AppError::Runtime(format!(
                    "Python 策略执行器提前退出。stdout: {}",
                    stdout_text.trim()
                )));
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            stdout_text.push_str(trimmed);
            stdout_text.push('\n');
            let value: Value = serde_json::from_str(trimmed).map_err(|error| {
                AppError::Runtime(format!(
                    "解析 Python 策略执行器输出失败: {error}。原始输出: {trimmed}"
                ))
            })?;
            if is_python_runner_event(&value) {
                on_event(&value);
                continue;
            }
            return ensure_python_result_ok(value);
        }
    }
}

impl Drop for PythonRunnerSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(handle) = self.stderr_handle.take() {
            if let Ok(stderr_text) = handle.join() {
                if !stderr_text.trim().is_empty() {
                    tracing::warn!("Python 策略执行器 stderr: {}", stderr_text.trim());
                }
            }
        }
    }
}

/// 发现运行策略元数据：从 .py 文件提取运行契约。
pub fn discover_runtime_strategy(
    project_root: &Path,
    file_name: &str,
) -> AppResult<RuntimeStrategyMeta> {
    let runner_path = find_runner_script(project_root);
    let normalized_file_name = normalize_strategy_file_name(file_name)?;
    let file_path = existing_strategy_file_path(project_root, &normalized_file_name)?;

    let command = json!({
        "action": "discover",
        "file_path": file_path.display().to_string()
    });

    let result = call_python_runner(&runner_path, command)?;

    Ok(RuntimeStrategyMeta {
        strategy_id: result["strategy_id"].as_str().unwrap_or("").to_string(),
        strategy_name: result["strategy_name"].as_str().unwrap_or("").to_string(),
        description: result["description"].as_str().unwrap_or("").to_string(),
        strategy_type: result["strategy_type"]
            .as_str()
            .unwrap_or("single_symbol_strategy")
            .to_string(),
        data_requirements: result
            .get("data_requirements")
            .cloned()
            .unwrap_or_else(|| json!({})),
        runtime_config: result
            .get("runtime_config")
            .cloned()
            .unwrap_or_else(|| json!({})),
        visualization: result
            .get("visualization")
            .cloned()
            .unwrap_or_else(|| json!({})),
        decision_contract: result
            .get("decision_contract")
            .cloned()
            .unwrap_or_else(|| json!({})),
        file_name: normalized_file_name,
    })
}

/// 执行运行策略计算：返回 actions 决策结果。
#[cfg(test)]
pub fn compute_runtime_decision(
    project_root: &Path,
    file_name: &str,
    config: &Value,
    candles: &[Value],
) -> AppResult<RuntimeStrategyDecision> {
    compute_runtime_decision_inner(project_root, file_name, config, candles, None, None)
}

#[cfg(test)]
pub fn compute_runtime_decision_with_context(
    project_root: &Path,
    file_name: &str,
    config: &Value,
    candles: &[Value],
    context: &Value,
) -> AppResult<RuntimeStrategyDecision> {
    compute_runtime_decision_inner(
        project_root,
        file_name,
        config,
        candles,
        Some(context),
        None,
    )
}

#[cfg(test)]
pub fn compute_runtime_decision_with_context_and_progress<F>(
    project_root: &Path,
    file_name: &str,
    config: &Value,
    candles: &[Value],
    context: &Value,
    mut on_progress: F,
) -> AppResult<RuntimeStrategyDecision>
where
    F: FnMut(&Value),
{
    compute_runtime_decision_inner(
        project_root,
        file_name,
        config,
        candles,
        Some(context),
        Some(&mut on_progress),
    )
}

pub fn compute_runtime_decision_with_context_and_events<F>(
    project_root: &Path,
    file_name: &str,
    config: &Value,
    candles: &[Value],
    context: &Value,
    mut on_event: F,
) -> AppResult<RuntimeStrategyDecision>
where
    F: FnMut(&Value),
{
    let runner_path = find_runner_script(project_root);
    let file_path = existing_strategy_file_path(project_root, file_name)?;
    let command =
        runtime_compute_command(&file_path, config, candles, Some(context), None, true, true);
    let result = call_python_runner_with_events(&runner_path, command, |event| on_event(event))?;
    runtime_decision_from_result(&result)
}

#[cfg(test)]
fn compute_runtime_decision_inner(
    project_root: &Path,
    file_name: &str,
    config: &Value,
    candles: &[Value],
    context: Option<&Value>,
    progress_callback: Option<&mut dyn FnMut(&Value)>,
) -> AppResult<RuntimeStrategyDecision> {
    let runner_path = find_runner_script(project_root);
    let file_path = existing_strategy_file_path(project_root, file_name)?;
    let command = runtime_compute_command(
        &file_path,
        config,
        candles,
        context,
        None,
        progress_callback.is_some(),
        false,
    );

    let result = if let Some(callback) = progress_callback {
        call_python_runner_with_progress(&runner_path, command, |event| callback(event))?
    } else {
        call_python_runner(&runner_path, command)?
    };

    runtime_decision_from_result(&result)
}

fn runtime_decision_from_result(result: &Value) -> AppResult<RuntimeStrategyDecision> {
    reject_removed_runtime_decision_top_level_fields(result)?;
    let actions = parse_runtime_actions(required_array_field(result, "actions")?)?;
    let execution_logs =
        parse_runtime_execution_logs(required_array_field(result, "execution_logs")?)?;
    let indicators = optional_object_field(result, "indicators")?;
    let diagnostics = required_object_value(result, "diagnostics")?;
    Ok(RuntimeStrategyDecision {
        actions,
        execution_logs,
        indicators,
        diagnostics,
    })
}

fn reject_removed_runtime_decision_top_level_fields(result: &Value) -> AppResult<()> {
    let removed = RUNTIME_ACTIONS_V1_REMOVED_TOP_LEVEL_FIELDS
        .iter()
        .filter(|field| result.get(**field).is_some())
        .copied()
        .collect::<Vec<_>>();
    if removed.is_empty() {
        return Ok(());
    }
    Err(AppError::Runtime(format!(
        "Python 策略执行器返回结果包含旧顶层字段 {}，actions_v1 只允许通过 actions/diagnostics/execution_logs 输出交易决策",
        removed.join("/")
    )))
}

fn ensure_python_result_ok(result: Value) -> AppResult<Value> {
    if result.get("ok").and_then(Value::as_bool) != Some(true) {
        let error_msg = result
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("未知错误");
        return Err(AppError::Runtime(format!(
            "Python 策略执行错误: {error_msg}"
        )));
    }
    Ok(result)
}

/// 执行运行策略最新决策诊断：返回 actions、诊断证据和策略内部日志。
#[cfg(test)]
pub fn compute_runtime_diagnostics(
    project_root: &Path,
    file_name: &str,
    config: &Value,
    candles: &[Value],
) -> AppResult<Value> {
    let decision =
        compute_runtime_diagnostics_decision_inner(project_root, file_name, config, candles, None)?;
    Ok(runtime_diagnostics_response_from_decision(&decision))
}

pub fn compute_runtime_diagnostics_decision_with_context(
    project_root: &Path,
    file_name: &str,
    config: &Value,
    candles: &[Value],
    context: &Value,
) -> AppResult<RuntimeStrategyDecision> {
    compute_runtime_diagnostics_decision_inner(
        project_root,
        file_name,
        config,
        candles,
        Some(context),
    )
}

fn compute_runtime_diagnostics_decision_inner(
    project_root: &Path,
    file_name: &str,
    config: &Value,
    candles: &[Value],
    context: Option<&Value>,
) -> AppResult<RuntimeStrategyDecision> {
    let runner_path = find_runner_script(project_root);
    let file_path = existing_strategy_file_path(project_root, file_name)?;

    let mut command = json!({
        "action": "diagnose",
        "file_path": file_path.display().to_string(),
        "config": config,
        "candles": candles
    });
    if let Some(context) = context {
        command["context"] = context.clone();
    }

    let result = call_python_runner(&runner_path, command)?;
    runtime_decision_from_result(&result)
}

pub fn runtime_diagnostics_response_from_decision(decision: &RuntimeStrategyDecision) -> Value {
    let mut diagnostics = decision.diagnostics.clone();
    let action_summary = runtime_action_summary(&decision.actions);
    let action_values = runtime_action_values(&decision.actions);
    let selected_symbols_value = json!(runtime_selected_symbols(&decision.actions));
    let execution_logs_value = serde_json::to_value(&decision.execution_logs)
        .expect("runtime execution logs should serialize");
    let object = diagnostics
        .as_object_mut()
        .expect("runtime decision diagnostics should be a JSON object");
    object.insert("actions".to_string(), Value::Array(action_values.clone()));
    object.insert("execution_logs".to_string(), execution_logs_value.clone());
    object.insert("action_summary".to_string(), action_summary.clone());
    object.insert(
        "selected_symbols".to_string(),
        selected_symbols_value.clone(),
    );
    object.insert("decision_protocol".to_string(), json!("actions_v1"));
    object
        .entry("summary".to_string())
        .or_insert_with(|| runtime_decision_summary_text(&action_summary));
    object.insert(
        "decision".to_string(),
        json!({
            "protocol": "actions_v1",
            "action_count": decision.actions.len(),
            "actions": action_values,
            "action_summary": action_summary,
            "selected_symbols": selected_symbols_value,
            "execution_logs": execution_logs_value,
        }),
    );
    diagnostics
}

fn runtime_action_values(actions: &[RuntimeStrategyAction]) -> Vec<Value> {
    actions
        .iter()
        .map(RuntimeStrategyAction::to_value)
        .collect()
}

fn runtime_action_summary(actions: &[RuntimeStrategyAction]) -> Value {
    let mut open_position = 0_u64;
    let mut close_position = 0_u64;
    let mut place_risk_order = 0_u64;
    let mut cancel_order = 0_u64;
    let mut modify_order = 0_u64;
    let mut hold = 0_u64;
    for action in actions {
        match action.action.trim().to_ascii_lowercase().as_str() {
            "open_position" => open_position += 1,
            "close_position" => close_position += 1,
            "place_risk_order" => place_risk_order += 1,
            "cancel_order" => cancel_order += 1,
            "modify_order" => modify_order += 1,
            "hold" => hold += 1,
            _ => unreachable!("runtime parser rejects unsupported actions"),
        }
    }
    json!({
        "open_position": open_position,
        "close_position": close_position,
        "place_risk_order": place_risk_order,
        "cancel_order": cancel_order,
        "modify_order": modify_order,
        "hold": hold,
        "total": actions.len(),
    })
}

fn runtime_selected_symbols(actions: &[RuntimeStrategyAction]) -> Vec<String> {
    let mut symbols = Vec::new();
    for action in actions {
        let symbol = action.symbol.trim();
        if symbol.is_empty() || symbols.iter().any(|item| item == symbol) {
            continue;
        }
        symbols.push(symbol.to_string());
    }
    symbols
}

fn runtime_decision_summary_text(summary: &Value) -> Value {
    let labels = [
        ("open_position", "开仓"),
        ("close_position", "平仓"),
        ("place_risk_order", "保护单"),
        ("cancel_order", "撤单"),
        ("modify_order", "改单"),
        ("hold", "等待"),
    ];
    let parts = labels
        .iter()
        .filter_map(|(key, label)| {
            let count = runtime_action_summary_count(summary, key);
            (count > 0).then(|| format!("{label} {count}"))
        })
        .collect::<Vec<_>>();
    if parts.is_empty() {
        json!("策略当前未返回可执行动作")
    } else {
        json!(format!("策略返回动作：{}", parts.join("，")))
    }
}

fn runtime_action_summary_count(summary: &Value, key: &str) -> u64 {
    summary
        .get(key)
        .and_then(Value::as_u64)
        .expect("runtime action summary count should be u64")
}

fn required_array_field<'a>(result: &'a Value, key: &str) -> AppResult<&'a Vec<Value>> {
    result
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::Runtime(format!("Python 策略执行器返回结果缺少有效 {key} list")))
}

fn required_object_value(result: &Value, key: &str) -> AppResult<Value> {
    let value = result
        .get(key)
        .ok_or_else(|| AppError::Runtime(format!("Python 策略执行器返回结果缺少 {key} dict")))?;
    if !value.is_object() {
        return Err(AppError::Runtime(format!(
            "Python 策略执行器返回结果的 {key} 必须是 dict"
        )));
    }
    Ok(value.clone())
}

fn optional_object_field(result: &Value, key: &str) -> AppResult<Value> {
    let Some(value) = result.get(key) else {
        return Ok(json!({}));
    };
    if !value.is_object() {
        return Err(AppError::Runtime(format!(
            "Python 策略执行器返回结果的 {key} 必须是 dict"
        )));
    }
    Ok(value.clone())
}

fn parse_runtime_actions(values: &[Value]) -> AppResult<Vec<RuntimeStrategyAction>> {
    let mut actions = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        actions.push(parse_runtime_action(value, index)?);
    }
    Ok(actions)
}

fn parse_runtime_action(value: &Value, index: usize) -> AppResult<RuntimeStrategyAction> {
    let object = value.as_object().ok_or_else(|| {
        AppError::Runtime(format!(
            "Python 策略执行器返回结果的 actions[{index}] 必须是 dict"
        ))
    })?;
    for key in RUNTIME_ACTIONS_V1_REMOVED_ACTION_ALIAS_FIELDS {
        if object.contains_key(*key) {
            return Err(AppError::Runtime(format!(
                "Python 策略执行器返回结果的 actions[{index}].{key} 旧动作别名已删除，请使用 action"
            )));
        }
    }
    for (alias, canonical) in RUNTIME_ACTIONS_V1_REMOVED_FIELD_ALIASES {
        if object.contains_key(*alias) {
            return Err(AppError::Runtime(format!(
                "Python 策略执行器返回结果的 actions[{index}].{alias} 字段别名已删除，请使用 {canonical}"
            )));
        }
    }
    for (field, reason) in RUNTIME_ACTIONS_V1_ENGINE_CONTROLLED_FIELDS {
        if object.contains_key(*field) {
            return Err(AppError::Runtime(format!(
                "Python 策略执行器返回结果的 actions[{index}].{field} 是交易引擎控制字段，{reason}"
            )));
        }
    }
    validate_runtime_action_json_field_types(value, index)?;
    let action = required_trimmed_string(value, "action", index)?.to_ascii_lowercase();
    if !RUNTIME_ACTIONS_V1_SUPPORTED_ACTIONS.contains(&action.as_str()) {
        let supported = RUNTIME_ACTIONS_V1_SUPPORTED_ACTIONS.join(", ");
        return Err(AppError::Runtime(format!(
            "Python 策略执行器返回结果的 actions[{index}].action={action} 不受支持，必须使用: {supported}"
        )));
    }
    validate_optional_action_enum_string(
        value,
        "target_order_kind",
        index,
        RUNTIME_ACTIONS_V1_TARGET_ORDER_KINDS,
    )?;
    validate_optional_action_order_type_enum_string(
        value,
        "target_order_type",
        index,
        LIVE_ALGO_TARGET_ORDER_TYPES,
    )?;
    Ok(RuntimeStrategyAction {
        action,
        symbol: required_trimmed_string(value, "symbol", index)?,
        side: required_trimmed_string(value, "side", index)?,
        order_type: required_trimmed_string(value, "order_type", index)?,
        price: optional_finite_f64_number(value, "price", index)?,
        reference_price: optional_finite_f64_number(value, "reference_price", index)?,
        reason: required_string(value, "reason", index)?,
        strength: required_finite_f64_number(value, "strength", index)?,
        timestamp: required_positive_i64_number(value, "timestamp", index)?,
        position_size: optional_finite_f64_number(value, "position_size", index)?,
        exchange_size: optional_trimmed_string(value, "exchange_size", index)?,
        raw: value.clone(),
    })
}

fn validate_runtime_action_json_field_types(value: &Value, index: usize) -> AppResult<()> {
    for field in RUNTIME_ACTIONS_V1_JSON_NUMBER_FIELDS {
        if let Some(raw) = value.get(*field) {
            if !raw.is_null() && raw.as_f64().filter(|item| item.is_finite()).is_none() {
                return Err(AppError::Runtime(format!(
                    "Python 策略执行器返回结果的 actions[{index}].{field} 必须是 JSON number，不能使用字符串数字"
                )));
            }
        }
    }
    for field in RUNTIME_ACTIONS_V1_JSON_BOOL_FIELDS {
        if let Some(raw) = value.get(*field) {
            if !raw.is_null() && !raw.is_boolean() {
                return Err(AppError::Runtime(format!(
                    "Python 策略执行器返回结果的 actions[{index}].{field} 必须是 JSON boolean"
                )));
            }
        }
    }
    for field in RUNTIME_ACTIONS_V1_JSON_TEXT_FIELDS {
        if let Some(raw) = value.get(*field) {
            if !raw.is_null() && !raw.is_string() {
                return Err(AppError::Runtime(format!(
                    "Python 策略执行器返回结果的 actions[{index}].{field} 必须是 JSON string"
                )));
            }
        }
    }
    Ok(())
}

fn required_trimmed_string(value: &Value, key: &str, index: usize) -> AppResult<String> {
    let raw = value.get(key).ok_or_else(|| {
        AppError::Runtime(format!(
            "Python 策略执行器返回结果的 actions[{index}].{key} 必须是非空字符串"
        ))
    })?;
    let text = raw.as_str().map(str::trim).filter(|item| !item.is_empty());
    text.map(str::to_string).ok_or_else(|| {
        AppError::Runtime(format!(
            "Python 策略执行器返回结果的 actions[{index}].{key} 必须是非空字符串"
        ))
    })
}

fn required_string(value: &Value, key: &str, index: usize) -> AppResult<String> {
    let raw = value.get(key).ok_or_else(|| {
        AppError::Runtime(format!(
            "Python 策略执行器返回结果的 actions[{index}].{key} 必须是 JSON string"
        ))
    })?;
    raw.as_str()
        .map(str::trim)
        .map(str::to_string)
        .ok_or_else(|| {
            AppError::Runtime(format!(
                "Python 策略执行器返回结果的 actions[{index}].{key} 必须是 JSON string"
            ))
        })
}

fn optional_trimmed_string(value: &Value, key: &str, index: usize) -> AppResult<Option<String>> {
    let Some(raw) = value.get(key) else {
        return Ok(None);
    };
    if raw.is_null() {
        return Ok(None);
    }
    let Some(text) = raw.as_str() else {
        return Err(AppError::Runtime(format!(
            "Python 策略执行器返回结果的 actions[{index}].{key} 必须是 JSON string"
        )));
    };
    let text = text.trim();
    Ok((!text.is_empty()).then(|| text.to_string()))
}

fn required_finite_f64_number(value: &Value, key: &str, index: usize) -> AppResult<f64> {
    let raw = value.get(key).ok_or_else(|| {
        AppError::Runtime(format!(
            "Python 策略执行器返回结果的 actions[{index}].{key} 必须是 JSON number"
        ))
    })?;
    finite_f64_number(raw).ok_or_else(|| {
        AppError::Runtime(format!(
            "Python 策略执行器返回结果的 actions[{index}].{key} 必须是 JSON number"
        ))
    })
}

fn optional_finite_f64_number(value: &Value, key: &str, index: usize) -> AppResult<Option<f64>> {
    let Some(raw) = value.get(key) else {
        return Ok(None);
    };
    if raw.is_null() {
        return Ok(None);
    }
    finite_f64_number(raw).map(Some).ok_or_else(|| {
        AppError::Runtime(format!(
            "Python 策略执行器返回结果的 actions[{index}].{key} 必须是 JSON number"
        ))
    })
}

fn finite_f64_number(value: &Value) -> Option<f64> {
    value.as_f64().filter(|item| item.is_finite())
}

fn required_positive_i64_number(value: &Value, key: &str, index: usize) -> AppResult<i64> {
    let raw = value.get(key).ok_or_else(|| {
        AppError::Runtime(format!(
            "Python 策略执行器返回结果的 actions[{index}].{key} 必须是有效毫秒时间戳"
        ))
    })?;
    let value = raw
        .as_i64()
        .or_else(|| raw.as_u64().and_then(|value| i64::try_from(value).ok()))
        .filter(|value| *value > 0);
    value.ok_or_else(|| {
        AppError::Runtime(format!(
            "Python 策略执行器返回结果的 actions[{index}].{key} 必须是有效毫秒时间戳"
        ))
    })
}

fn validate_optional_action_enum_string(
    value: &Value,
    key: &str,
    index: usize,
    supported_values: &[&str],
) -> AppResult<()> {
    let Some(raw) = value.get(key) else {
        return Ok(());
    };
    if raw.is_null() {
        return Ok(());
    }
    let Some(text) = raw.as_str().map(str::trim).filter(|item| !item.is_empty()) else {
        return Err(AppError::Runtime(format!(
            "Python 策略执行器返回结果的 actions[{index}].{key} 必须是 JSON string"
        )));
    };
    let normalized = text.to_ascii_lowercase();
    if supported_values.contains(&normalized.as_str()) {
        return Ok(());
    }
    let supported = supported_values.join(", ");
    Err(AppError::Runtime(format!(
        "Python 策略执行器返回结果的 actions[{index}].{key}={normalized} 不受支持，必须使用: {supported}"
    )))
}

fn validate_optional_action_order_type_enum_string(
    value: &Value,
    key: &str,
    index: usize,
    supported_values: &[&str],
) -> AppResult<()> {
    let Some(raw) = value.get(key) else {
        return Ok(());
    };
    if raw.is_null() {
        return Ok(());
    }
    let Some(text) = raw.as_str().map(str::trim).filter(|item| !item.is_empty()) else {
        return Err(AppError::Runtime(format!(
            "Python 策略执行器返回结果的 actions[{index}].{key} 必须是 JSON string"
        )));
    };
    let normalized = normalize_runtime_order_type_text(text);
    if supported_values.contains(&normalized.as_str()) {
        return Ok(());
    }
    let supported = supported_values.join(", ");
    Err(AppError::Runtime(format!(
        "Python 策略执行器返回结果的 actions[{index}].{key}={normalized} 不受支持，必须使用: {supported}"
    )))
}

fn parse_runtime_execution_logs(values: &[Value]) -> AppResult<Vec<RuntimeStrategyExecutionLog>> {
    let mut logs = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let Some(object) = value.as_object() else {
            return Err(AppError::Runtime(format!(
                "Python 策略执行器返回结果的 execution_logs[{index}] 必须是 dict"
            )));
        };
        let stage = required_runtime_log_string(value, index, "stage")?;
        let level = required_runtime_log_string(value, index, "level")?;
        match level.as_str() {
            "info" | "warn" | "error" | "success" => {}
            _ => {
                return Err(AppError::Runtime(format!(
                    "Python 策略执行器返回结果的 execution_logs[{index}].level 必须是 info/warn/error/success"
                )));
            }
        }
        let message = required_runtime_log_string(value, index, "message")?;
        let Some(details) = object.get("details").filter(|details| details.is_object()) else {
            return Err(AppError::Runtime(format!(
                "Python 策略执行器返回结果的 execution_logs[{index}].details 必须是 dict"
            )));
        };
        logs.push(RuntimeStrategyExecutionLog {
            stage,
            level,
            message,
            details: details.clone(),
        });
    }
    Ok(logs)
}

fn required_runtime_log_string(value: &Value, index: usize, key: &str) -> AppResult<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            AppError::Runtime(format!(
                "Python 策略执行器返回结果的 execution_logs[{index}].{key} 必须是非空字符串"
            ))
        })
}

#[cfg(test)]
mod result_contract_tests {
    use super::*;

    #[test]
    fn runtime_result_requires_actions_array() {
        let result = json!({
            "ok": true,
            "diagnostics": {},
            "execution_logs": [],
        });

        let error = runtime_decision_from_result(&result)
            .expect_err("missing actions must not be parsed as an empty decision")
            .to_string();

        assert!(error.contains("actions"));
        assert!(error.contains("list"));
    }

    #[test]
    fn runtime_result_rejects_removed_top_level_output_fields_without_python_sidecar() {
        let result = json!({
            "ok": true,
            "actions": [],
            "orders": [],
            "risk_orders": [],
            "signals": [],
            "portfolio_layers": [],
            "diagnostics": {},
            "execution_logs": [],
        });

        let error = runtime_decision_from_result(&result)
            .expect_err("Rust parser must reject removed top-level strategy outputs")
            .to_string();

        assert!(error.contains("旧顶层字段"));
        assert!(error.contains("orders/risk_orders/signals/portfolio_layers"));
        assert!(error.contains("actions_v1"));
    }

    #[test]
    fn runtime_result_rejects_invalid_execution_log_level() {
        let result = json!({
            "ok": true,
            "actions": [],
            "diagnostics": {},
            "execution_logs": [{
                "stage": "strategy",
                "level": "verbose",
                "message": "bad level",
                "details": {},
            }],
        });

        let error = runtime_decision_from_result(&result)
            .expect_err("invalid execution log level must not be normalized silently")
            .to_string();

        assert!(error.contains("execution_logs[0].level"));
    }

    #[test]
    fn runtime_result_rejects_malformed_action_item() {
        let result = json!({
            "ok": true,
            "actions": [{
                "action": "open_position",
                "symbol": "BTC-USDT-SWAP",
                "side": "long",
                "order_type": "market",
                "reason": "invalid_timestamp",
                "strength": 0.5,
                "timestamp": 0,
            }],
            "diagnostics": {},
            "execution_logs": [],
        });

        let error = runtime_decision_from_result(&result)
            .expect_err("invalid action timestamp must not be coerced to a valid action")
            .to_string();

        assert!(error.contains("actions[0].timestamp"));
    }

    #[test]
    fn runtime_result_rejects_missing_normalized_action_fields_without_python_sidecar() {
        let base_action = json!({
            "action": "open_position",
            "symbol": "BTC-USDT-SWAP",
            "side": "long",
            "order_type": "market",
            "reason": "",
            "strength": 0.5,
            "timestamp": 1_780_290_000_000_i64,
        });

        for key in ["order_type", "reason", "strength", "timestamp"] {
            let mut action = base_action.clone();
            action.as_object_mut().unwrap().remove(key);
            let result = json!({
                "ok": true,
                "actions": [action],
                "diagnostics": {},
                "execution_logs": [],
            });

            let error = runtime_decision_from_result(&result)
                .expect_err("Rust parser must not synthesize normalized action fields")
                .to_string();

            assert!(
                error.contains(&format!("actions[0].{key}")),
                "{key} should be reported as missing, got: {error}"
            );
        }
    }

    #[test]
    fn runtime_result_rejects_string_numeric_action_fields_without_python_sidecar() {
        let result = json!({
            "ok": true,
            "actions": [{
                "action": "open_position",
                "symbol": "BTC-USDT-SWAP",
                "side": "long",
                "order_type": "market",
                "price": "100.5",
                "reason": "string_price",
                "strength": 0.5,
                "timestamp": 1_780_290_000_000_i64,
            }],
            "diagnostics": {},
            "execution_logs": [],
        });

        let error = runtime_decision_from_result(&result)
            .expect_err("Rust parser must reject string numeric action fields")
            .to_string();

        assert!(error.contains("actions[0].price"));
        assert!(error.contains("JSON number"));
    }

    #[test]
    fn runtime_result_rejects_non_string_action_text_fields_without_python_sidecar() {
        let result = json!({
            "ok": true,
            "actions": [{
                "action": "open_position",
                "symbol": "BTC-USDT-SWAP",
                "side": "long",
                "order_type": 123,
                "reason": "numeric_order_type",
                "strength": 0.5,
                "timestamp": 1_780_290_000_000_i64,
            }],
            "diagnostics": {},
            "execution_logs": [],
        });

        let error = runtime_decision_from_result(&result)
            .expect_err("Rust parser must reject non-string action text fields")
            .to_string();

        assert!(error.contains("actions[0].order_type"));
        assert!(error.contains("JSON string"));
    }

    #[test]
    fn runtime_result_rejects_legacy_action_alias_without_python_sidecar() {
        let result = json!({
            "ok": true,
            "actions": [{
                "intent_action": "entry",
                "symbol": "BTC-USDT-SWAP",
                "side": "long",
                "timestamp": 1_780_290_000_000_i64,
            }],
            "diagnostics": {},
            "execution_logs": [],
        });

        let error = runtime_decision_from_result(&result)
            .expect_err("Rust parser must reject legacy action aliases")
            .to_string();

        assert!(error.contains("actions[0].intent_action"));
        assert!(error.contains("旧动作别名已删除"));
    }

    #[test]
    fn runtime_result_rejects_engine_controlled_action_field_without_python_sidecar() {
        let result = json!({
            "ok": true,
            "actions": [{
                "action": "open_position",
                "symbol": "BTC-USDT-SWAP",
                "side": "long",
                "leverage": 10,
                "timestamp": 1_780_290_000_000_i64,
            }],
            "diagnostics": {},
            "execution_logs": [],
        });

        let error = runtime_decision_from_result(&result)
            .expect_err("Rust parser must reject strategy-controlled leverage")
            .to_string();

        assert!(error.contains("actions[0].leverage"));
        assert!(error.contains("交易引擎控制字段"));
    }

    #[test]
    fn runtime_result_rejects_unknown_action_and_target_kind_without_python_sidecar() {
        let unsupported_action = json!({
            "ok": true,
            "actions": [{
                "action": "buy",
                "symbol": "BTC-USDT-SWAP",
                "side": "long",
                "order_type": "market",
                "reason": "unsupported_action",
                "strength": 0.5,
                "timestamp": 1_780_290_000_000_i64,
            }],
            "diagnostics": {},
            "execution_logs": [],
        });
        let action_error = runtime_decision_from_result(&unsupported_action)
            .expect_err("Rust parser must reject unsupported actions")
            .to_string();
        assert!(action_error.contains("actions[0].action=buy 不受支持"));

        let unsupported_target = json!({
            "ok": true,
            "actions": [{
                "action": "cancel_order",
                "symbol": "BTC-USDT-SWAP",
                "side": "flat",
                "order_type": "market",
                "target_order_kind": "paper",
                "reason": "unsupported_target_kind",
                "strength": 0.5,
                "timestamp": 1_780_290_000_000_i64,
            }],
            "diagnostics": {},
            "execution_logs": [],
        });
        let target_error = runtime_decision_from_result(&unsupported_target)
            .expect_err("Rust parser must reject paper target kind")
            .to_string();
        assert!(target_error.contains("actions[0].target_order_kind=paper 不受支持"));
    }

    #[test]
    fn runtime_result_accepts_hyphen_target_order_type_without_python_sidecar() {
        let result = json!({
            "ok": true,
            "actions": [{
                "action": "modify_order",
                "symbol": "BTC-USDT-SWAP",
                "side": "flat",
                "order_type": "market",
                "target_order_kind": "algo",
                "target_order_type": "stop-market",
                "client_order_id": "algo-order-1",
                "new_price": "98.5",
                "reason": "hyphen_target_order_type",
                "strength": 0.5,
                "timestamp": 1_780_290_000_000_i64,
            }],
            "diagnostics": {},
            "execution_logs": [],
        });

        let decision = runtime_decision_from_result(&result)
            .expect("Rust parser should accept the same hyphen order type aliases as Python");

        assert_eq!(decision.actions.len(), 1);
        assert_eq!(
            decision.actions[0].raw["target_order_type"].as_str(),
            Some("stop-market")
        );
    }
}
