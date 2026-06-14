use crate::error::{AppError, AppResult};

pub fn mask_key(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    if value.chars().count() < 10 {
        return "*".repeat(value.chars().count());
    }
    let prefix: String = value.chars().take(4).collect();
    let suffix: String = value
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("{prefix}{}{suffix}", "*".repeat(value.chars().count() - 8))
}

pub fn sanitize_secret_value(field: &str, incoming: &str, existing: &str) -> AppResult<String> {
    let value = incoming.trim();
    if value.is_empty() {
        return Ok(existing.to_string());
    }
    if value.contains('*') {
        if existing.is_empty() {
            return Err(AppError::Validation(format!(
                "{field} 为遮蔽值，请输入真实密钥"
            )));
        }
        return Ok(existing.to_string());
    }
    Ok(value.to_string())
}
