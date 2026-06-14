use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use crate::error::{AppError, AppResult};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct StrategyFileFingerprint {
    file_name: String,
    len: u64,
    modified_ms: u128,
}

pub(super) fn strategy_scan_signature(
    project_root: &Path,
    file_names: &[String],
) -> AppResult<Vec<StrategyFileFingerprint>> {
    file_names
        .iter()
        .map(|file_name| {
            let path = strategy_file_path(project_root, file_name)?;
            let metadata = std::fs::metadata(path)?;
            let modified_ms = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_millis())
                .unwrap_or(0);
            Ok(StrategyFileFingerprint {
                file_name: file_name.clone(),
                len: metadata.len(),
                modified_ms,
            })
        })
        .collect()
}

/// 返回当前参与自动发现的策略文件。归档目录不参与注册，避免旧版本策略 ID 冲突。
pub fn discoverable_strategy_files(project_root: &Path) -> AppResult<Vec<String>> {
    let mut files = Vec::new();
    let base_dir = primary_strategy_source_root(project_root);
    if !base_dir.is_dir() {
        return Ok(files);
    }
    collect_strategy_files_in_dir(&base_dir, Path::new(""), &mut files)?;
    let runtime_dir = base_dir.join("runtime");
    if runtime_dir.is_dir() {
        collect_strategy_files_in_dir(&base_dir, Path::new("runtime"), &mut files)?;
    }
    files.sort();
    files.dedup();
    Ok(files)
}

pub fn discoverable_strategy_ids_fast(project_root: &Path) -> AppResult<Vec<String>> {
    let mut ids = Vec::new();
    for file_name in discoverable_strategy_files(project_root)? {
        let path = strategy_file_path(project_root, &file_name)?;
        let source = std::fs::read_to_string(path)?;
        if let Some(strategy_id) = python_string_assignment(&source, "STRATEGY_ID") {
            ids.push(strategy_id);
        }
    }
    ids.sort();
    ids.dedup();
    Ok(ids)
}

pub fn find_discoverable_strategy_file(
    project_root: &Path,
    strategy_id: &str,
) -> AppResult<Option<String>> {
    let target = strategy_id.trim();
    if target.is_empty() {
        return Ok(None);
    }
    for file_name in discoverable_strategy_files(project_root)? {
        let path = strategy_file_path(project_root, &file_name)?;
        let source = std::fs::read_to_string(path)?;
        if python_string_assignment(&source, "STRATEGY_ID").as_deref() == Some(target) {
            return Ok(Some(file_name));
        }
    }
    Ok(None)
}

fn python_string_assignment(source: &str, name: &str) -> Option<String> {
    for line in source.lines() {
        let line = line.trim_start();
        if !line.starts_with(name) {
            continue;
        }
        let rest = line[name.len()..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let rest = rest.trim_start();
        let quote = rest.chars().next()?;
        if quote != '"' && quote != '\'' {
            continue;
        }
        let raw = &rest[quote.len_utf8()..];
        let mut value = String::new();
        let mut escaped = false;
        for ch in raw.chars() {
            if escaped {
                value.push(ch);
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == quote {
                return (!value.trim().is_empty()).then_some(value);
            }
            value.push(ch);
        }
    }
    None
}

fn collect_strategy_files_in_dir(
    base_dir: &Path,
    relative_dir: &Path,
    files: &mut Vec<String>,
) -> AppResult<()> {
    let dir = base_dir.join(relative_dir);
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|item| item.to_str()) != Some("py") {
            continue;
        }
        let relative_path = path
            .strip_prefix(base_dir)
            .map_err(|error| AppError::Runtime(format!("计算策略相对路径失败: {error}")))?;
        files.push(normalize_strategy_file_name(
            &relative_path.display().to_string(),
        )?);
    }
    Ok(())
}

pub fn normalize_strategy_file_name(file_name: &str) -> AppResult<String> {
    let raw = file_name.trim();
    if raw.is_empty() || raw.contains('\\') {
        return Err(AppError::Validation(
            "策略文件名必须是策略目录下的相对 .py 路径".to_string(),
        ));
    }

    let path = Path::new(raw);
    if path.is_absolute() {
        return Err(AppError::Validation(
            "运行策略文件名不能是绝对路径".to_string(),
        ));
    }

    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                let part = part.to_str().ok_or_else(|| {
                    AppError::Validation("运行策略文件名必须是有效 UTF-8".to_string())
                })?;
                if part.is_empty() || part == "__pycache__" {
                    return Err(AppError::Validation(
                        "运行策略文件路径包含非法目录".to_string(),
                    ));
                }
                parts.push(part.to_string());
            }
            Component::CurDir => {}
            _ => {
                return Err(AppError::Validation(
                    "运行策略文件路径不能包含 .. 或根目录".to_string(),
                ));
            }
        }
    }

    let Some(last) = parts.last() else {
        return Err(AppError::Validation("运行策略文件名不能为空".to_string()));
    };
    if !last.ends_with(".py") {
        return Err(AppError::Validation(
            "运行策略文件名必须以 .py 结尾".to_string(),
        ));
    }

    Ok(parts.join("/"))
}

pub(super) fn strategy_file_path(project_root: &Path, file_name: &str) -> AppResult<PathBuf> {
    let normalized = normalize_strategy_file_name(file_name)?;
    let root = primary_strategy_source_root(project_root);
    let candidate = root.join(&normalized);
    if candidate.exists() {
        return Ok(candidate);
    }
    Ok(root.join(normalized))
}

fn primary_strategy_source_root(project_root: &Path) -> PathBuf {
    project_root.join("strategies")
}

pub fn runtime_execution_stamp(project_root: &Path, file_name: &str) -> Value {
    let strategy_path = strategy_file_path(project_root, file_name)
        .unwrap_or_else(|_| primary_strategy_source_root(project_root).join(file_name));
    let runner_path = project_root.join("src-tauri/python/strategy_runner.py");
    json!({
        "schema": "runtime_execution_stamp_v1",
        "strategy": file_stamp(project_root, &strategy_path),
        "runner": file_stamp(project_root, &runner_path),
    })
}

fn file_stamp(project_root: &Path, path: &Path) -> Value {
    let relative_path = path
        .strip_prefix(project_root)
        .ok()
        .map(|item| item.display().to_string())
        .unwrap_or_else(|| path.display().to_string());
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) => {
            return json!({
                "path": path.display().to_string(),
                "project_relative_path": relative_path,
                "exists": false,
                "error": error.to_string(),
            });
        }
    };
    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let sha256 = fs::read(path)
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
        .unwrap_or_default();
    json!({
        "path": path.display().to_string(),
        "project_relative_path": relative_path,
        "exists": true,
        "len": metadata.len(),
        "modified_ms": modified_ms,
        "sha256": sha256,
    })
}
