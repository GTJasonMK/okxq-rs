use serde_json::{Map, Value};

use crate::error::AppResult;

use super::{PreferencesStore, PreferencesStoreState};

impl PreferencesStore {
    pub async fn load(&self) -> AppResult<Map<String, Value>> {
        let mut state = self.lock.lock().await;
        self.load_cached_unlocked(&mut state)
    }

    pub async fn get(&self, key: &str) -> AppResult<Option<Value>> {
        let data = self.load().await?;
        Ok(data.get(key).cloned())
    }

    pub async fn save_all(&self, data: Map<String, Value>) -> AppResult<()> {
        let mut state = self.lock.lock().await;
        tracing::info!(
            path = %self.path.display(),
            keys = data.len(),
            "saving preferences"
        );
        self.save_cached_unlocked(&mut state, &data)
    }

    pub async fn merge(&self, partial: Map<String, Value>) -> AppResult<Map<String, Value>> {
        let mut state = self.lock.lock().await;
        let mut data = self.load_cached_unlocked(&mut state)?;
        tracing::debug!(
            path = %self.path.display(),
            keys = partial.len(),
            "merging preferences"
        );
        data.extend(partial);
        self.save_cached_unlocked(&mut state, &data)?;
        Ok(data)
    }

    pub async fn delete(&self, key: &str) -> AppResult<Map<String, Value>> {
        let mut state = self.lock.lock().await;
        let mut data = self.load_cached_unlocked(&mut state)?;
        tracing::info!(
            path = %self.path.display(),
            key,
            "deleting preference key"
        );
        data.remove(key);
        self.save_cached_unlocked(&mut state, &data)?;
        Ok(data)
    }

    pub(in crate::config::preferences::store) fn load_cached_unlocked(
        &self,
        state: &mut PreferencesStoreState,
    ) -> AppResult<Map<String, Value>> {
        if let Some(data) = state.data.as_ref() {
            return Ok(data.clone());
        }
        let data = self.load_unlocked()?;
        state.data = Some(data.clone());
        Ok(data)
    }

    pub(in crate::config::preferences::store) fn save_cached_unlocked(
        &self,
        state: &mut PreferencesStoreState,
        data: &Map<String, Value>,
    ) -> AppResult<()> {
        self.save_unlocked(data)?;
        state.data = Some(data.clone());
        state.watched_symbols = None;
        Ok(())
    }

    pub(in crate::config::preferences::store) fn load_unlocked(
        &self,
    ) -> AppResult<Map<String, Value>> {
        let content = match std::fs::read_to_string(&self.path) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Map::new()),
            Err(error) => return Err(error.into()),
        };
        if content.trim().is_empty() {
            return Ok(Map::new());
        }
        match serde_json::from_str::<Value>(&content)? {
            Value::Object(map) => Ok(map),
            _ => Ok(Map::new()),
        }
    }

    pub(in crate::config::preferences::store) fn save_unlocked(
        &self,
        data: &Map<String, Value>,
    ) -> AppResult<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp_path = self.path.with_extension("tmp");
        let content = serde_json::to_string_pretty(data)?;
        std::fs::write(&tmp_path, content)?;
        std::fs::rename(&tmp_path, &self.path)?;
        Ok(())
    }
}
