//! OKX outbound governor.
//!
//! The original project used a sliding-window governor keyed by OKX operation
//! and official scope. A token bucket can still allow a cold-start burst, which
//! is exactly what hurts history-candle backfills. Keep the public type names so
//! existing callers remain stable, but enforce strict rolling windows here.

mod concurrency;
mod registry;
mod settings;
mod window;

pub use self::registry::{SharedTokenBucketRegistry, TokenBucketRegistry};
pub use self::settings::OKXConcurrencySettings;

pub(super) const GLOBAL_CONCURRENCY_KEY: &str = "global";
pub(super) const UNKNOWN_CONCURRENCY_KEY: &str = "unknown";
