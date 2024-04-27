use crate::storage::DlcStoreProvider;
use crate::storage::KeyValue;
use anyhow::Context;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct TenTenOneInMemoryStorage {
    dlc_store: InMemoryDlcStoreProvider,
}

impl TenTenOneInMemoryStorage {
    pub fn new() -> Self {
        Self {
            dlc_store: InMemoryDlcStoreProvider::new(),
        }
    }
}

impl Default for TenTenOneInMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl DlcStoreProvider for TenTenOneInMemoryStorage {
    fn read(&self, kind: u8, key: Option<Vec<u8>>) -> anyhow::Result<Vec<KeyValue>> {
        self.dlc_store.read(kind, key)
    }

    fn write(&self, kind: u8, key: Vec<u8>, value: Vec<u8>) -> anyhow::Result<()> {
        self.dlc_store.write(kind, key, value)
    }

    fn delete(&self, kind: u8, key: Option<Vec<u8>>) -> anyhow::Result<()> {
        self.dlc_store.delete(kind, key)
    }
}

type InMemoryStore = Arc<RwLock<HashMap<u8, HashMap<Vec<u8>, Vec<u8>>>>>;

#[derive(Clone)]
pub struct InMemoryDlcStoreProvider {
    memory: InMemoryStore,
}

impl Default for InMemoryDlcStoreProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryDlcStoreProvider {
    pub fn new() -> Self {
        InMemoryDlcStoreProvider {
            memory: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl DlcStoreProvider for InMemoryDlcStoreProvider {
    fn read(&self, kind: u8, key: Option<Vec<u8>>) -> anyhow::Result<Vec<KeyValue>> {
        let store = self.memory.read();
        let store = match store.get(&kind) {
            Some(store) => store,
            None => return Ok(vec![]),
        };

        if let Some(key) = key {
            let result = match store.get(&key) {
                Some(value) => vec![KeyValue {
                    key,
                    value: value.clone(),
                }],
                None => vec![],
            };
            Ok(result)
        } else {
            Ok(store
                .clone()
                .into_iter()
                .map(|e| KeyValue {
                    key: e.0,
                    value: e.1,
                })
                .collect())
        }
    }

    fn write(&self, kind: u8, key: Vec<u8>, value: Vec<u8>) -> anyhow::Result<()> {
        self.memory
            .write()
            .entry(kind)
            .and_modify(|v| {
                v.insert(key.clone(), value.clone());
            })
            .or_insert(HashMap::from([(key, value)]));

        Ok(())
    }

    fn delete(&self, kind: u8, key: Option<Vec<u8>>) -> anyhow::Result<()> {
        if let Some(key) = key {
            self.memory
                .write()
                .get_mut(&kind)
                .context("couldn't find map")?
                .remove(&key);
        } else {
            self.memory.write().remove(&kind);
        }

        Ok(())
    }
}
