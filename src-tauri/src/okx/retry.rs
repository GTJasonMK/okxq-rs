use std::{error::Error as _, time::Duration};

pub(super) const RETRY_LIMIT: u32 = 3;
const NETWORK_RETRY_BASE_MS: u64 = 200;
const RETRYABLE_ERRORS: &[&str] = &[
    "connection reset",
    "connection closed",
    "timeout",
    "timed out",
    "end of file",
    "unexpected eof",
    "broken pipe",
    "dns",
    "tls",
    "ssl",
    "connect",
];

pub(super) fn should_retry_network_error(error: &reqwest::Error, attempt: u32) -> bool {
    attempt < RETRY_LIMIT && is_retryable_network_error(error)
}

pub(super) fn network_retry_backoff(attempt: u32) -> Duration {
    Duration::from_millis(NETWORK_RETRY_BASE_MS * 2u64.pow(attempt.saturating_sub(1)))
}

pub(super) fn reqwest_error_chain(error: &reqwest::Error) -> String {
    let mut parts = vec![error.to_string()];
    let mut source = error.source();
    while let Some(item) = source {
        parts.push(item.to_string());
        source = item.source();
    }
    parts.join("; caused by: ")
}

fn is_retryable_network_error(error: &reqwest::Error) -> bool {
    let err_str = error.to_string().to_lowercase();
    RETRYABLE_ERRORS.iter().any(|kw| err_str.contains(kw))
}
