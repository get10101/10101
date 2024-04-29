use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use sled::Db;
use xxi_node::commons::Backup;
use xxi_node::commons::DeleteBackup;
use xxi_node::commons::Restore;

const BACKUPS_DIRECTORY: &str = "user_backups";

/// Holds the user backups in a sled database
///
/// TODO(holzeis): This is fine for now, once we grow we should consider moving that into a dedicate
/// KV database, potentially to a managed service.
pub struct SledBackup {
    db: Db,
}

impl SledBackup {
    pub fn new(data_dir: String) -> Self {
        SledBackup {
            db: sled::open(format!("{data_dir}/{BACKUPS_DIRECTORY}")).expect("valid path"),
        }
    }

    pub fn restore(&self, node_id: PublicKey) -> Result<Vec<Restore>> {
        tracing::debug!(%node_id, "Restoring backup");
        let tree = self.db.open_tree(node_id.to_string())?;

        let mut backup = vec![];
        for entry in tree.into_iter() {
            let entry = entry?;
            let key = String::from_utf8(entry.0.to_vec())?;
            let value = entry.1.to_vec();
            backup.push(Restore { key, value });
        }

        Ok(backup)
    }

    pub async fn back_up(&self, node_id: PublicKey, backup: Backup) -> Result<()> {
        tracing::debug!(%node_id, backup.key, "Create user backup");
        let tree = self.db.open_tree(node_id.to_string())?;
        tree.insert(backup.key, backup.value)?;
        tree.flush()?;
        Ok(())
    }

    pub fn delete(&self, node_id: PublicKey, backup: DeleteBackup) -> Result<()> {
        tracing::debug!(%node_id, key=backup.key, "Deleting user backup");
        let tree = self.db.open_tree(node_id.to_string())?;
        tree.remove(backup.key)?;
        tree.flush()?;
        Ok(())
    }
}
