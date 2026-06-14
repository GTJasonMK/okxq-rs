use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct EnvFile {
    lines: Vec<String>,
    values: BTreeMap<String, String>,
}

impl EnvFile {
    pub fn load(path: &Path) -> AppResult<Self> {
        let content = match std::fs::read_to_string(path) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error.into()),
        };
        let lines = content.lines().map(ToOwned::to_owned).collect::<Vec<_>>();
        let values = parse_values(&lines);
        Ok(Self { lines, values })
    }

    pub fn get(&self, key: &str) -> String {
        self.values.get(key).cloned().unwrap_or_default()
    }

    pub fn get_non_empty(&self, key: &str) -> Option<String> {
        let value = self.get(key);
        if value.trim().is_empty() {
            None
        } else {
            Some(value)
        }
    }

    pub fn get_bool(&self, key: &str, default: bool) -> bool {
        match self.get(key).to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "y" | "on" => true,
            "false" | "0" | "no" | "n" | "off" => false,
            _ => default,
        }
    }

    pub fn get_u64(&self, key: &str, default: u64) -> u64 {
        self.get(key).parse::<u64>().unwrap_or(default)
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.values.insert(key.to_string(), value.to_string());
    }

    pub fn save(&self, path: &Path) -> AppResult<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut written = BTreeSet::new();
        let mut next_lines = Vec::new();
        for line in &self.lines {
            if let Some((key, _)) = split_env_line(line) {
                if let Some(value) = self.values.get(&key) {
                    next_lines.push(format!("{key}={value}"));
                    written.insert(key);
                }
                continue;
            }
            next_lines.push(line.clone());
        }

        for (key, value) in &self.values {
            if written.contains(key) {
                continue;
            }
            if !next_lines.is_empty() && next_lines.last().is_some_and(|line| !line.is_empty()) {
                next_lines.push(String::new());
            }
            next_lines.push(format!("{key}={value}"));
        }

        let tmp_path = path.with_extension("tmp");
        let mut content = next_lines.join("\n");
        if !content.is_empty() {
            content.push('\n');
        }
        std::fs::write(&tmp_path, content)
            .map_err(|error| AppError::Io(format!("写入临时配置失败: {error}")))?;
        std::fs::rename(&tmp_path, path)
            .map_err(|error| AppError::Io(format!("替换配置文件失败: {error}")))?;
        Ok(())
    }
}

fn parse_values(lines: &[String]) -> BTreeMap<String, String> {
    lines
        .iter()
        .filter_map(|line| split_env_line(line))
        .collect::<BTreeMap<_, _>>()
}

fn split_env_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') || !trimmed.contains('=') {
        return None;
    }
    let (key, value) = trimmed.split_once('=')?;
    Some((key.trim().to_string(), unquote(value.trim())))
}

fn unquote(value: &str) -> String {
    let bytes = value.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
    {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}
