use std::path::PathBuf;

use serde_json::{Map, Value};
use tokio::sync::Mutex;

use super::watched::WatchedSymbolRecord;

mod core;
mod watched;

pub struct PreferencesStore {
    path: PathBuf,
    lock: Mutex<PreferencesStoreState>,
}

#[derive(Default)]
pub(in crate::config::preferences::store) struct PreferencesStoreState {
    data: Option<Map<String, Value>>,
    watched_symbols: Option<Vec<WatchedSymbolRecord>>,
}

impl PreferencesStore {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            lock: Mutex::new(PreferencesStoreState::default()),
        }
    }
}
