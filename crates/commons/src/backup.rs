use crate::signature::create_sign_message;
use secp256k1::ecdsa::Signature;
use secp256k1::PublicKey;
use serde::Deserialize;
use serde::Serialize;

/// A message to restore a key with its value.
#[derive(Serialize, Deserialize)]
pub struct Restore {
    pub key: String,
    pub value: Vec<u8>,
}

/// A message to backup a key with its value.
#[derive(Serialize, Deserialize)]
pub struct Backup {
    pub key: String,
    pub value: Vec<u8>,
    /// A signature of the value using the nodes private key
    pub signature: Signature,
}

impl Backup {
    /// Verifies if the backup was from the given node id
    pub fn verify(&self, node_id: &PublicKey) -> anyhow::Result<()> {
        let message = create_sign_message(self.value.clone());
        self.signature.verify(&message, node_id)?;
        Ok(())
    }
}

/// A message to delete a backup of a key
#[derive(Serialize, Deserialize)]
pub struct DeleteBackup {
    pub key: String,
    /// A signature of the requesting node id using the nodes private key
    pub signature: Signature,
}

impl DeleteBackup {
    pub fn verify(&self, node_id: &PublicKey) -> anyhow::Result<()> {
        let message = node_id.to_string().as_bytes().to_vec();
        let message = create_sign_message(message);
        self.signature.verify(&message, node_id)?;
        Ok(())
    }
}
