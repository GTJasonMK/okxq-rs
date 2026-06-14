use std::collections::BTreeMap;

use crate::error::AppResult;

use super::super::normalize::normalize_inst_id;

pub(super) fn normalize_inst_ids(inst_ids: &[String]) -> AppResult<Vec<String>> {
    let mut normalized = Vec::new();
    for inst_id in inst_ids {
        let inst_id = normalize_inst_id(inst_id)?;
        if !normalized.contains(&inst_id) {
            normalized.push(inst_id);
        }
    }
    Ok(normalized)
}

pub(super) fn decrement_ref(map: &mut BTreeMap<String, usize>, key: &str) {
    let should_remove = if let Some(count) = map.get_mut(key) {
        if *count > 1 {
            *count -= 1;
            false
        } else {
            true
        }
    } else {
        false
    };
    if should_remove {
        map.remove(key);
    }
}
