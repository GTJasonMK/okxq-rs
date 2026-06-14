use std::{collections::HashMap, path::Path, sync::Mutex};

use serde_json::{Map, Value};

use crate::error::{AppError, AppResult};

use super::{
    files::{
        discoverable_strategy_files, find_discoverable_strategy_file, strategy_scan_signature,
        StrategyFileFingerprint,
    },
    runner::discover_runtime_strategy,
    types::RuntimeStrategyMeta,
};

#[derive(Default)]
struct RegistryState {
    metas_by_id: HashMap<String, RuntimeStrategyMeta>,
    scan_signature: Option<Vec<StrategyFileFingerprint>>,
    scan_metas: Vec<RuntimeStrategyMeta>,
}

static REGISTRY: Mutex<Option<RegistryState>> = Mutex::new(None);

fn registry() -> &'static Mutex<Option<RegistryState>> {
    &REGISTRY
}

fn with_registry<F, R>(f: F) -> R
where
    F: FnOnce(&mut RegistryState) -> R,
{
    let mut guard = registry().lock().unwrap();
    let state = guard.get_or_insert_with(RegistryState::default);
    f(state)
}

/// 扫描 strategies 目录，发现并注册所有运行时策略。
pub fn scan_and_register(project_root: &Path) -> AppResult<Vec<RuntimeStrategyMeta>> {
    let file_names = discoverable_strategy_files(project_root)?;
    let signature = strategy_scan_signature(project_root, &file_names)?;
    if let Some(cached) = with_registry(|state| {
        (state.scan_signature.as_ref() == Some(&signature)).then(|| state.scan_metas.clone())
    }) {
        return Ok(cached);
    }

    let mut metas = Vec::new();
    for file_name in file_names {
        match discover_runtime_strategy(project_root, &file_name) {
            Ok(meta) => {
                metas.push(meta);
            }
            Err(error) => {
                tracing::warn!("跳过运行策略文件 {}: {}", file_name, error);
            }
        }
    }

    with_registry(|state| {
        state.metas_by_id.clear();
        for meta in &metas {
            state
                .metas_by_id
                .insert(meta.strategy_id.clone(), meta.clone());
        }
        state.scan_metas = metas.clone();
        state.scan_signature = Some(signature);
    });

    Ok(metas)
}

pub fn ensure_registered(project_root: &Path, strategy_id: &str) -> AppResult<RuntimeStrategyMeta> {
    if let Some(meta) = get_meta(strategy_id) {
        return Ok(meta);
    }
    let Some(file_name) = find_discoverable_strategy_file(project_root, strategy_id)? else {
        return Err(AppError::Validation(format!(
            "策略不存在或无法用于运行: {strategy_id}"
        )));
    };
    let meta = discover_runtime_strategy(project_root, &file_name)?;
    register(meta.clone());
    Ok(meta)
}

/// 注册单个运行策略
pub fn register(meta: RuntimeStrategyMeta) {
    with_registry(|state| {
        state.metas_by_id.insert(meta.strategy_id.clone(), meta);
        state.scan_signature = None;
    });
}

/// 获取运行策略元数据
pub fn get_meta(strategy_id: &str) -> Option<RuntimeStrategyMeta> {
    with_registry(|state| state.metas_by_id.get(strategy_id).cloned())
}

/// 从策略 RUNTIME_CONFIG 生成默认参数。
pub fn default_params(strategy_id: &str) -> Option<Map<String, Value>> {
    let meta = get_meta(strategy_id)?;
    let mut params = Map::new();
    if let Some(runtime_params) = meta.runtime_config.get("params").and_then(Value::as_object) {
        for (key, value) in runtime_params {
            params.insert(key.clone(), value.clone());
        }
    }
    Some(params)
}

/// 合并运行策略默认参数和诊断上下文参数。
pub fn merge_default_params(
    strategy_id: &str,
    context_params: Map<String, Value>,
) -> Map<String, Value> {
    let mut params = default_params(strategy_id).unwrap_or_default();
    for (key, value) in context_params {
        params.insert(key, value);
    }
    params
}
