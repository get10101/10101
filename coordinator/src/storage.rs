use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use xxi_node::storage::sled::SledStorageProvider;
use xxi_node::storage::DlcStoreProvider;
use xxi_node::storage::KeyValue;

#[derive(Clone)]
pub struct CoordinatorTenTenOneStorage {
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
        let dlc_storage = Arc::new(SledStorageProvider::new(&data_dir));

        CoordinatorTenTenOneStorage {
            dlc_storage,
            data_dir,
        }
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
