use lightning::util::persist::KVStore;
use lightning_persister::fs_store::FilesystemStore;
use ln_dlc_storage::sled::SledStorageProvider;
use ln_dlc_storage::DlcStoreProvider;
use ln_dlc_storage::KeyValue;
use std::fs;
use std::io::Error;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct CoordinatorTenTenOneStorage {
    pub ln_storage: Arc<FilesystemStore>,
    pub dlc_storage: Arc<SledStorageProvider>,
    pub data_dir: String,
}

impl CoordinatorTenTenOneStorage {
    pub fn new(data_dir: String) -> CoordinatorTenTenOneStorage {
        let data_dir = PathBuf::from(data_dir);

        if !data_dir.exists() {
            fs::create_dir_all(data_dir.as_path()).expect("Failed to create data dir");
        }

        let ln_storage = Arc::new(FilesystemStore::new(data_dir.clone()));

        let data_dir = data_dir.to_string_lossy().to_string();
        let dlc_storage = Arc::new(SledStorageProvider::new(&data_dir));

        CoordinatorTenTenOneStorage {
            ln_storage,
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

impl KVStore for CoordinatorTenTenOneStorage {
    fn read(
        &self,
        primary_namespace: &str,
        secondary_namespace: &str,
        key: &str,
    ) -> Result<Vec<u8>, Error> {
        self.ln_storage
            .read(primary_namespace, secondary_namespace, key)
    }

    fn write(
        &self,
        primary_namespace: &str,
        secondary_namespace: &str,
        key: &str,
        value: &[u8],
    ) -> Result<(), Error> {
        self.ln_storage
            .write(primary_namespace, secondary_namespace, key, value)
    }

    fn remove(
        &self,
        primary_namespace: &str,
        secondary_namespace: &str,
        key: &str,
        lazy: bool,
    ) -> Result<(), Error> {
        self.ln_storage
            .remove(primary_namespace, secondary_namespace, key, lazy)
    }

    fn list(
        &self,
        primary_namespace: &str,
        secondary_namespace: &str,
    ) -> Result<Vec<String>, Error> {
        self.ln_storage.list(primary_namespace, secondary_namespace)
    }
}
