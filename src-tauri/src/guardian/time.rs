pub(super) fn now_text() -> String {
    chrono::Utc::now().to_rfc3339()
}
