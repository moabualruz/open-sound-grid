use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use super::FilterHandle;

/// Thread-safe store of FilterHandles keyed by channel name (e.g. "osg.filter.{ulid}").
/// Shared between the PW mainloop (writes peaks) and the reducer (reads peaks, sets EQ).
#[derive(Debug, Clone, Default)]
pub struct FilterHandleStore {
    inner: Arc<RwLock<HashMap<String, FilterHandle>>>,
}

#[allow(clippy::unwrap_used)]
impl FilterHandleStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a handle, returning any previous one.
    pub fn insert(&self, key: String, handle: FilterHandle) -> Option<FilterHandle> {
        self.inner.write().unwrap().insert(key, handle)
    }

    /// Remove a handle by key.
    pub fn remove(&self, key: &str) -> Option<FilterHandle> {
        self.inner.write().unwrap().remove(key)
    }

    /// Get a clone of a handle by key.
    pub fn get(&self, key: &str) -> Option<FilterHandle> {
        self.inner.read().unwrap().get(key).cloned()
    }

    /// Read all filter peaks and return (key, left, right) tuples.
    pub fn read_all_peaks(&self) -> Vec<(String, f32, f32)> {
        self.inner
            .read()
            .unwrap()
            .iter()
            .map(|(k, h)| {
                let (l, r) = h.peak();
                (k.clone(), l, r)
            })
            .collect()
    }
}
