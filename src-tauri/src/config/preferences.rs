mod store;
mod watched;

pub use self::store::PreferencesStore;
pub use self::watched::{AddWatchedSymbolRequest, WatchedSymbolRecord, WatchedSymbolSyncPlan};

const WATCHED_SYMBOLS_KEY: &str = "watched_symbols";
