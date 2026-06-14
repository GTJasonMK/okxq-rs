use std::collections::BTreeSet;

pub(super) fn normalize_collection_symbols(raw: &[String]) -> Result<Vec<String>, String> {
    let mut seen = BTreeSet::new();
    let symbols = raw
        .iter()
        .map(|item| normalize_inst_id(item))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|item| seen.insert(item.clone()))
        .collect::<Vec<_>>();

    if symbols.is_empty() {
        Err("秒级采集器需要至少一个明确的采集标的".to_string())
    } else {
        Ok(symbols)
    }
}

pub(super) fn collection_symbol_filter(active_symbols: &[String]) -> BTreeSet<String> {
    active_symbols.iter().cloned().collect()
}

pub(super) fn normalize_book_channel(value: &str) -> Result<String, String> {
    let normalized = value.trim();
    if normalized == "books5" {
        Ok(normalized.to_string())
    } else {
        Err(format!(
            "秒级采集器只支持 books5 盘口频道，当前配置为 {normalized:?}"
        ))
    }
}

fn normalize_inst_id(value: &str) -> Result<String, String> {
    let normalized = value.trim().to_uppercase();
    if normalized.is_empty() {
        Err("秒级采集器采集标的不能为空".to_string())
    } else if !normalized.contains('-') {
        Err(format!("秒级采集器采集标的格式无效: {normalized}"))
    } else {
        Ok(normalized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_collector_requires_explicit_symbols() {
        let result = normalize_collection_symbols(&[]);

        assert!(result.is_err());
    }

    #[test]
    fn tick_collector_rejects_invalid_symbol_instead_of_dropping_it() {
        let result = normalize_collection_symbols(&["BTCUSDT".to_string()]);

        assert!(result.is_err());
    }

    #[test]
    fn tick_collector_deduplicates_explicit_symbols() {
        let result = normalize_collection_symbols(&[
            "btc-usdt-swap".to_string(),
            "BTC-USDT-SWAP".to_string(),
        ])
        .expect("valid symbols should normalize");

        assert_eq!(result, vec!["BTC-USDT-SWAP".to_string()]);
    }

    #[test]
    fn tick_collector_rejects_unsupported_book_channel() {
        let result = normalize_book_channel("books");

        assert!(result.is_err());
    }
}
