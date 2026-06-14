pub(super) fn now_text() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub(super) fn bool_i64(flag: bool) -> i64 {
    if flag {
        1
    } else {
        0
    }
}
