use crate::backup::RemoteBackupClient;
use crate::backup::DB_BACKUP_KEY;
use crate::backup::DB_BACKUP_NAME;
use crate::backup::DLC_BACKUP_KEY;
use crate::backup::LN_BACKUP_KEY;
use crate::cipher::AesCipher;
use crate::db;
use anyhow::Result;
use bitcoin::secp256k1::SecretKey;
use bitcoin::Network;
use lightning::util::persist::KVStore;
use lightning_persister::fs_store::FilesystemStore;
use ln_dlc_storage::sled::SledStorageProvider;
use ln_dlc_storage::DlcStoreProvider;
use ln_dlc_storage::KeyValue;
use std::fs;
use std::io::Error;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct TenTenOneNodeStorage {
    pub client: RemoteBackupClient,
    pub ln_storage: Arc<FilesystemStore>,
    pub dlc_storage: Arc<SledStorageProvider>,
    pub data_dir: String,
    pub backup_dir: String,
    pub network: Network,
}

impl TenTenOneNodeStorage {
    pub fn new(data_dir: String, network: Network, secret_key: SecretKey) -> TenTenOneNodeStorage {
        let mut data_dir = PathBuf::from(data_dir);
        data_dir.push(network.to_string());

        if !data_dir.exists() {
            fs::create_dir_all(data_dir.as_path()).expect("Failed to create data dir");
        }

        let backup_dir = data_dir.join(Path::new("backup"));
        if !backup_dir.exists() {
            fs::create_dir_all(backup_dir.as_path()).expect("Failed to create backup dir");
        }

        let backup_dir = backup_dir.to_string_lossy().to_string();
        tracing::info!("Created backup dir at {backup_dir}");

        let ln_storage = Arc::new(FilesystemStore::new(data_dir.clone()));

        let data_dir = data_dir.to_string_lossy().to_string();
        let dlc_storage = Arc::new(SledStorageProvider::new(&data_dir));
        let client = RemoteBackupClient::new(AesCipher::new(secret_key));

        TenTenOneNodeStorage {
            ln_storage,
            dlc_storage,
            data_dir,
            backup_dir,
            network,
            client,
        }
    }

    /// Creates a full backup of the lightning and dlc data.
    pub async fn full_backup(&self) -> Result<()> {
        tracing::info!("Running full backup");
        let mut handles = vec![];

        let db_backup = db::back_up()?;
        let value = fs::read(db_backup)?;
        let handle = self
            .client
            .backup(format!("{DB_BACKUP_KEY}/{DB_BACKUP_NAME}"), value);
        handles.push(handle);

        for dlc_backup in self.dlc_storage.export().into_iter() {
            let key = [
                DLC_BACKUP_KEY,
                &hex::encode([dlc_backup.kind]),
                &hex::encode(dlc_backup.key),
            ]
            .join("/");
            let handle = self.client.backup(key, dlc_backup.value);
            handles.push(handle);
        }

        futures::future::join_all(handles).await;

        tracing::info!("Successfully created a full backup!");

        Ok(())
    }
}

impl DlcStoreProvider for TenTenOneNodeStorage {
    fn read(&self, kind: u8, key: Option<Vec<u8>>) -> Result<Vec<KeyValue>> {
        self.dlc_storage.read(kind, key)
    }

    fn write(&self, kind: u8, key: Vec<u8>, value: Vec<u8>) -> Result<()> {
        self.dlc_storage.write(kind, key.clone(), value.clone())?;

        let key = [DLC_BACKUP_KEY, &hex::encode([kind]), &hex::encode(key)].join("/");

        // Let the backup run asynchronously we don't really care if it is successful or not as the
        // next write may fix the issue. Note, if we want to handle failed backup attempts we
        // would need to remember those remote handles and handle a failure accordingly.
        self.client.backup(key, value).forget();

        Ok(())
    }

    fn delete(&self, kind: u8, key: Option<Vec<u8>>) -> Result<()> {
        self.dlc_storage.delete(kind, key.clone())?;

        let key = match key {
            Some(key) => [DLC_BACKUP_KEY, &hex::encode([kind]), &hex::encode(key)].join("/"),
            None => [DLC_BACKUP_KEY, &hex::encode([kind])].join("/"),
        };

        // Let the delete backup run asynchronously we don't really care if it is successful or not.
        // We may end up with a key that should have been deleted. That should hopefully not
        // be a problem. Note, if we want to handle failed backup attempts we would need to
        // remember those remote handles and handle a failure accordingly.
        self.client.delete(key).forget();
        Ok(())
    }
}

impl KVStore for TenTenOneNodeStorage {
    fn read(
        &self,
        primary_namespace: &str,
        secondary_namespace: &str,
        key: &str,
    ) -> std::result::Result<Vec<u8>, Error> {
        self.ln_storage
            .read(primary_namespace, secondary_namespace, key)
    }

    fn write(
        &self,
        primary_namespace: &str,
        secondary_namespace: &str,
        key: &str,
        value: &[u8],
    ) -> std::result::Result<(), Error> {
        self.ln_storage
            .write(primary_namespace, secondary_namespace, key, value)?;

        let value = value.to_vec();
        let key = [primary_namespace, secondary_namespace, key]
            .into_iter()
            .filter(|&k| !k.is_empty())
            .collect::<Vec<&str>>()
            .join("/");
        tracing::trace!("Creating a backup of {:?}", key);

        // Let the backup run asynchronously we don't really care if it is successful or not as the
        // next persist will fix the issue. Note, if we want to handle failed backup attempts we
        // would need to remember those remote handles and handle a failure accordingly.
        self.client
            .backup([LN_BACKUP_KEY, &key].join("/"), value)
            .forget();

        Ok(())
    }

    fn remove(
        &self,
        primary_namespace: &str,
        secondary_namespace: &str,
        key: &str,
        lazy: bool,
    ) -> std::result::Result<(), Error> {
        self.ln_storage
            .remove(primary_namespace, secondary_namespace, key, lazy)
    }

    fn list(
        &self,
        primary_namespace: &str,
        secondary_namespace: &str,
    ) -> std::result::Result<Vec<String>, Error> {
        self.ln_storage.list(primary_namespace, secondary_namespace)
    }
}
