use bitcoin::hashes::hex::FromHex;
use bitcoin::BlockHash;
use bitcoin::Txid;
use lightning::chain::channelmonitor::ChannelMonitor;
use lightning::sign::EntropySource;
use lightning::sign::SignerProvider;
use lightning::util::persist::KVStorePersister;
use lightning::util::ser::ReadableArgs;
use lightning::util::ser::Writeable;
use ln_dlc_storage::sled::InMemoryDlcStoreProvider;
use ln_dlc_storage::DlcStoreProvider;
use ln_dlc_storage::KeyValue;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::io::Cursor;
use std::ops::Deref;
use std::sync::Arc;

pub trait TenTenOneStorage:
    KVStorePersister + LDKStoreReader + DlcStoreProvider + Sync + Send + Clone
{
}

impl<T> TenTenOneStorage for T where
    T: KVStorePersister + LDKStoreReader + DlcStoreProvider + Sync + Send + Clone
{
}

/// Represents an un-opinionated storage interfaces for reading lightning data.
pub trait LDKStoreReader {
    fn read_network_graph(&self) -> Option<Vec<u8>>;
    fn read_manager(&self) -> Option<Vec<u8>>;
    #[allow(clippy::type_complexity)]
    fn read_channelmonitors<ES: Deref, SP: Deref>(
        &self,
        entropy_source: ES,
        signer_provider: SP,
    ) -> std::io::Result<
        Vec<(
            BlockHash,
            ChannelMonitor<<SP::Target as SignerProvider>::Signer>,
        )>,
    >
    where
        ES::Target: EntropySource + Sized,
        SP::Target: SignerProvider + Sized;

    /// Exports all data for a backup
    fn export(&self) -> anyhow::Result<Vec<(String, Vec<u8>)>>;
}

#[derive(Clone)]
pub struct TenTenOneInMemoryStorage {
    network_graph: Arc<RwLock<Option<Vec<u8>>>>,
    manager: Arc<RwLock<Option<Vec<u8>>>>,
    monitors: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    dlc_store: InMemoryDlcStoreProvider,
}

impl TenTenOneInMemoryStorage {
    pub fn new() -> Self {
        Self {
            network_graph: Arc::new(RwLock::new(None)),
            manager: Arc::new(RwLock::new(None)),
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

impl KVStorePersister for TenTenOneInMemoryStorage {
    fn persist<W: Writeable>(&self, key: &str, object: &W) -> std::io::Result<()> {
        let value = object.encode();
        if key == "manager" {
            *self.manager.write() = Some(value);
        } else if key.contains("monitors") {
            self.monitors.write().insert(key.to_string(), value);
        } else if key == "network_graph" {
            *self.network_graph.write() = Some(value);
        }

        Ok(())
    }
}

impl LDKStoreReader for TenTenOneInMemoryStorage {
    fn read_network_graph(&self) -> Option<Vec<u8>> {
        self.network_graph.read().clone()
    }

    fn read_manager(&self) -> Option<Vec<u8>> {
        self.manager.read().clone()
    }

    fn read_channelmonitors<ES: Deref, SP: Deref>(
        &self,
        entropy_source: ES,
        signer_provider: SP,
    ) -> std::io::Result<
        Vec<(
            BlockHash,
            ChannelMonitor<<SP::Target as SignerProvider>::Signer>,
        )>,
    >
    where
        ES::Target: EntropySource + Sized,
        SP::Target: SignerProvider + Sized,
    {
        let mut res = Vec::new();
        for entry in self.monitors.read().iter() {
            if !entry.0.contains("monitors") {
                continue;
            }

            let filename = entry.0;
            if !filename.is_ascii() || filename.len() < 65 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Invalid ChannelMonitor file name",
                ));
            }

            let txid: Txid = Txid::from_hex(filename.split_at(64).0).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid tx ID in filename")
            })?;

            let index: u16 = filename.split_at(65).1.parse().map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Invalid tx index in filename",
                )
            })?;

            let contents = entry.1;
            let mut buffer = Cursor::new(&contents);
            match <(
                BlockHash,
                ChannelMonitor<<SP::Target as SignerProvider>::Signer>,
            )>::read(&mut buffer, (&*entropy_source, &*signer_provider))
            {
                Ok((blockhash, channel_monitor)) => {
                    if channel_monitor.get_original_funding_txo().0.txid != txid
                        || channel_monitor.get_original_funding_txo().0.index != index
                    {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "ChannelMonitor was stored in the wrong file",
                        ));
                    }
                    res.push((blockhash, channel_monitor));
                }
                Err(e) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("Failed to deserialize ChannelMonitor: {}", e),
                    ))
                }
            }
        }
        Ok(res)
    }

    fn export(&self) -> anyhow::Result<Vec<(String, Vec<u8>)>> {
        unimplemented!("export not supported for in memory storage")
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
