use aes_gcm_siv::AeadInPlace;
use aes_gcm_siv::Aes256GcmSiv;
use aes_gcm_siv::KeyInit;
use aes_gcm_siv::Nonce;
use anyhow::anyhow;
use anyhow::Result;
use bitcoin::secp256k1::ecdsa::Signature;
use bitcoin::secp256k1::rand;
use bitcoin::secp256k1::rand::Rng;
use bitcoin::secp256k1::PublicKey;
use bitcoin::secp256k1::SecretKey;
use bitcoin::secp256k1::SECP256K1;

#[derive(Clone)]
pub struct AesCipher {
    secret_key: SecretKey,
    inner: Aes256GcmSiv,
}

impl AesCipher {
    pub fn new(secret_key: SecretKey) -> Self {
        let cipher = Aes256GcmSiv::new_from_slice(secret_key.secret_bytes().as_slice())
            .expect("secret key to have correct key size");
        Self {
            secret_key,
            inner: cipher,
        }
    }

    pub fn encrypt(&self, value: Vec<u8>) -> Result<Vec<u8>> {
        let nonce = generate_nonce();
        let nonce = Nonce::from_slice(&nonce);

        let mut buffer: Vec<u8> = vec![];
        buffer.extend_from_slice(value.as_slice());

        // Encrypt `buffer` in-place, replacing the plaintext contents with ciphertext
        self.inner
            .encrypt_in_place(nonce, b"", &mut buffer)
            .map_err(|e| anyhow!("{e:#}"))?;

        let mut cipher_text = nonce.to_vec();
        cipher_text.extend_from_slice(buffer.as_slice());
        Ok(cipher_text)
    }

    pub fn decrypt(&self, value: Vec<u8>) -> Result<Vec<u8>> {
        let nonce = Nonce::from_slice(&value[0..12]);

        let mut buffer: Vec<u8> = Vec::new();
        buffer.extend_from_slice(&value[12..]);

        // Decrypt `buffer` in-place, replacing its ciphertext context with the original plaintext
        self.inner
            .decrypt_in_place(nonce, b"", &mut buffer)
            .map_err(|e| anyhow!("{e:#}"))?;

        Ok(buffer.to_vec())
    }

    pub fn sign(&self, value: Vec<u8>) -> Result<Signature> {
        let message = orderbook_commons::create_sign_message(value);
        Ok(self.secret_key.sign_ecdsa(message))
    }

    pub fn public_key(&self) -> PublicKey {
        self.secret_key.public_key(SECP256K1)
    }
}

fn generate_nonce() -> [u8; 12] {
    let mut rng = rand::thread_rng();
    let mut nonce = [0u8; 12];

    rng.fill(&mut nonce);
    nonce
}

#[cfg(test)]
mod tests {
    use crate::cipher::AesCipher;
    use bitcoin::secp256k1;
    use bitcoin::secp256k1::SecretKey;
    use bitcoin::secp256k1::SECP256K1;

    #[test]
    fn cipher_backup_value() {
        let secret_key = SecretKey::new(&mut secp256k1::rand::thread_rng());
        let cipher = AesCipher::new(secret_key);
        let message = b"10101";

        let encrypted_message = cipher.encrypt(message.to_vec()).unwrap();
        assert_ne!(encrypted_message, message);

        let decrypted_message = cipher.decrypt(encrypted_message).unwrap();
        assert_eq!(decrypted_message, message);
    }

    #[test]
    fn sign_backup_value() {
        let secret_key = SecretKey::new(&mut secp256k1::rand::thread_rng());
        let cipher = AesCipher::new(secret_key);
        let message = b"10101";

        let signature = cipher.sign(message.to_vec()).unwrap();

        let message = orderbook_commons::create_sign_message(message.to_vec());
        signature
            .verify(&message, &secret_key.public_key(SECP256K1))
            .unwrap()
    }
}
