use bitcoin::BlockHash;
use lightning::chain::channelmonitor::ChannelMonitor;
use lightning::sign::EntropySource;
use lightning::sign::SignerProvider;
use lightning::util::persist::KVStorePersister;
use lightning::util::ser::Writeable;
use lightning_persister::FilesystemPersister;
use ln_dlc_node::storage::LDKStoreReader;
use ln_dlc_storage::sled::SledStorageProvider;
use ln_dlc_storage::DlcStoreProvider;
use ln_dlc_storage::KeyValue;
use std::fs;
use std::ops::Deref;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct CoordinatorTenTenOneStorage {
    pub ln_storage: Arc<FilesystemPersister>,
    pub dlc_storage: Arc<SledStorageProvider>,
    pub data_dir: String,
}

impl CoordinatorTenTenOneStorage {
    pub fn new(data_dir: String) -> CoordinatorTenTenOneStorage {
        let data_dir = PathBuf::from(data_dir);

        if !data_dir.exists() {
            fs::create_dir_all(data_dir.as_path()).expect("Failed to create data dir");
        }

        let data_dir = data_dir.to_string_lossy().to_string();

        let ln_storage = Arc::new(FilesystemPersister::new(data_dir.clone()));
        let dlc_storage = Arc::new(SledStorageProvider::new(&data_dir));

        CoordinatorTenTenOneStorage {
            ln_storage,
            dlc_storage,
            data_dir,
        }
    }
}

impl LDKStoreReader for CoordinatorTenTenOneStorage {
    fn read_network_graph(&self) -> Option<Vec<u8>> {
        let path = &format!("{}/network_graph", self.data_dir);
        let network_graph_path = Path::new(path);
        network_graph_path
            .exists()
            .then(|| fs::read(network_graph_path).expect("network graph to be readable"))
    }

    fn read_manager(&self) -> Option<Vec<u8>> {
        let path = &format!("{}/manager", self.data_dir);
        let manager_path = Path::new(path);
        manager_path
            .exists()
            .then(|| fs::read(manager_path).expect("manager to be readable"))
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
        self.ln_storage
            .read_channelmonitors(entropy_source, signer_provider)
    }

    fn export(&self) -> anyhow::Result<Vec<(String, Vec<u8>)>> {
        unimplemented!("Exporting the coordinators lightning data is not supported")
    }
}

impl DlcStoreProvider for CoordinatorTenTenOneStorage {
    fn read(&self, kind: u8, key: Option<Vec<u8>>) -> anyhow::Result<Vec<KeyValue>> {
        self.dlc_storage.read(kind, key)
    }

    fn write(&self, kind: u8, key: Vec<u8>, value: Vec<u8>) -> anyhow::Result<()> {
        self.dlc_storage.write(kind, key, value)
    }

    fn delete(&self, kind: u8, key: Option<Vec<u8>>) -> anyhow::Result<()> {
        self.dlc_storage.delete(kind, key)
    }
}

impl KVStorePersister for CoordinatorTenTenOneStorage {
    fn persist<W: Writeable>(&self, key: &str, value: &W) -> std::io::Result<()> {
        self.ln_storage.persist(key, value)?;
        Ok(())
    }
}
