use crate::storage::DlcStoreProvider;
use crate::storage::KeyValue;
use anyhow::Context;
use lightning::util::persist::KVStore;
use lightning::util::persist::CHANNEL_MONITOR_PERSISTENCE_PRIMARY_NAMESPACE;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::io::Error;
use std::io::ErrorKind;
use std::sync::Arc;

#[derive(Clone)]
pub struct TenTenOneInMemoryStorage {
    network_graph: Arc<RwLock<Option<Vec<u8>>>>,
    manager: Arc<RwLock<Option<Vec<u8>>>>,
    scorer: Arc<RwLock<Option<Vec<u8>>>>,
    monitors: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    dlc_store: InMemoryDlcStoreProvider,
}

impl TenTenOneInMemoryStorage {
    pub fn new() -> Self {
        Self {
            network_graph: Arc::new(RwLock::new(None)),
            manager: Arc::new(RwLock::new(None)),
            scorer: Arc::new(RwLock::new(None)),
            monitors: Arc::new(RwLock::new(HashMap::new())),
            dlc_store: InMemoryDlcStoreProvider::new(),
        }
    }
}

impl Default for TenTenOneInMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl KVStore for TenTenOneInMemoryStorage {
    fn read(
        &self,
        primary_namespace: &str,
        secondary_namespace: &str,
        key: &str,
    ) -> Result<Vec<u8>, Error> {
        let value = match key {
            "manager" => self.manager.read().clone(),
            "network_graph" => self.network_graph.read().clone(),
            "scorer" => self.scorer.read().clone(),
            _ if primary_namespace == CHANNEL_MONITOR_PERSISTENCE_PRIMARY_NAMESPACE => {
                self.monitors.read().get(key).cloned()
            }
            _ => None,
        };

        value.ok_or(Error::new(
            ErrorKind::NotFound,
            format!("{primary_namespace}/{secondary_namespace}/{key}"),
        ))
    }

    fn write(
        &self,
        primary_namespace: &str,
        _secondary_namespace: &str,
        key: &str,
        value: &[u8],
    ) -> Result<(), Error> {
        match key {
            "manager" => *self.manager.write() = Some(value.to_vec()),
            "network_graph" => *self.network_graph.write() = Some(value.to_vec()),
            "scorer" => *self.scorer.write() = Some(value.to_vec()),
            _ if primary_namespace == CHANNEL_MONITOR_PERSISTENCE_PRIMARY_NAMESPACE => {
                self.monitors
                    .write()
                    .insert(key.to_string(), value.to_vec());
            }
            _ => tracing::warn!(primary_namespace, _secondary_namespace, key, "unknown key"),
        }

        Ok(())
    }

    fn remove(
        &self,
        primary_namespace: &str,
        _secondary_namespace: &str,
        key: &str,
        _lazy: bool,
    ) -> Result<(), Error> {
        match key {
            "manager" => *self.manager.write() = None,
            "network_graph" => *self.network_graph.write() = None,
            "scorer" => *self.scorer.write() = None,
            _ if primary_namespace == CHANNEL_MONITOR_PERSISTENCE_PRIMARY_NAMESPACE => {
                self.monitors.write().remove(key);
            }
            _ => tracing::warn!(primary_namespace, _secondary_namespace, key, "unknown key"),
        }

        Ok(())
    }

    fn list(
        &self,
        primary_namespace: &str,
        _secondary_namespace: &str,
    ) -> Result<Vec<String>, Error> {
        if primary_namespace == CHANNEL_MONITOR_PERSISTENCE_PRIMARY_NAMESPACE {
            let store = self.monitors.read().clone();
            let monitors = store.into_keys().collect();
            return Ok(monitors);
        }

        Ok(vec![])
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
