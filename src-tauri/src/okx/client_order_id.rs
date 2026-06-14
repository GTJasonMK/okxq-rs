use crate::error::{AppError, AppResult};

const OKX_CLIENT_ORDER_ID_MAX_LEN: usize = 32;
const OKXQ_CLIENT_ORDER_ID_PREFIX: &str = "okxq";
const OKXQ_CLIENT_ORDER_ID_RANDOM_LEN: usize = 24;

pub(crate) fn generate_okx_client_order_id() -> String {
    let random = uuid::Uuid::new_v4().simple().to_string();
    format!(
        "{OKXQ_CLIENT_ORDER_ID_PREFIX}{}",
        &random[..OKXQ_CLIENT_ORDER_ID_RANDOM_LEN]
    )
}

pub(crate) fn normalized_okx_client_order_id(client_order_id: &str) -> AppResult<Option<&str>> {
    let client_order_id = client_order_id.trim();
    if client_order_id.is_empty() {
        return Ok(None);
    }
    if !is_okx_client_order_id_compatible(client_order_id) {
        return Err(AppError::Validation(
            "OKX 下单参数 clOrdId 只能包含 1-32 位英文字母或数字".to_string(),
        ));
    }
    Ok(Some(client_order_id))
}

pub(crate) fn is_okx_client_order_id_compatible(client_order_id: &str) -> bool {
    let client_order_id = client_order_id.trim();
    !client_order_id.is_empty()
        && client_order_id.len() <= OKX_CLIENT_ORDER_ID_MAX_LEN
        && client_order_id
            .chars()
            .all(|item| item.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_okx_client_order_id_is_exchange_compatible() {
        let client_order_id = generate_okx_client_order_id();

        assert!(client_order_id.starts_with("okxq"));
        assert!(is_okx_client_order_id_compatible(&client_order_id));
        assert!(client_order_id.len() <= OKX_CLIENT_ORDER_ID_MAX_LEN);
        assert!(client_order_id
            .chars()
            .all(|item| item.is_ascii_alphanumeric()));
    }

    #[test]
    fn normalized_okx_client_order_id_rejects_non_alphanumeric_values() {
        let error = normalized_okx_client_order_id("okxq_bad")
            .expect_err("underscore is rejected by OKX clOrdId rules")
            .to_string();

        assert!(error.contains("clOrdId"));
    }

    #[test]
    fn normalized_okx_client_order_id_rejects_overlong_values() {
        let error = normalized_okx_client_order_id("123456789012345678901234567890123")
            .expect_err("clOrdId longer than 32 chars must be rejected")
            .to_string();

        assert!(error.contains("clOrdId"));
    }
}
